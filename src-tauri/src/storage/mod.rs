//! Credential and application-state storage.
//!
//! Two rules govern everything in this module:
//!
//! 1. A secret never leaves this module in plaintext except to the adapter that
//!    is about to write it into its tool's own config.
//! 2. A secret is never logged, never serialised into diagnostics, and never
//!    included in an error message.
//!
//! See `docs/SECURITY_MODEL.md`.

use crate::error::Result;

pub mod encrypted_file;
pub mod keychain;

/// Opaque handle to a stored secret.
///
/// The application passes handles around; only [`CredentialStore`] can turn one
/// back into secret material.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SecretRef(pub String);

/// Secret material, in memory, for as short a time as possible.
///
/// Deliberately does not implement `Debug`, `Display`, or `Serialize` so it
/// cannot be logged or accidentally serialised.
pub struct Secret(Vec<u8>);

impl Secret {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    pub fn expose(&self) -> &[u8] {
        &self.0
    }
}

impl Drop for Secret {
    fn drop(&mut self) {
        // Best-effort zeroing. Replace with the `zeroize` crate once the
        // dependency set is finalised (see docs/adr/0003).
        self.0.iter_mut().for_each(|byte| *byte = 0);
    }
}

/// Backend-agnostic secret storage.
pub trait CredentialStore: Send + Sync {
    /// Short identifier used in settings and diagnostics, e.g. `keychain`.
    fn id(&self) -> &'static str;

    /// Whether this backend can be used on this machine right now.
    fn is_available(&self) -> bool;

    fn put(&self, key: &SecretRef, secret: &Secret) -> Result<()>;
    fn get(&self, key: &SecretRef) -> Result<Option<Secret>>;
    fn delete(&self, key: &SecretRef) -> Result<()>;
}

/// Select the best available backend: OS keychain first, encrypted file second.
pub fn default_store() -> Box<dyn CredentialStore> {
    let keychain = keychain::KeychainStore;
    if keychain.is_available() {
        return Box::new(keychain);
    }
    Box::new(encrypted_file::EncryptedFileStore)
}
