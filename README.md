# LiquiVault contracts

[![CI](https://github.com/MoscowDev/liquivault-contracts/actions/workflows/ci.yml/badge.svg)](https://github.com/MoscowDev/liquivault-contracts/actions/workflows/ci.yml)
[MIT license](LICENSE)

Three experimental Soroban contracts for invoice factoring: `invoice_registry`, `liquidity_pool` and `factoring_escrow`. **Pre-audit, testnet only.** The demonstration uses a custom test token, not Circle-issued USDC.

## Build and test

Use a current Rust toolchain and install the target:

```sh
rustup target add wasm32v1-none
cargo test --workspace --locked
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -A clippy::inconsistent_digit_grouping -A clippy::too_many_arguments -D warnings
for c in invoice_registry liquidity_pool factoring_escrow; do
  cargo rustc -p "$c" --crate-type cdylib --target wasm32v1-none --release --locked
done
```

Crates are declared as rlibs for host tests. The explicit cdylib builds produce deployment WASM in `target/wasm32v1-none/release/`. The checked-in CI uses these same commands. See [CONTRIBUTING.md](CONTRIBUTING.md) for PowerShell instructions.

## Contracts and authorization

| Contract | Main entry points | Authority |
| --- | --- | --- |
| Registry | `mint_invoice`, `verify_invoice`, `get_invoice`, `get_merchant_invoices`, `transfer_ownership` | Merchant mints; configured verifier/admin verifies; eligible owner transfers |
| Registry lifecycle | `mark_factored`, `mark_settled`, `mark_defaulted` | Configured escrow only |
| Pool | `deposit`, `withdraw`, `deposit_junior`, `withdraw_junior`, state/position getters | Provider authorizes token/share operations |
| Pool lifecycle | `allocate_liquidity`, `repay_liquidity`, `write_off_default` | Configured escrow; payer also authorizes repayment |
| Escrow | `preview_factoring_quote`, `factor_invoice`, `settle_invoice`, `mark_defaulted` | Owner factors; payer repays; admin defaults after grace |
| Administration | `set_paused`, `transfer_admin`, `upgrade`, contract-specific settings | Current admin; new admin also authorizes role transfer |

Exact argument names/order and types are in each `src/lib.rs` and `src/types.rs`. IDs/timestamps are `u64`; amounts/shares are `i128`; risk tiers encode as `u32`. On-chain ownership fields do not establish legal title to a receivable.

## Accounting protections

- Junior capital absorbs defaults before senior capital. Both tranches can lose their entire deposit.
- Junior yield is capped by its share of total capital and the configured bps (default 3,000; configured maximum 8,000). It is not a guaranteed yield premium.
- Each provider's latest deposit locks that tranche position for 24 hours, including previously held shares.
- Withdrawals require idle liquidity. Junior withdrawals must additionally preserve at least 10% coverage of deployed advances.
- A wiped-out tranche with outstanding shares rejects new deposits. No automatic recapitalization/reset exists.
- Registry defaults are escrow-only, preventing disconnected loss accounting.
- Escrow pause blocks new factoring while repayment and overdue default accounting remain available.
- All three contracts expose admin-authorized WASM upgrades. Storage compatibility and trusted admin custody remain necessary; no timelock or migration engine is included.

## Deployment evidence

[deployments/testnet.json](deployments/testnet.json) is the current public record. Historical records are explicitly dated. A deployment record is evidence only for its source/WASM identifiers; the October deployment does not contain the later repairs.

For a fresh signed demonstration use `node scripts/deploy-testnet.mjs` with Node.js 22.13+ and Stellar CLI 28.1+. The runner builds current WASM, uses isolated testnet identities, checkpoints progress, verifies the invoice lifecycle and accounting, and writes the public record only after all assertions pass. See the [shared deployment guide](https://github.com/MoscowDev/LiquiVault/blob/main/docs/deployment.md). Never commit local `.stellar/` configuration or secret keys.

## Documentation

- [Architecture and formulas](https://github.com/MoscowDev/LiquiVault/blob/main/docs/architecture.md)
- [Verification evidence](https://github.com/MoscowDev/LiquiVault/blob/main/docs/EVIDENCE.md)
- [Storage and archival limits](https://github.com/MoscowDev/LiquiVault/blob/main/docs/storage.md)
- [Security policy](SECURITY.md)
- [Contribution guide](CONTRIBUTING.md)

No mainnet readiness, independent audit, legal collection infrastructure or Drips approval is claimed.
