//! End-to-end tests against a real Postgres. Run with:
//!   TEST_DATABASE_URL=postgres://postgres@127.0.0.1:55432/dearus_test cargo test
//! Without TEST_DATABASE_URL these tests are skipped.

use dear_us_cloud::{build_state, config::Config, router};
use reqwest::StatusCode;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use uuid::Uuid;

struct Server {
    base: String,
    db: sqlx::PgPool,
    _dir: std::path::PathBuf,
}

async fn start() -> Option<Server> {
    let Ok(url) = std::env::var("TEST_DATABASE_URL") else {
        eprintln!("TEST_DATABASE_URL not set: skipping cloud API tests");
        return None;
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let dir = std::env::temp_dir().join(format!("dearus-cloud-test-{}", Uuid::new_v4()));
    let state = build_state(Config::for_tests(&url, &base, dir.to_str().unwrap())).await.expect("state");
    let db = state.db.clone();
    tokio::spawn(async move { axum::serve(listener, router(state)).await.unwrap() });
    Some(Server { base, db, _dir: dir })
}

struct Client {
    http: reqwest::Client,
    base: String,
    token: String,
    refresh: String,
    user: Uuid,
}

impl Server {
    async fn signup(&self) -> Client {
        let http = reqwest::Client::new();
        let email = format!("{}@test.dearus", Uuid::new_v4());
        let r: Value = http.post(format!("{}/v1/auth/signup", self.base)).json(&json!({"email": email, "password": "correct horse"})).send().await.unwrap().json().await.unwrap();
        Client {
            http,
            base: self.base.clone(),
            token: r["access_token"].as_str().unwrap().into(),
            refresh: r["refresh_token"].as_str().unwrap().into(),
            user: r["user"]["id"].as_str().unwrap().parse().unwrap(),
        }
    }
}

impl Client {
    async fn call(&self, method: reqwest::Method, path: &str, body: Option<Value>) -> (StatusCode, Value) {
        let mut req = self.http.request(method, format!("{}{path}", self.base)).bearer_auth(&self.token);
        if let Some(b) = body {
            req = req.json(&b);
        }
        let r = req.send().await.unwrap();
        let status = r.status();
        let text = r.text().await.unwrap();
        (status, serde_json::from_str(&text).unwrap_or(Value::Null))
    }
    async fn post(&self, path: &str, body: Value) -> (StatusCode, Value) {
        self.call(reqwest::Method::POST, path, Some(body)).await
    }
    async fn get(&self, path: &str) -> (StatusCode, Value) {
        self.call(reqwest::Method::GET, path, None).await
    }
}

fn sha(s: &str) -> String {
    hex::encode(Sha256::digest(s.as_bytes()))
}

/// Two people, paired, one PC each.
async fn couple(s: &Server) -> (Client, Client, Uuid, Uuid, Uuid) {
    let a = s.signup().await;
    let b = s.signup().await;
    let space = Uuid::new_v4();
    let (da, db) = (Uuid::new_v4(), Uuid::new_v4());
    assert_eq!(a.post("/v1/spaces", json!({"id": space})).await.0, StatusCode::OK);
    assert_eq!(a.post(&format!("/v1/spaces/{space}/devices"), json!({"id": da, "name": "Laptop A"})).await.0, StatusCode::OK);
    let secret = "invite-secret-".to_string() + &Uuid::new_v4().to_string();
    assert_eq!(a.post(&format!("/v1/spaces/{space}/invites"), json!({"secret_hash": sha(&secret)})).await.0, StatusCode::OK);
    let (st, r) = b.post("/v1/invites/redeem", json!({"secret": secret})).await;
    assert_eq!(st, StatusCode::OK, "{r}");
    assert_eq!(r["space_id"], json!(space));
    assert_eq!(b.post(&format!("/v1/spaces/{space}/devices"), json!({"id": db, "name": "Desktop B"})).await.0, StatusCode::OK);
    // the invite is one-time
    let (st, _) = s.signup().await.post("/v1/invites/redeem", json!({"secret": secret})).await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    (a, b, space, da, db)
}

#[tokio::test]
async fn accounts_and_tokens() {
    let Some(s) = start().await else { return };
    let mut a = s.signup().await;
    // wrong password, then a duplicate email
    let http = reqwest::Client::new();
    let email: String = sqlx::query_scalar("select email from users where id = $1").bind(a.user).fetch_one(&s.db).await.unwrap();
    let r = http.post(format!("{}/v1/auth/signin", s.base)).json(&json!({"email": email, "password": "wrong password"})).send().await.unwrap();
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
    let r = http.post(format!("{}/v1/auth/signup", s.base)).json(&json!({"email": email.to_uppercase(), "password": "whatever123"})).send().await.unwrap();
    assert_eq!(r.status(), StatusCode::CONFLICT);
    // refresh rotates: the old refresh token stops working
    let r: Value = http.post(format!("{}/v1/auth/refresh", s.base)).json(&json!({"refresh_token": a.refresh})).send().await.unwrap().json().await.unwrap();
    let r2 = http.post(format!("{}/v1/auth/refresh", s.base)).json(&json!({"refresh_token": a.refresh})).send().await.unwrap();
    assert_eq!(r2.status(), StatusCode::UNAUTHORIZED);
    a.token = r["access_token"].as_str().unwrap().into();
    assert_eq!(a.post("/v1/spaces", json!({"id": Uuid::new_v4()})).await.0, StatusCode::OK);
    // no token
    let r = http.post(format!("{}/v1/spaces", s.base)).json(&json!({"id": Uuid::new_v4()})).send().await.unwrap();
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn pairing_starts_the_trial_and_strangers_stay_out() {
    let Some(s) = start().await else { return };
    let a = s.signup().await;
    let space = Uuid::new_v4();
    a.post("/v1/spaces", json!({"id": space})).await;
    let (_, p) = a.get(&format!("/v1/spaces/{space}/plan")).await;
    assert_eq!(p["tier"], "solo");
    let stranger = s.signup().await;
    assert_eq!(stranger.get(&format!("/v1/spaces/{space}/devices")).await.0, StatusCode::FORBIDDEN);
    assert_eq!(stranger.post("/v1/spaces", json!({"id": space})).await.0, StatusCode::FORBIDDEN, "can't take over someone's space id");

    let (a, _b, space, _, _) = couple(&s).await;
    let (_, p) = a.get(&format!("/v1/spaces/{space}/plan")).await;
    assert_eq!(p["tier"], "trial");
    assert_eq!(p["media_cap_bytes"], Value::Null);
    assert_eq!(p["cupid_remaining"], 100);
    let (_, devs) = a.get(&format!("/v1/spaces/{space}/devices")).await;
    assert_eq!(devs.as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn mailbox_roundtrip_envelopes_and_files() {
    let Some(s) = start().await else { return };
    let (a, b, space, da, db) = couple(&s).await;

    // envelope A -> B
    let env = Uuid::new_v4();
    let (st, _) = a.post(&format!("/v1/spaces/{space}/envelopes"), json!({"id": env, "sender_device": da, "recipients": [db], "ciphertext": "sealed"})).await;
    assert_eq!(st, StatusCode::OK);
    // idempotent
    assert_eq!(a.post(&format!("/v1/spaces/{space}/envelopes"), json!({"id": env, "sender_device": da, "recipients": [db], "ciphertext": "sealed"})).await.0, StatusCode::OK);
    let (_, got) = b.get(&format!("/v1/spaces/{space}/envelopes?device={db}&after=0&limit=20")).await;
    assert_eq!(got.as_array().unwrap().len(), 1);
    assert_eq!(got[0]["ciphertext"], "sealed");
    // B can't pretend to be A's device
    assert_eq!(b.post(&format!("/v1/spaces/{space}/envelopes/{env}/ack"), json!({"device": da})).await.0, StatusCode::FORBIDDEN);
    assert_eq!(a.get(&format!("/v1/spaces/{space}/envelopes/{env}/delivered")).await.1, json!(false));
    assert_eq!(a.call(reqwest::Method::DELETE, &format!("/v1/spaces/{space}/envelopes/{env}"), None).await.1, json!(false), "not yet delivered");
    b.post(&format!("/v1/spaces/{space}/envelopes/{env}/ack"), json!({"device": db})).await;
    assert_eq!(a.call(reqwest::Method::DELETE, &format!("/v1/spaces/{space}/envelopes/{env}"), None).await.1, json!(true));

    // a 10-byte file in two chunks
    let hash = sha("file");
    let (st, r) = a.call(reqwest::Method::PUT, &format!("/v1/spaces/{space}/blobs/{hash}"), Some(json!({"size": 10, "chunk_size": 8, "chunk_count": 2, "sender_device": da, "recipients": [db]}))).await;
    assert_eq!(st, StatusCode::OK, "{r}");
    for (n, data) in [(0, b"chunk-zero".to_vec()), (1, b"one".to_vec())] {
        let (st, link) = a.post(&format!("/v1/spaces/{space}/blobs/{hash}/chunks/{n}/upload"), json!({})).await;
        assert_eq!(st, StatusCode::OK, "{link}");
        let put = a.http.put(link["url"].as_str().unwrap()).body(data).send().await.unwrap();
        assert!(put.status().is_success());
    }
    // only the sender's own devices may upload
    assert_eq!(b.post(&format!("/v1/spaces/{space}/blobs/{hash}/chunks/0/upload"), json!({})).await.0, StatusCode::FORBIDDEN);
    let (_, link) = b.post(&format!("/v1/spaces/{space}/blobs/{hash}/chunks/0/download"), json!({})).await;
    let bytes = b.http.get(link["url"].as_str().unwrap()).send().await.unwrap().bytes().await.unwrap();
    assert_eq!(&bytes[..], b"chunk-zero");
    // a tampered link is refused
    let bad = link["url"].as_str().unwrap().replace("sig=", "sig=x");
    assert_eq!(b.http.get(bad).send().await.unwrap().status(), StatusCode::FORBIDDEN);

    let (_, meta) = b.get(&format!("/v1/spaces/{space}/blobs/{hash}")).await;
    assert_eq!(meta["chunk_count"], 2);
    b.post(&format!("/v1/spaces/{space}/blobs/{hash}/ack"), json!({"device": db})).await;
    assert_eq!(a.call(reqwest::Method::DELETE, &format!("/v1/spaces/{space}/blobs/{hash}"), None).await.1, json!(true));
    assert_eq!(b.get(&format!("/v1/spaces/{space}/blobs/{hash}")).await.1, Value::Null);
    assert_eq!(b.http.get(link["url"].as_str().unwrap()).send().await.unwrap().status(), StatusCode::NOT_FOUND, "chunks are deleted");

    // revoked devices stop counting as recipients and can't send
    a.post(&format!("/v1/spaces/{space}/devices/{db}/revoke"), json!({})).await;
    let env2 = Uuid::new_v4();
    a.post(&format!("/v1/spaces/{space}/envelopes"), json!({"id": env2, "sender_device": da, "recipients": [db], "ciphertext": "x"})).await;
    assert_eq!(a.get(&format!("/v1/spaces/{space}/envelopes/{env2}/delivered")).await.1, json!(true));
    assert_eq!(b.post(&format!("/v1/spaces/{space}/envelopes"), json!({"id": Uuid::new_v4(), "sender_device": db, "recipients": [da], "ciphertext": "x"})).await.0, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn free_tier_caps_cloud_media_per_month_but_files_over_wifi_are_free() {
    let Some(s) = start().await else { return };
    let (a, _b, space, da, db) = couple(&s).await;
    sqlx::query("update spaces set trial_ends_at = now() - interval '1 day' where id = $1").bind(space).execute(&s.db).await.unwrap();
    let mb = 1024 * 1024i64;
    let announce = |hash: String, size: i64| {
        let a = &a;
        async move {
            let chunk = 8 * mb;
            let count = (size + chunk - 1) / chunk;
            a.call(reqwest::Method::PUT, &format!("/v1/spaces/{space}/blobs/{hash}"), Some(json!({"size": size, "chunk_size": chunk, "chunk_count": count, "sender_device": da, "recipients": [db]})))
                .await
        }
    };
    // announced but delivered over Wi-Fi: never charged
    assert_eq!(announce(sha("wifi"), 90 * mb).await.0, StatusCode::OK);
    let (_, p) = a.get(&format!("/v1/spaces/{space}/plan")).await;
    assert_eq!(p["tier"], "free");
    assert_eq!(p["media_used_bytes"], 0);

    assert_eq!(announce(sha("a"), 60 * mb).await.0, StatusCode::OK);
    assert_eq!(a.post(&format!("/v1/spaces/{space}/blobs/{}/chunks/0/upload", sha("a")), json!({})).await.0, StatusCode::OK);
    // later chunks of the same file are not charged again
    assert_eq!(a.post(&format!("/v1/spaces/{space}/blobs/{}/chunks/1/upload", sha("a")), json!({})).await.0, StatusCode::OK);
    assert_eq!(a.get(&format!("/v1/spaces/{space}/usage")).await.1, json!(60 * mb));

    assert_eq!(announce(sha("b"), 50 * mb).await.0, StatusCode::OK);
    let (st, e) = a.post(&format!("/v1/spaces/{space}/blobs/{}/chunks/0/upload", sha("b")), json!({})).await;
    assert_eq!(st, StatusCode::PAYMENT_REQUIRED);
    assert_eq!(e["code"], "media_quota");

    // a new month starts fresh
    sqlx::query("update spaces set media_period = '2000-01' where id = $1").bind(space).execute(&s.db).await.unwrap();
    assert_eq!(a.post(&format!("/v1/spaces/{space}/blobs/{}/chunks/0/upload", sha("b")), json!({})).await.0, StatusCode::OK);
    // premium: no cap
    sqlx::query("update spaces set premium_until = now() + interval '30 days' where id = $1").bind(space).execute(&s.db).await.unwrap();
    assert_eq!(announce(sha("c"), 500 * mb).await.0, StatusCode::OK);
    assert_eq!(a.post(&format!("/v1/spaces/{space}/blobs/{}/chunks/0/upload", sha("c")), json!({})).await.0, StatusCode::OK);
    assert_eq!(a.get(&format!("/v1/spaces/{space}/plan")).await.1["tier"], "premium");
}

#[tokio::test]
async fn cupid_arrows_are_counted_and_refunded_on_failure() {
    let Some(s) = start().await else { return };
    let (a, _b, space, _, _) = couple(&s).await;
    let ask = json!({"kind": "chat", "system": "You are Cupid.", "turns": [{"role": "user", "text": "hi"}]});
    // the test server has no Gemini key: the request fails and the arrow comes back
    let (st, _) = a.post(&format!("/v1/spaces/{space}/cupid"), ask.clone()).await;
    assert_eq!(st, StatusCode::SERVICE_UNAVAILABLE);
    let (_, p) = a.get(&format!("/v1/spaces/{space}/plan")).await;
    assert_eq!(p["cupid_used"], 0);

    // free tier with the month's 10 used and 1 top-up arrow
    sqlx::query("update spaces set trial_ends_at = now() - interval '1 day', cupid_period = to_char(now() at time zone 'utc', 'YYYY-MM'), cupid_used = 10, cupid_bonus = 1 where id = $1")
        .bind(space)
        .execute(&s.db)
        .await
        .unwrap();
    let (_, p) = a.get(&format!("/v1/spaces/{space}/plan")).await;
    assert_eq!(p["cupid_remaining"], 1);
    a.post(&format!("/v1/spaces/{space}/cupid"), ask.clone()).await; // fails (no key) → bonus refunded
    let (_, p) = a.get(&format!("/v1/spaces/{space}/plan")).await;
    assert_eq!(p["cupid_bonus"], 1, "top-up arrow refunded");
    sqlx::query("update spaces set cupid_bonus = 0 where id = $1").bind(space).execute(&s.db).await.unwrap();
    let (st, e) = a.post(&format!("/v1/spaces/{space}/cupid"), ask).await;
    assert_eq!(st, StatusCode::PAYMENT_REQUIRED);
    assert_eq!(e["code"], "cupid_quota");

    // the daily question pick is free, once a day
    let pick = json!({"kind": "daily_pick", "system": "s", "turns": [{"role": "user", "text": "pick"}], "json": true});
    assert_eq!(a.post(&format!("/v1/spaces/{space}/cupid"), pick.clone()).await.0, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(a.post(&format!("/v1/spaces/{space}/cupid"), pick).await.0, StatusCode::CONFLICT);
}
