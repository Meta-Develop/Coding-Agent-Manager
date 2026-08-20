//! Public M6 router-core contract tests. No network or vendor process is used.

use coding_agent_manager_lib::model::{
    ProviderQuotaList, QuotaListError, QuotaListErrorKind, QuotaListOutcome, QuotaSnapshot,
    QuotaSource, RouteRule,
};
use coding_agent_manager_lib::router::{
    validate_rules, RouteAccount, RouteError, RouteRuleField, RouterState,
};
use time::format_description::well_known::Rfc3339;
use time::{Duration, OffsetDateTime};

fn timestamp(value: &str) -> OffsetDateTime {
    OffsetDateTime::parse(value, &Rfc3339).expect("test timestamp")
}

fn now() -> OffsetDateTime {
    timestamp("2030-01-01T00:00:00Z")
}

fn rule(pattern: &str, provider: &str, target: &str) -> RouteRule {
    RouteRule {
        match_model: pattern.to_owned(),
        provider_id: provider.to_owned(),
        target_model: target.to_owned(),
        max_utilization: None,
    }
}

fn gated_rule(pattern: &str, provider: &str, target: &str, ceiling: f32) -> RouteRule {
    RouteRule {
        max_utilization: Some(ceiling),
        ..rule(pattern, provider, target)
    }
}

fn account(provider: &str, account: &str) -> RouteAccount {
    RouteAccount {
        provider_id: provider.to_owned(),
        account_id: account.to_owned(),
    }
}

fn snapshot(
    account: &str,
    model: Option<&str>,
    utilization: f32,
    resets_at: Option<&str>,
) -> QuotaSnapshot {
    QuotaSnapshot {
        account_id: account.to_owned(),
        model: model.map(str::to_owned),
        utilization,
        window_label: Some("FAKE-window".to_owned()),
        resets_at: resets_at.map(str::to_owned),
        captured_at: "2029-12-31T23:59:00Z".to_owned(),
        source: QuotaSource::Header,
    }
}

fn available(provider: &str, snapshots: Vec<QuotaSnapshot>) -> ProviderQuotaList {
    ProviderQuotaList {
        provider_id: provider.to_owned(),
        plan_label: None,
        snapshots,
        outcome: QuotaListOutcome::Available,
    }
}

fn no_signal(provider: &str) -> ProviderQuotaList {
    ProviderQuotaList {
        provider_id: provider.to_owned(),
        plan_label: None,
        snapshots: Vec::new(),
        outcome: QuotaListOutcome::NoSignal,
    }
}

fn failed(provider: &str) -> ProviderQuotaList {
    ProviderQuotaList {
        provider_id: provider.to_owned(),
        plan_label: None,
        snapshots: Vec::new(),
        outcome: QuotaListOutcome::Failed {
            error: QuotaListError {
                kind: QuotaListErrorKind::Other,
                path: None,
                message: "FAKE-quota-collection-failed".to_owned(),
            },
        },
    }
}

#[test]
fn route_rule_wire_contract_is_camel_case_and_closed() {
    let value = serde_json::to_value(gated_rule("model-*", "provider-a", "upstream-a", 0.5))
        .expect("serialize rule");
    assert_eq!(
        value,
        serde_json::json!({
            "matchModel": "model-*",
            "providerId": "provider-a",
            "targetModel": "upstream-a",
            "maxUtilization": 0.5
        })
    );

    let unknown = serde_json::json!({
        "matchModel": "model-*",
        "providerId": "provider-a",
        "targetModel": "upstream-a",
        "maxUtilization": null,
        "accountId": "FAKE-account-must-not-enter-rule"
    });
    assert!(serde_json::from_value::<RouteRule>(unknown).is_err());
}

#[test]
fn ordered_exact_and_prefix_matching_are_case_sensitive() {
    let rules = vec![
        rule("model-*", "provider-a", "upstream-a"),
        rule("model-pro", "provider-b", "upstream-b"),
    ];
    let accounts = vec![
        account("provider-a", "account-a"),
        account("provider-b", "account-b"),
    ];
    let mut router = RouterState::default();

    let selection = router
        .select_next(&rules, &accounts, &[], "model-pro", 0, now())
        .expect("ordered prefix match");
    assert_eq!(selection.rule_index, 0);
    assert_eq!(selection.provider_id, "provider-a");
    assert_eq!(selection.account_id, "account-a");
    assert_eq!(selection.target_model, "upstream-a");

    assert_eq!(
        router.select_next(&rules, &accounts, &[], "MODEL-pro", 0, now()),
        Err(RouteError::UnmatchedModel)
    );

    let exact = vec![rule("model-pro", "provider-b", "upstream-b")];
    assert_eq!(
        router.select_next(&exact, &accounts, &[], "model-pro-plus", 0, now()),
        Err(RouteError::UnmatchedModel)
    );
}

#[test]
fn validation_rejects_bad_fields_without_echoing_them() {
    let invalid_patterns = ["", "model-*suffix", "model-**"];
    for pattern in invalid_patterns {
        let rules = [rule(pattern, "provider-a", "upstream-a")];
        assert_eq!(
            validate_rules(&rules),
            Err(RouteError::InvalidRule {
                rule_index: 0,
                field: RouteRuleField::MatchModel,
            })
        );
    }

    let bad_provider = [rule("model", "   ", "upstream")];
    assert!(matches!(
        validate_rules(&bad_provider),
        Err(RouteError::InvalidRule {
            field: RouteRuleField::ProviderId,
            ..
        })
    ));
    let bad_target = [rule("model", "provider", "   ")];
    assert!(matches!(
        validate_rules(&bad_target),
        Err(RouteError::InvalidRule {
            field: RouteRuleField::TargetModel,
            ..
        })
    ));
    for ceiling in [-0.01, 1.01, f32::INFINITY, f32::NAN] {
        let rules = [gated_rule("model", "provider", "upstream", ceiling)];
        assert!(matches!(
            validate_rules(&rules),
            Err(RouteError::InvalidRule {
                field: RouteRuleField::MaxUtilization,
                ..
            })
        ));
    }

    let submitted = "FAKE-secret-shaped-submitted-value";
    let error = validate_rules(&[rule(submitted, "provider", "")]).expect_err("invalid target");
    assert!(!error.to_string().contains(submitted));
}

#[test]
fn quota_uses_the_conservative_maximum_and_allows_equality() {
    let rules = vec![
        gated_rule("model", "provider-a", "upstream-a", 0.6),
        rule("model", "provider-b", "upstream-b"),
    ];
    let accounts = vec![
        account("provider-a", "account-a"),
        account("provider-b", "account-b"),
    ];
    let at_ceiling = vec![available(
        "provider-a",
        vec![
            snapshot("account-a", Some("upstream-a"), 0.4, None),
            snapshot("account-a", None, 0.6, None),
        ],
    )];
    let mut router = RouterState::default();
    assert_eq!(
        router
            .select_next(&rules, &accounts, &at_ceiling, "model", 0, now())
            .expect("equal utilization remains eligible")
            .provider_id,
        "provider-a"
    );

    let above = vec![available(
        "provider-a",
        vec![
            snapshot("account-a", Some("upstream-a"), 0.7, None),
            snapshot("account-a", None, 0.6, None),
        ],
    )];
    assert_eq!(
        router
            .select_next(&rules, &accounts, &above, "model", 0, now())
            .expect("above-ceiling route falls through")
            .provider_id,
        "provider-b"
    );
}

#[test]
fn every_missing_or_unusable_quota_case_makes_a_gated_rule_ungateable() {
    let rules = vec![
        gated_rule("model", "provider-a", "upstream-a", 0.8),
        rule("model", "provider-b", "upstream-b"),
    ];
    let accounts = vec![
        account("provider-a", "account-a"),
        account("provider-b", "account-b"),
    ];
    let mut invalid = snapshot("account-a", Some("upstream-a"), 0.2, None);
    invalid.captured_at = "not-a-timestamp".to_owned();
    let cases = vec![
        Vec::new(),
        vec![no_signal("provider-a")],
        vec![failed("provider-a")],
        vec![available(
            "provider-a",
            vec![snapshot("wrong-account", Some("upstream-a"), 0.2, None)],
        )],
        vec![available(
            "provider-a",
            vec![snapshot("account-a", Some("wrong-model"), 0.2, None)],
        )],
        vec![available("provider-a", Vec::new())],
        vec![available("provider-a", vec![invalid])],
    ];

    for quotas in cases {
        let selection = RouterState::default()
            .select_next(&rules, &accounts, &quotas, "model", 0, now())
            .expect("ungateable preferred rule must skip");
        assert_eq!(selection.provider_id, "provider-b");
    }
}

#[test]
fn ungated_rule_is_eligible_without_a_quota_signal() {
    let rules = [rule("model", "provider-a", "upstream-a")];
    let accounts = [account("provider-a", "account-a")];
    let selection = RouterState::default()
        .select_next(
            &rules,
            &accounts,
            &[no_signal("provider-a")],
            "model",
            0,
            now(),
        )
        .expect("ungated rule");
    assert_eq!(selection.account_id, "account-a");
    assert_eq!(selection.quota_resets_at, None);
}

#[test]
fn selection_returns_the_latest_applicable_future_reset() {
    let rules = [rule("model", "provider-a", "upstream-a")];
    let accounts = [account("provider-a", "account-a")];
    let quotas = [available(
        "provider-a",
        vec![
            snapshot(
                "account-a",
                Some("upstream-a"),
                0.2,
                Some("2030-01-01T00:10:00Z"),
            ),
            snapshot("account-a", None, 0.3, Some("2030-01-01T01:00:00Z")),
            snapshot(
                "account-a",
                Some("wrong-model"),
                0.9,
                Some("2030-01-02T00:00:00Z"),
            ),
            snapshot(
                "account-a",
                Some("upstream-a"),
                0.1,
                Some("2029-12-31T23:00:00Z"),
            ),
        ],
    )];
    let selection = RouterState::default()
        .select_next(&rules, &accounts, &quotas, "model", 0, now())
        .expect("selection");
    assert_eq!(
        selection.quota_resets_at,
        Some(timestamp("2030-01-01T01:00:00Z"))
    );
}

#[test]
fn unmatched_and_matching_but_ineligible_are_distinct() {
    let rules = [gated_rule("model-a", "provider-a", "upstream-a", 0.8)];
    let accounts = [account("provider-a", "account-a")];
    let mut router = RouterState::default();

    assert_eq!(
        router.select_next(&[], &accounts, &[], "model-a", 0, now()),
        Err(RouteError::UnmatchedModel)
    );
    assert_eq!(
        router.select_next(&rules, &accounts, &[], "model-b", 0, now()),
        Err(RouteError::UnmatchedModel)
    );
    assert_eq!(
        router.select_next(&rules, &accounts, &[], "model-a", 0, now()),
        Err(RouteError::NoEligibleRoute)
    );
}

#[test]
fn missing_or_duplicate_accounts_are_errors_not_fallback() {
    let rules = vec![
        rule("model", "provider-a", "upstream-a"),
        rule("model", "provider-b", "upstream-b"),
    ];
    let fallback_only = [account("provider-b", "account-b")];
    assert_eq!(
        RouterState::default().select_next(&rules, &fallback_only, &[], "model", 0, now()),
        Err(RouteError::MissingRouteAccount { rule_index: 0 })
    );

    let duplicate = [
        account("provider-a", "account-a"),
        account("provider-a", "account-a-duplicate"),
        account("provider-b", "account-b"),
    ];
    assert_eq!(
        RouterState::default().select_next(&rules, &duplicate, &[], "model", 0, now()),
        Err(RouteError::DuplicateRouteAccounts { rule_index: 0 })
    );
}

#[test]
fn throttle_skips_until_expiry_and_same_account_later_rules_stay_skipped() {
    let rules = vec![
        rule("model", "provider-a", "upstream-a-first"),
        rule("model", "provider-a", "upstream-a-second"),
        rule("model", "provider-b", "upstream-b"),
    ];
    let accounts = vec![
        account("provider-a", "account-a"),
        account("provider-b", "account-b"),
    ];
    let mut router = RouterState::default();
    let first = router
        .select_next(&rules, &accounts, &[], "model", 0, now())
        .expect("first account");
    let deadline = now() + Duration::minutes(10);
    router.record_rate_limit(&first, deadline);

    let failover = router
        .select_next(&rules, &accounts, &[], "model", first.rule_index + 1, now())
        .expect("same account skipped, next account selected");
    assert_eq!(failover.rule_index, 2);
    assert_eq!(failover.account_id, "account-b");

    let after_expiry = router
        .select_next(
            &rules,
            &accounts,
            &[],
            "model",
            0,
            deadline + Duration::seconds(1),
        )
        .expect("expired account eligible again");
    assert_eq!(after_expiry.rule_index, 0);
    assert_eq!(after_expiry.account_id, "account-a");
}

#[test]
fn rate_limit_deadline_updates_monotonically() {
    let rules = [rule("model", "provider-a", "upstream-a")];
    let accounts = [account("provider-a", "account-a")];
    let mut router = RouterState::default();
    let selection = router
        .select_next(&rules, &accounts, &[], "model", 0, now())
        .expect("selection");
    let later = now() + Duration::hours(1);
    let earlier = now() + Duration::minutes(5);

    router.record_rate_limit(&selection, later);
    router.record_rate_limit(&selection, earlier);
    assert_eq!(
        router.throttle_until("provider-a", "account-a"),
        Some(later)
    );
}
