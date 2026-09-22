use nyrva_core::{parse_statusline, DataStatus, Provider};
use serde_json::json;
const NOW: u64 = 1_800_000_000_000;
fn parse(value: serde_json::Value) -> nyrva_core::Snapshot {
    parse_statusline(Provider::Antigravity, "personal", &value.to_string(), NOW).unwrap()
}

#[test]
fn antigravity_reports_independent_buckets_and_relative_reset() {
    let s = parse(
        json!({"quota":{"gemini-weekly":{"remaining_fraction":0.75,"reset_in_seconds":3600},"claude-5h":{"remaining_fraction":0.2}}}),
    );
    assert_eq!(s.status, DataStatus::Live);
    assert_eq!(s.buckets.len(), 2);
    let b = s.buckets.iter().find(|b| b.id == "gemini-weekly").unwrap();
    assert_eq!(b.remaining_fraction, Some(0.75));
    assert_eq!(b.resets_at_ms, Some(NOW + 3_600_000));
}
#[test]
fn zero_is_real_exhaustion_not_missing_data() {
    let s = parse(json!({"quota":{"weekly":{"remaining_fraction":0.0}}}));
    assert_eq!(s.buckets.len(), 1);
    assert_eq!(s.buckets[0].remaining_fraction, Some(0.0));
    assert_eq!(s.status, DataStatus::Live);
}
#[test]
fn missing_quota_is_not_zero_usage() {
    let s = parse(json!({"agent_state":"idle"}));
    assert_eq!(s.status, DataStatus::Unavailable);
    assert!(s.buckets.is_empty());
}
#[test]
fn invalid_fraction_is_rejected_not_clamped() {
    let s = parse(
        json!({"quota":{"bad":{"remaining_fraction":1.5},"negative":{"remaining_fraction":-0.2},"good":{"remaining_fraction":1.0}}}),
    );
    assert_eq!(s.buckets.len(), 1);
    assert_eq!(s.buckets[0].id, "good");
}
#[test]
fn absolute_reset_takes_precedence_and_elapsed_reset_is_not_replenished() {
    let s = parse(
        json!({"quota":{"weekly":{"remaining_fraction":0.15,"reset_time":"2027-01-15T08:00:00Z","reset_in_seconds":42}}}),
    );
    assert_eq!(s.buckets[0].resets_at_ms, Some(NOW));
    assert_eq!(s.buckets[0].remaining_fraction, Some(0.15));
    assert_eq!(s.effective_status(NOW + 700_000), DataStatus::Stale);
}
#[test]
fn privacy_allowlist_discards_credentials_prompts_email_and_absolute_paths() {
    let s = parse(
        json!({"quota":{"weekly":{"remaining_fraction":0.5}},"model":{"id":"gemini"},"context_window":{"used_percentage":42.0,"context_window_size":1000000},"email":"PRIVATE@example.com","access_token":"TOP_SECRET","prompt":"CONFIDENTIAL","workspace":{"current_dir":"C:\\private\\project"}}),
    );
    let text = serde_json::to_string(&s).unwrap();
    for forbidden in [
        "PRIVATE",
        "TOP_SECRET",
        "CONFIDENTIAL",
        "private",
        "current_dir",
    ] {
        assert!(!text.contains(forbidden), "leaked {forbidden}");
    }
    assert_eq!(s.context.unwrap().used_percentage, Some(42.0));
    assert_eq!(s.model.as_deref(), Some("gemini"));
}
#[test]
fn malformed_oversized_and_unsafe_account_inputs_fail_explicitly() {
    assert!(parse_statusline(Provider::Antigravity, "personal", "not-json", NOW).is_err());
    assert!(parse_statusline(Provider::Antigravity, "../secret", "{}", NOW).is_err());
    assert!(parse_statusline(Provider::Antigravity, "personal", &" ".repeat(262145), NOW).is_err());
}
#[test]
fn control_characters_cannot_reach_terminal_labels() {
    let s = parse(
        json!({"quota":{"weekly":{"remaining_fraction":0.5}},"model":{"id":"safe\u{001b}[31m\ntext"}}),
    );
    assert!(!s.model.unwrap().chars().any(char::is_control));
}
