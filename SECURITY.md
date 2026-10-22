# Security Policy & Threat Model

> **Status: pre-audit, testnet-only.** These contracts have not been independently audited. Do not deploy with real funds.

## Reporting a vulnerability

Please **do not open a public issue** for security problems. Report privately via GitHub Security Advisories on this repository ("Security" tab, "Report a vulnerability"). Include steps to reproduce and the affected contract/function. We aim to acknowledge reports within 72 hours.

## Scope

| Contract | Role |
| :--- | :--- |
| `invoice_registry` | Tokenizes invoices; tracks status and ownership; verification. |
| `factoring_escrow` | Prices invoices, swaps them for USDC from the pool, settles or defaults. |
| `liquidity_pool` | Holds LP USDC, issues pro-rata shares, funds factoring, books yield and losses. |

## Roles and trust assumptions

| Role | Powers | Trust assumption |
| :--- | :--- | :--- |
| **Admin** (per contract) | Set escrow address, pause (registry, escrow), update rates (escrow), transfer admin (escrow), mark escrow invoices defaulted after grace. | Honest and key-secure. Should be a multisig. |
| **Verifier** | Verify invoices and set their risk tier. | Honest and performs real-world checks off-chain. **The protocol cannot check that an invoice is real.** |
| **Debtor** | Named on the invoice; repays at maturity. No verification rights (see F-1). | Untrusted. |
| **Merchant** | Mints invoices, receives USDC. | Untrusted. |
| **LPs** | Deposit and withdraw USDC. | Untrusted. |

## Known issues and limitations (read before relying on this code)

Severity is our own assessment.

### F-1 (Critical, FIXED): a merchant could self-verify a fake invoice
`invoice_registry::verify_invoice` used to accept the invoice's **debtor** as a valid verifier, and `mint_invoice` lets the merchant choose any debtor address. A merchant could name a second address they control as debtor, verify with it (also choosing the risk tier), factor the invoice and receive pool USDC for a receivable that doesn't exist. **Fix:** only the verifier or admin can verify. Regression test: `test_debtor_and_merchant_cannot_verify_own_invoice`. A debtor co-signature may be added later as an *additional* step, never as a substitute.

### F-2 (High, FIXED): pool could not be paused
`liquidity_pool` checked `state.paused`, but nothing set it. **Fix:** added admin-only `set_paused`. Regression test: `test_set_paused_blocks_deposit_and_withdraw_and_requires_admin`. Note that `repay_liquidity`, `allocate_liquidity` (paused check) and `write_off_default` behave as before; pausing does not block repayments.

### F-3 (High, PARTIALLY MITIGATED): LPs can exit ahead of a default
`write_off_default` reduces `total_assets` when an invoice is defaulted, so remaining LPs absorb the loss. LPs who withdraw idle liquidity before `mark_defaulted` avoid it. This is inherent to a pooled design. Possible mitigations: withdrawal queue or delay, per-tier loss reserves, an insurance reserve funded from the reserve factor.
**Mitigation added:** a junior (first-loss) tranche. Defaults hit junior capital first, and junior withdrawals are blocked unless junior capital stays at or above 10% of deployed capital. Senior LPs can still exit ahead of a default and leave junior and remaining senior holders with the loss, so a withdrawal queue is still advisable.

### F-4 (Medium): centralized default marking and state mismatch
Only the escrow admin can call `factoring_escrow::mark_defaulted` (after the grace period). `invoice_registry::mark_defaulted` can also be called directly by the registry admin **without** a grace-period check and without updating the escrow record, which leaves registry and escrow out of sync.

### F-5 (Medium): off-chain verification is out of scope
The risk tier and the validity of an invoice come from a trusted party (verifier). There is no on-chain proof of the underlying trade. `metadata_hash` is only a commitment to off-chain documents.

### F-6 (Medium): pool accounting edge cases
- If defaults drive `total_assets` to 0 while `total_shares > 0`, the next deposit mints 1:1 shares and the share price is mis-based.
- `write_off_default` uses `unwrap_or(0)` when subtracting from `total_assets`; this silently floors instead of reverting.
- Withdrawals are limited to idle liquidity, so LPs can be temporarily unable to exit while capital is deployed.

### F-7 (Low): missing checks and events
- `invoice_registry::mark_settled` / `mark_defaulted` skip the pause check.
- `set_factoring_escrow` (pool, registry) emits no event, and neither contract has an admin transfer function.
- `settle_invoice` accepts any payer (by design: anyone may repay), and does not require the payer to be the debtor.

### Design notes
- Arithmetic uses checked operations; amounts are `i128`.
- State is written before cross-contract calls. On Soroban a failed call reverts the whole transaction, so this ordering is safe, but we do not rely on classic reentrancy guards.
- Pool balances are tracked internally (`total_assets`), so direct token donations to the pool do not change the share price.
- TTLs are bumped on access; archived entries need restoration by keepers.

## Recommended before mainnet
1. Address F-3 to F-7.
2. Move all admin roles to a multisig; separate the admin and verifier keys.
3. Independent audit and a bug bounty.
4. Legal and KYC/AML review for the verifier process and merchant onboarding.
