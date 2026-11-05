//! Direct PC-to-PC file transfer on the same network (Phase 6) — an optimization, never a
//! requirement: anything that fails here falls back to the cloud relay.
//!
//! - Each PC runs a tiny HTTP server that serves ONLY the photos/videos it is currently
//!   sending to the partner (rows in `sync_blobs` with direction 'up'), never arbitrary files.
//! - Chunks are sealed with the space key exactly like cloud chunks (same AAD), so the
//!   receiver verifies them the same way, and a network eavesdropper sees only ciphertext.
//! - Every request carries a short-lived MAC made with the space key: only the partner's
//!   PC can ask for anything.

use super::crypto::SpaceKey;
use crate::error::{AppError, AppResult};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::sync::Arc;
use std::time::Duration;

pub type ChunkProvider = Arc<dyn Fn(&str, i64) -> AppResult<Vec<u8>> + Send + Sync>;

const WINDOW_MINUTES: i64 = 3;

fn minute() -> i64 {
    chrono::Utc::now().timestamp() / 60
}

fn sign(key: &SpaceKey, path: &str, minute: i64) -> String {
    key.mac(format!("dearus-direct-v1|{path}|{minute}").as_bytes())
}

fn authorized(key: &SpaceKey, path: &str, token: &str) -> bool {
    let now = minute();
    (now - WINDOW_MINUTES..=now + WINDOW_MINUTES).any(|m| constant_eq(&sign(key, path, m), token))
}

fn constant_eq(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// This PC's address on the local network (no packet is actually sent).
pub fn lan_ip() -> Option<String> {
    let s = UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("192.0.2.1:80").ok()?; // TEST-NET address: only used to pick the outgoing interface
    let ip = s.local_addr().ok()?.ip();
    (!ip.is_loopback() && !ip.is_unspecified()).then(|| ip.to_string())
}

fn respond(mut s: TcpStream, status: &str, body: &[u8]) {
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = s.write_all(head.as_bytes());
    let _ = s.write_all(body);
}

fn handle(stream: TcpStream, key: &SpaceKey, provider: &ChunkProvider) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let mut reader = BufReader::new(match stream.try_clone() {
        Ok(s) => s,
        Err(_) => return,
    });
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() || line.len() > 300 {
        return;
    }
    let mut token = String::new();
    for _ in 0..40 {
        let mut h = String::new();
        if reader.read_line(&mut h).is_err() || h == "\r\n" || h.is_empty() {
            break;
        }
        if let Some(v) = h.strip_prefix("X-DearUs-Auth:").or_else(|| h.strip_prefix("x-dearus-auth:")) {
            token = v.trim().to_string();
        }
    }
    let mut parts = line.split_whitespace();
    let (method, path) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
    if method != "GET" {
        return respond(stream, "405 Method Not Allowed", b"");
    }
    if !authorized(key, path, &token) {
        return respond(stream, "401 Unauthorized", b"");
    }
    if path == "/ping" {
        return respond(stream, "200 OK", b"dearus");
    }
    let seg: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    if seg.len() == 3 && seg[0] == "chunk" && seg[1].len() == 64 && seg[1].chars().all(|c| c.is_ascii_hexdigit()) {
        if let Ok(n) = seg[2].parse::<i64>() {
            return match provider(seg[1], n) {
                Ok(bytes) => respond(stream, "200 OK", &bytes),
                Err(_) => respond(stream, "404 Not Found", b""),
            };
        }
    }
    respond(stream, "404 Not Found", b"")
}

/// Starts the server. Returns the port it listens on.
pub fn serve(bind: &str, key: SpaceKey, provider: ChunkProvider) -> AppResult<u16> {
    let listener = TcpListener::bind(bind)?;
    let port = listener.local_addr()?.port();
    std::thread::Builder::new()
        .name("direct-transfer".into())
        .spawn(move || {
            for conn in listener.incoming().flatten() {
                let (k, p) = (key.clone(), provider.clone());
                std::thread::spawn(move || handle(conn, &k, &p));
            }
        })
        .map_err(|e| AppError::invalid(e.to_string()))?;
    Ok(port)
}

/// Talks to the partner's PC directly, if it's reachable.
pub struct DirectClient {
    key: SpaceKey,
    endpoints: HashMap<String, String>,
    http: ureq::Agent,
}

impl DirectClient {
    pub fn new(key: SpaceKey, endpoints: HashMap<String, String>) -> DirectClient {
        let http = ureq::AgentBuilder::new().timeout_connect(Duration::from_secs(2)).timeout(Duration::from_secs(60)).build();
        DirectClient { key, endpoints, http }
    }

    fn get(&self, device: &str, path: &str) -> AppResult<Vec<u8>> {
        let ep = self.endpoints.get(device).ok_or_else(|| AppError::invalid("no direct address"))?;
        let r = self
            .http
            .get(&format!("http://{ep}{path}"))
            .set("X-DearUs-Auth", &sign(&self.key, path, minute()))
            .call()
            .map_err(|e| AppError::invalid(format!("direct: {e}")))?;
        let mut buf = Vec::new();
        std::io::Read::read_to_end(&mut std::io::Read::take(r.into_reader(), 64 * 1024 * 1024), &mut buf)?;
        Ok(buf)
    }

    /// Any partner device we can reach right now.
    pub fn reachable(&self) -> Option<String> {
        self.endpoints.keys().find(|d| self.get(d, "/ping").map(|b| b == b"dearus").unwrap_or(false)).cloned()
    }

    pub fn chunk(&self, device: &str, hash: &str, n: i64) -> AppResult<Vec<u8>> {
        self.get(device, &format!("/chunk/{hash}/{n}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_partner_with_the_key_gets_chunks() {
        let key = SpaceKey::generate();
        let hash = "a".repeat(64);
        let provider: ChunkProvider = Arc::new(|h: &str, n: i64| Ok(format!("{h}:{n}").into_bytes()));
        let port = serve("127.0.0.1:0", key.clone(), provider).unwrap();
        let mut eps = HashMap::new();
        eps.insert("partner".to_string(), format!("127.0.0.1:{port}"));

        let good = DirectClient::new(key.clone(), eps.clone());
        assert_eq!(good.reachable().as_deref(), Some("partner"));
        assert_eq!(good.chunk("partner", &hash, 3).unwrap(), format!("{hash}:3").into_bytes());

        let stranger = DirectClient::new(SpaceKey::generate(), eps);
        assert!(stranger.reachable().is_none());
        assert!(stranger.chunk("partner", &hash, 3).is_err(), "wrong key is refused");
    }

    #[test]
    fn auth_tokens_expire() {
        let key = SpaceKey::generate();
        let old = sign(&key, "/ping", minute() - 10);
        assert!(!authorized(&key, "/ping", &old));
        assert!(authorized(&key, "/ping", &sign(&key, "/ping", minute())));
        assert!(!authorized(&key, "/other", &sign(&key, "/ping", minute())), "bound to the path");
    }
}
