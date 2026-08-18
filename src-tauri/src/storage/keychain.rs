//! OS-native credential storage.
//!
//! Target backends: macOS Keychain, Windows Credential Manager, and the
//! Freedesktop Secret Service (libsecret / gnome-keyring / KWallet) on Linux.
//! This is the preferred store; the encrypted-file store exists only for hosts
//! where no Secret Service is running — headless Linux, for example.

use super::{CredentialStore, Secret, SecretRef};
use crate::error::{Error, Result};

#[derive(Debug, Default)]
pub struct KeychainStore;

impl CredentialStore for KeychainStore {
    fn id(&self) -> &'static str {
        "keychain"
    }

    fn is_available(&self) -> bool {
        // TODO(M1): probe the platform credential service. Returning `false`
        // keeps the fallback selected until this is implemented, which is the
        // safe direction: an unimplemented keychain must never silently
        // succeed.
        false
    }

    fn put(&self, _key: &SecretRef, _secret: &Secret) -> Result<()> {
        Err(Error::NotImplemented("keychain::put"))
    }

    fn get(&self, _key: &SecretRef) -> Result<Option<Secret>> {
        Err(Error::NotImplemented("keychain::get"))
    }

    fn delete(&self, _key: &SecretRef) -> Result<()> {
        Err(Error::NotImplemented("keychain::delete"))
    }
}
