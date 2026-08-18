//! Model mapping and tiered routing.
//!
//! Given an inbound model name and the current quota picture, decide which
//! account and which upstream model should serve the request. Routing sits on
//! top of the relay and on top of quota collection; neither exists yet, so this
//! module currently only fixes the shape of a rule.

use crate::error::{Error, Result};

/// One routing rule, evaluated in order.
#[derive(Debug, Clone)]
pub struct RouteRule {
    /// Inbound model name or glob, e.g. `gpt-4o*`.
    pub match_model: String,
    /// Provider to serve it from.
    pub provider_id: String,
    /// Upstream model name to substitute.
    pub target_model: String,
    /// Skip this rule when the account is above this utilisation, 0.0..=1.0.
    pub max_utilization: Option<f32>,
}

/// Pick the first rule whose model pattern and quota ceiling both match.
pub fn select(_rules: &[RouteRule], _inbound_model: &str) -> Result<RouteRule> {
    Err(Error::NotImplemented("router::select"))
}
