//! Model mapping and tiered routing.
//!
//! Given an inbound model name and the current quota picture, decide which
//! provider and which upstream model should serve the request. [`select`] is a
//! pure function: no I/O, no network, no filesystem. Failover on a rate-limit
//! response (`FR-7`) needs a live relay and is not here.

use std::collections::HashMap;
use std::io;

use crate::error::{Error, Result};

/// One routing rule, evaluated in order.
#[derive(Debug, Clone)]
pub struct RouteRule {
    /// Inbound model name or glob, e.g. `gpt-4o*`.
    ///
    /// Syntax is documented on [`select`]: an exact name, or a single trailing
    /// `*`. Matching is case-sensitive.
    pub match_model: String,
    /// Provider to serve it from.
    pub provider_id: String,
    /// Upstream model name to substitute.
    pub target_model: String,
    /// Skip this rule when the account is above this utilisation, 0.0..=1.0.
    pub max_utilization: Option<f32>,
}

/// Pick the first rule whose model pattern matches `inbound_model` and whose
/// `max_utilization` gate is satisfied.
///
/// Rules are evaluated in slice order. The first eligible rule wins; there is
/// no "best" or "most specific" match. If no rule is eligible the request
/// fails — there is no default rule and no implicit fallback
/// (`docs/ARCHITECTURE.md` §6). Silently spending the wrong account's quota is
/// worse than an error.
///
/// # Model pattern syntax
///
/// `RouteRule.match_model` is either an exact inbound model name or a prefix
/// glob. This is the whole language; the UI should describe it as such.
///
/// - `gpt-4o` matches only `gpt-4o`.
/// - `gpt-4o*` matches `gpt-4o` and any name that starts with `gpt-4o`
///   (`gpt-4o-mini`, `gpt-4o-2024-05-13`, …). This is the documented example.
/// - `*` matches every inbound name.
///
/// Matching is **case-sensitive**: `GPT-4o` does not match `gpt-4o`. A `*`
/// that is not the final character is taken literally, not as a wildcard.
/// There is no `?`, no character class (`[abc]`), and no escape sequence.
///
/// # Utilisation gate
///
/// `utilization` is the current window-consumed fraction (`0.0..=1.0`) of the
/// account each provider would spend, keyed by [`RouteRule::provider_id`]. A
/// missing key means utilisation is **unknown**, not zero.
///
/// The picture is keyed by provider, not by rule, because a `RouteRule` names
/// a provider and an upstream model, not an account. The caller collapses the
/// quota snapshot of the account that provider would spend into this map; two
/// rules targeting the same provider share one number.
///
/// The three cases are kept distinct:
///
/// - No ceiling: the gate does not apply.
/// - Ceiling set and utilisation known: skip the rule when utilisation is
///   strictly above the ceiling. Equal to the ceiling still matches.
/// - Ceiling set and utilisation unknown: the gate is satisfied. Unknown is
///   not a number, so it is not compared. Treating a missing signal as
///   "already too high" would fabricate a utilisation (`NFR-8`) and would
///   make every ceilinged rule dead for providers that publish no quota
///   signal — the normal case until quota collection exists, and a permanent
///   case for several adapters. §6 refuses an *unmatched* request falling
///   through to an arbitrary account; it does not refuse a user-authored rule
///   because telemetry is missing. Spending the account the user ordered
///   when the optional gate cannot be evaluated is following the list, not
///   inventing a fallback.
pub fn select(
    rules: &[RouteRule],
    inbound_model: &str,
    utilization: &HashMap<&str, f32>,
) -> Result<RouteRule> {
    rules
        .iter()
        .find(|rule| {
            model_matches(&rule.match_model, inbound_model) && within_ceiling(rule, utilization)
        })
        .cloned()
        .ok_or_else(|| no_matching_rule(inbound_model))
}

fn model_matches(pattern: &str, inbound: &str) -> bool {
    match pattern.strip_suffix('*') {
        Some(prefix) if !prefix.contains('*') => inbound.starts_with(prefix),
        _ => pattern == inbound,
    }
}

fn within_ceiling(rule: &RouteRule, utilization: &HashMap<&str, f32>) -> bool {
    let Some(max) = rule.max_utilization else {
        return true;
    };
    match utilization.get(rule.provider_id.as_str()) {
        Some(&current) => current <= max,
        None => true,
    }
}

/// No rule was eligible. Names the inbound model and nothing else: no account
/// id, no credential, no provider list (`NFR-1`).
///
/// `Error` has no routing variant; `Io`/`NotFound` is the closest existing
/// bucket that can carry the inbound model. A dedicated `NoMatchingRoute`
/// variant would be the right taxonomy.
fn no_matching_rule(inbound_model: &str) -> Error {
    Error::Io(io::Error::new(
        io::ErrorKind::NotFound,
        format!("no routing rule matches inbound model `{inbound_model}`"),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(match_model: &str, provider_id: &str) -> RouteRule {
        RouteRule {
            match_model: match_model.to_owned(),
            provider_id: provider_id.to_owned(),
            target_model: format!("{provider_id}-upstream"),
            max_utilization: None,
        }
    }

    fn assert_no_match(result: Result<RouteRule>, inbound: &str) {
        let err = result.expect_err("must not pick a rule");
        let text = err.to_string();
        assert!(
            text.contains(inbound),
            "error must name the inbound model, got: {text}"
        );
    }

    #[test]
    fn exact_name_matches_only_that_name() {
        // An un-globbed pattern is a precise binding: `gpt-4o` must not
        // quietly absorb `gpt-4o-mini` and spend a different account's quota.
        let rules = [rule("gpt-4o", "openai")];
        let none = HashMap::new();

        let got = select(&rules, "gpt-4o", &none).expect("exact match");
        assert_eq!(got.provider_id, "openai");
        assert_eq!(got.target_model, "openai-upstream");

        assert_no_match(select(&rules, "gpt-4o-mini", &none), "gpt-4o-mini");
    }

    #[test]
    fn documented_prefix_glob_matches_the_family() {
        // `gpt-4o*` is the example in the `RouteRule` field docs and the
        // syntax the UI will describe. The prefix itself and any longer name
        // must match; a different family must not.
        let rules = [rule("gpt-4o*", "openai")];
        let none = HashMap::new();

        assert_eq!(
            select(&rules, "gpt-4o", &none)
                .expect("prefix itself")
                .provider_id,
            "openai"
        );
        assert_eq!(
            select(&rules, "gpt-4o-mini", &none)
                .expect("longer name")
                .provider_id,
            "openai"
        );
        assert_no_match(select(&rules, "gpt-5", &none), "gpt-5");
    }

    #[test]
    fn first_matching_rule_wins_even_when_a_later_rule_is_more_specific() {
        // Evaluation is ordered, not scored. A later exact rule must not
        // steal a request the earlier glob already claimed, or the list the
        // user authored is not the list that runs.
        let rules = [rule("gpt-4o*", "openai"), rule("gpt-4o", "anthropic")];

        let got = select(&rules, "gpt-4o", &HashMap::new()).expect("first match");
        assert_eq!(got.provider_id, "openai");
    }

    #[test]
    fn a_rule_over_its_ceiling_falls_through_to_the_next_match() {
        // The gate is a skip, not a failure: an exhausted preferred account
        // must yield to the next rule that still matches, not abort the
        // request.
        let mut preferred = rule("gpt-4o", "openai");
        preferred.max_utilization = Some(0.5);
        let rules = [preferred, rule("gpt-4o", "anthropic")];
        let utilization = HashMap::from([("openai", 0.8)]);

        let got = select(&rules, "gpt-4o", &utilization).expect("fallthrough");
        assert_eq!(got.provider_id, "anthropic");
    }

    #[test]
    fn unknown_utilization_does_not_skip_a_ceilinged_rule() {
        // Unknown is not a number (`NFR-8`). Fabricating "too high" would
        // make every ceilinged rule dead for providers that publish no
        // signal, which is the normal case until quota collection exists.
        let mut gated = rule("gpt-4o", "openai");
        gated.max_utilization = Some(0.5);
        let fallback = rule("gpt-4o", "anthropic");
        let rules = [gated, fallback];

        let got = select(&rules, "gpt-4o", &HashMap::new()).expect("unknown satisfies");
        assert_eq!(got.provider_id, "openai");
    }

    #[test]
    fn no_matching_rule_is_an_error_not_an_arbitrary_account() {
        // §6: silently spending the wrong account's quota is worse than an
        // error. A later, unrelated rule must not be promoted into a default.
        let rules = [rule("claude-*", "anthropic"), rule("gemini-*", "google")];

        assert_no_match(select(&rules, "gpt-4o", &HashMap::new()), "gpt-4o");
    }

    #[test]
    fn an_empty_rule_list_is_the_same_error_not_a_panic() {
        // An empty list is "nothing matched", not a programming error. The
        // relay will start with no rules configured; that must be a clean
        // failure naming the inbound model.
        assert_no_match(select(&[], "gpt-4o", &HashMap::new()), "gpt-4o");
    }

    #[test]
    fn matching_is_case_sensitive() {
        // Vendor model ids are case-sensitive strings. Folding case would
        // make `GPT-4o` spend the `gpt-4o` rule, which is a different name.
        let rules = [rule("gpt-4o", "openai")];
        assert_no_match(select(&rules, "GPT-4o", &HashMap::new()), "GPT-4o");
    }

    #[test]
    fn a_rule_with_no_ceiling_ignores_known_utilization() {
        // The gate is optional (`FR-7`). A missing ceiling must not be read
        // as zero, or every rule would be skipped the moment any utilisation
        // is known.
        let rules = [rule("gpt-4o", "openai")];
        let utilization = HashMap::from([("openai", 0.99)]);

        let got = select(&rules, "gpt-4o", &utilization).expect("no ceiling");
        assert_eq!(got.provider_id, "openai");
    }

    #[test]
    fn a_rule_at_its_ceiling_still_matches() {
        // "Above" is strict: equal to the ceiling is still within it. A
        // ceiling of 1.0 would otherwise refuse a fully consumed window.
        let mut gated = rule("gpt-4o", "openai");
        gated.max_utilization = Some(0.8);
        let utilization = HashMap::from([("openai", 0.8)]);

        let got = select(&[gated], "gpt-4o", &utilization).expect("at ceiling");
        assert_eq!(got.provider_id, "openai");
    }

    #[test]
    fn a_star_that_is_not_final_is_literal() {
        // The UI can describe the syntax without lying: there is no hidden
        // mid-string wildcard.
        let rules = [rule("gpt-*-mini", "openai")];
        let none = HashMap::new();

        assert_no_match(select(&rules, "gpt-4o-mini", &none), "gpt-4o-mini");
        assert_eq!(
            select(&rules, "gpt-*-mini", &none)
                .expect("literal star")
                .provider_id,
            "openai"
        );
    }
}
