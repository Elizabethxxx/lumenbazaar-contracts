# Policy Wallet Example

This package is example code only. It is not production wallet infrastructure, is not audited, and should not custody funds without a full wallet security design.

The contract demonstrates the shape of a policy-controlled smart account for LumenBazaar agents:

- Stores an owner address that authorizes wallet administration.
- Stores the target `upto-session` contract address used by the surrounding application.
- Exposes `require_owner` as the minimal owner gate used by later spending-policy examples.
- Keeps all state in this contract instance so it remains isolated from `upto-session`.

Spending policies are keyed by agent address and can restrict:

- Maximum amount per payment.
- Maximum total amount per ledger-day bucket.
- Allowed sellers (maximum 16 entries, duplicates rejected).
- Allowed assets (maximum 16 entries, duplicates rejected).
- Allowed resource hashes (maximum 16 entries, duplicates rejected).
- Validity window by ledger sequence.

## Policy Revisions and Concurrency

Policy updates enforce optimistic concurrency control via monotonic revisions:
- Each agent policy tracks a `revision: u32`.
- `set_policy(policy, expected_revision)` requires passing the expected current revision.
- New policies must use `expected_revision = 0` (or `create_policy(policy)`), initializing at revision 1.
- Stale expected revisions fail with `StaleRevision` without modifying policy or daily spend state.
- `get_policy_revision(agent)` returns the current revision.

`check_payment` validates a proposed payment without mutating state. `authorize_payment` requires the agent signature and records the amount against the current ledger-day bucket.

See [docs/policy-examples.md](docs/policy-examples.md) for the executable examples mirrored by the test suite.
