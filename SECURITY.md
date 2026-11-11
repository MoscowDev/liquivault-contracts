# Security policy and threat model

**Pre-audit, testnet only.** No independent audit has certified LiquiVault. Tests and internal review do not establish safety for real funds.

## Report a vulnerability

Use the affected repository's GitHub **Security -> Report a vulnerability** if private reporting is enabled. If unavailable, request a private reporting channel through a minimal issue containing no exploit details, keys or sensitive data. Do not publish a working exploit in a public issue. Maintainers should enable private reporting before inviting security submissions; no response-time guarantee is asserted here.

## Roles

| Role | Authority and assumption |
| --- | --- |
| Registry verifier/admin | Verify invoices and set risk tiers; trusted to inspect and deduplicate real documents |
| Contract admin | Pause, change supported configuration, transfer authority, upgrade compatible WASM; escrow admin also executes overdue defaults |
| Merchant | Mint and factor owned eligible invoices; untrusted |
| Payer | Authorize transfer of repayment tokens; need not be named debtor |
| LP | Deposit/withdraw under share, time, coverage and liquidity constraints; untrusted |
| Configured escrow | Registry status transitions and pool allocation/repayment/write-off; cross-contract auth required |

Admin upgrades create a trust boundary: malicious replacement code can change protections. Configuration changes can also affect future calls. There is no timelock, governance voting or audited migration engine.

## Implemented protections

- Verification is restricted to the configured verifier/admin; naming a debtor does not grant that role.
- A factored invoice cannot be factored again through that record.
- Registry default transitions are escrow-only, preserving coordinated pool loss accounting.
- Junior yield is capped by capital share and the configured maximum.
- Both tranches enforce a 24-hour post-deposit withdrawal delay.
- Wiped-out tranches reject deposits that would dilute new capital with stale shares.
- Junior withdrawals retain at least 10% coverage of deployed capital.
- Escrow pause blocks new factoring but allows repayment and overdue default accounting. Pool pause blocks deposits, withdrawals and allocation while allowing repayments/write-offs. Registry pause blocks its configured creation/verification/transfer paths; escrow lifecycle completion remains possible.
- Financial arithmetic uses checked operations; a failed cross-contract operation reverts the transaction.

These are implemented behaviors with regression tests, not formal proofs.

## Remaining risks

Off-chain fraud, duplicate real-world receivables, admin/verifier compromise, delayed defaults, residual withdrawal timing incentives, illiquidity, loss beyond junior reserves, stale or archived state, client configuration errors and unaudited dependencies remain relevant. The on-chain ownership field does not enforce a legal assignment. No KYC/KYB or SEP-24 integration is supplied.

Before real-fund deployment: obtain independent contract/client review, address the documented economic limits, design operational custody/monitoring and recovery, validate storage-compatible upgrades, and review applicable commercial processes. See [known limitations](https://github.com/MoscowDev/LiquiVault/blob/main/docs/KNOWN_LIMITATIONS.md) and [evidence](https://github.com/MoscowDev/LiquiVault/blob/main/docs/EVIDENCE.md).
