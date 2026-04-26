//! Thin wrapper over the OS keychain — every Senda secret is namespaced under
//! the `senda` service. The frontend only ever sees the **id** (e.g.
//! `github:pat:42`); the actual token never leaves Rust.

use keyring::Entry;
use thiserror::Error;

const SERVICE: &str = "senda";

#[derive(Debug, Error)]
pub enum SecretError {
    #[error("keyring error: {0}")]
    Keyring(#[from] keyring::Error),
}

pub fn save(id: &str, value: &str) -> Result<(), SecretError> {
    let entry = Entry::new(SERVICE, id)?;
    entry.set_password(value)?;
    Ok(())
}

#[allow(dead_code)] // used by Phase 5 publish flow
pub fn load(id: &str) -> Result<Option<String>, SecretError> {
    let entry = Entry::new(SERVICE, id)?;
    match entry.get_password() {
        Ok(s) => Ok(Some(s)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(SecretError::Keyring(e)),
    }
}

pub fn delete(id: &str) -> Result<(), SecretError> {
    let entry = Entry::new(SERVICE, id)?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(SecretError::Keyring(e)),
    }
}
