//! Secrets (Supabase session, space key, Gemini key…) live in Windows Credential Manager,
//! never in the database or plain files, and are never synced.
//! Secret values are only read inside Rust — no command returns them to the UI.

use crate::error::{AppError, AppResult};

const SERVICE: &str = "app.dearus.journal";

fn entry(name: &str) -> AppResult<keyring::Entry> {
    if name.is_empty() || name.len() > 64 {
        return Err(AppError::invalid("invalid secret name"));
    }
    Ok(keyring::Entry::new(SERVICE, name)?)
}

pub fn set(name: &str, value: &str) -> AppResult<()> {
    Ok(entry(name)?.set_password(value)?)
}

pub fn get(name: &str) -> AppResult<Option<String>> {
    match entry(name)?.get_password() {
        Ok(v) => Ok(Some(v)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn exists(name: &str) -> AppResult<bool> {
    Ok(get(name)?.is_some())
}

pub fn delete(name: &str) -> AppResult<()> {
    match entry(name)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Touches the real Windows Credential Manager with a throwaway entry.
    #[test]
    fn roundtrip_in_credential_manager() {
        let name = format!("test-{}", uuid::Uuid::now_v7());
        assert_eq!(get(&name).unwrap(), None);
        set(&name, "s3cret").unwrap();
        assert_eq!(get(&name).unwrap().as_deref(), Some("s3cret"));
        delete(&name).unwrap();
        assert!(!exists(&name).unwrap());
        delete(&name).unwrap(); // deleting twice is fine
    }
}
