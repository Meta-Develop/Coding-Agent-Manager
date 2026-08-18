//! Local API relay.
//!
//! Accepts requests in one vendor's wire format and forwards them to a backing
//! account that may speak another. The three formats in scope for v1 are the
//! OpenAI Chat Completions / Responses shape, the Anthropic Messages shape, and
//! the Gemini `generateContent` shape.
//!
//! Security posture: the listener binds to loopback and nothing else unless the
//! user explicitly opts in, and an opt-in requires an authentication token.
//! See `docs/SECURITY_MODEL.md`.

use crate::error::{Error, Result};

/// Wire formats the relay can accept and emit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireFormat {
    OpenAiChatCompletions,
    OpenAiResponses,
    AnthropicMessages,
    GeminiGenerateContent,
}

/// Runtime configuration for the relay listener.
#[derive(Debug, Clone)]
pub struct RelayConfig {
    /// Defaults to `127.0.0.1`. Changing it requires an explicit opt-in.
    pub bind_address: String,
    pub port: u16,
    /// Required whenever `bind_address` is not a loopback address.
    pub auth_token: Option<String>,
}

impl Default for RelayConfig {
    fn default() -> Self {
        Self {
            bind_address: "127.0.0.1".to_string(),
            port: 8787,
            auth_token: None,
        }
    }
}

impl RelayConfig {
    /// Reject a configuration that would expose an unauthenticated listener.
    pub fn validate(&self) -> Result<()> {
        let is_loopback = self.bind_address == "127.0.0.1" || self.bind_address == "::1";
        if !is_loopback && self.auth_token.is_none() {
            return Err(Error::CredentialStoreUnavailable(
                "a non-loopback relay binding requires an auth token".to_string(),
            ));
        }
        Ok(())
    }
}

/// Translate a request body between wire formats.
pub fn translate(_from: WireFormat, _to: WireFormat, _body: &[u8]) -> Result<Vec<u8>> {
    Err(Error::NotImplemented("relay::translate"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_binding_needs_no_token() {
        assert!(RelayConfig::default().validate().is_ok());
    }

    #[test]
    fn exposed_binding_without_token_is_rejected() {
        let config = RelayConfig {
            bind_address: "0.0.0.0".to_string(),
            ..RelayConfig::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn exposed_binding_with_token_is_allowed() {
        let config = RelayConfig {
            bind_address: "0.0.0.0".to_string(),
            auth_token: Some("token".to_string()),
            ..RelayConfig::default()
        };
        assert!(config.validate().is_ok());
    }
}
