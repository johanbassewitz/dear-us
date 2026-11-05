//! Hybrid Logical Clock: wall-clock millis + counter + device id.
//! Strings compare correctly with plain string ordering (fixed-width, zero-padded),
//! so "newer edit wins" works even if the two PCs' clocks drift a little.

use crate::error::AppResult;
use rusqlite::{params, Connection, OptionalExtension};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Stamp {
    pub millis: u64,
    pub counter: u32,
}

impl Stamp {
    pub fn format(&self, device: &str) -> String {
        format!("{:015}-{:06}-{}", self.millis, self.counter, device)
    }

    pub fn parse(s: &str) -> Option<Stamp> {
        let mut it = s.splitn(3, '-');
        let millis = it.next()?.parse().ok()?;
        let counter = it.next()?.parse().ok()?;
        Some(Stamp { millis, counter })
    }
}

/// Next stamp after `last`, given the current wall time.
pub fn tick(last: Option<Stamp>, wall_millis: u64) -> Stamp {
    match last {
        Some(l) if l.millis >= wall_millis => Stamp { millis: l.millis, counter: l.counter + 1 },
        _ => Stamp { millis: wall_millis, counter: 0 },
    }
}

/// Merge a remote stamp into our clock.
pub fn receive(last: Option<Stamp>, remote: Stamp, wall_millis: u64) -> Stamp {
    let l = last.unwrap_or(Stamp { millis: 0, counter: 0 });
    let millis = l.millis.max(remote.millis).max(wall_millis);
    let counter = if millis == l.millis && millis == remote.millis {
        l.counter.max(remote.counter) + 1
    } else if millis == l.millis {
        l.counter + 1
    } else if millis == remote.millis {
        remote.counter + 1
    } else {
        0
    };
    Stamp { millis, counter }
}

/// Issue the next HLC string for this device and persist it (inside the caller's transaction).
pub fn next(conn: &Connection, device: &str) -> AppResult<String> {
    let last: Option<String> = conn
        .query_row("SELECT value FROM local_meta WHERE key = 'hlc'", [], |r| r.get(0))
        .optional()?;
    let wall = chrono::Utc::now().timestamp_millis().max(0) as u64;
    let stamp = tick(last.as_deref().and_then(Stamp::parse), wall);
    let s = stamp.format(device);
    conn.execute(
        "INSERT INTO local_meta(key, value) VALUES ('hlc', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![s],
    )?;
    Ok(s)
}

/// Merge a remote HLC into ours, so our next stamps sort after everything we've seen.
pub fn observe(conn: &Connection, remote: &str, device: &str) -> AppResult<()> {
    let Some(remote) = Stamp::parse(remote) else { return Ok(()) };
    let last: Option<String> = conn
        .query_row("SELECT value FROM local_meta WHERE key = 'hlc'", [], |r| r.get(0))
        .optional()?;
    let wall = chrono::Utc::now().timestamp_millis().max(0) as u64;
    let s = receive(last.as_deref().and_then(Stamp::parse), remote, wall).format(device);
    conn.execute(
        "INSERT INTO local_meta(key, value) VALUES ('hlc', ?1) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![s],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monotonic_even_if_wall_clock_goes_backwards() {
        let a = tick(None, 1000);
        let b = tick(Some(a), 900);
        let c = tick(Some(b), 1000);
        let d = tick(Some(c), 2000);
        assert!(a < b && b < c && c < d);
        assert!(a.format("x") < b.format("x") && c.format("x") < d.format("x"));
    }

    #[test]
    fn receive_moves_past_remote() {
        let local = Stamp { millis: 1000, counter: 3 };
        let remote = Stamp { millis: 5000, counter: 7 };
        let r = receive(Some(local), remote, 2000);
        assert!(r > remote && r > local);
    }

    #[test]
    fn roundtrip_format() {
        let s = Stamp { millis: 1_762_000_000_000, counter: 12 };
        assert_eq!(Stamp::parse(&s.format("dev-1")), Some(s));
    }

    #[test]
    fn next_persists_and_increases() {
        let conn = crate::db::open_in_memory().unwrap();
        let a = next(&conn, "d").unwrap();
        let b = next(&conn, "d").unwrap();
        assert!(a < b);
    }
}
