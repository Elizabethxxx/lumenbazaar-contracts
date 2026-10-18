# Policy Wallet Examples

These examples are mirrored by executable tests in `src/test.rs`.

| Scenario | Test name | Expected result |
| --- | --- | --- |
| Allowed payment | `documentation_allowed_payment_example_authorizes` | The agent-authorized payment succeeds and increments daily spend. |
| Blocked payment | `documentation_blocked_payment_example_reports_amount_cap` | A payment over the per-payment cap returns `AmountExceedsPaymentCap`. |
| Expired policy | `documentation_expired_policy_example_reports_expired` | A payment checked after `valid_until_ledger` returns `PolicyExpired`. |
| Wrong seller | `documentation_wrong_seller_example_reports_seller_error` | A seller outside the allowlist returns `SellerNotAllowed`. |
| Wrong asset | `documentation_wrong_asset_example_reports_asset_error` | An asset outside the allowlist returns `AssetNotAllowed`. |
| Stale revision write | `stale_expected_revision_fails_without_modifying_policy_or_spend` | Passing an outdated expected revision returns `StaleRevision` without modifying policy or daily spend. |
| Oversized allowlist | `allowlists_enforce_exact_limits_and_fail_at_limit_plus_one` | Allowlist exceeding 16 entries returns `InvalidPolicy`. |
| Duplicate allowlist entries | `allowlists_reject_duplicate_sellers_deterministically` | Duplicate seller, asset, or resource hash returns `InvalidPolicy`. |
