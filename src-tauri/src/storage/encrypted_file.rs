//! Encrypted-file fallback store.
//!
//! Used only when no OS credential service is available. The file lives in the
//! platform data directory, is written with `0600` permissions on Unix, and is
//! encrypted with a key derived from a user passphrase.
//!
//! An unencrypted fallback is explicitly **not** offered; if neither a keychain
//! nor a passphrase is available, the application refuses to store secrets and
//! says so, rather than degrading silently.

use super::{CredentialStore, Secret, SecretRef};
use crate::error::{Error, Result};

#[derive(Debug, Default)]
pub struct EncryptedFileStore;

impl CredentialStore for EncryptedFileStore {
    fn id(&self) -> &'static str {
        "encrypted-file"
    }

    fn is_available(&self) -> bool {
        // TODO(M1): available once a passphrase has been established for this
        // installation.
        false
    }

    fn put(&self, _key: &SecretRef, _secret: &Secret) -> Result<()> {
        Err(Error::NotImplemented("encrypted-file::put"))
    }

    fn get(&self, _key: &SecretRef) -> Result<Option<Secret>> {
        Err(Error::NotImplemented("encrypted-file::get"))
    }

    fn delete(&self, _key: &SecretRef) -> Result<()> {
        Err(Error::NotImplemented("encrypted-file::delete"))
    }
}
