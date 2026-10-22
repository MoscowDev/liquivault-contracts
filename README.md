# LiquiVault Smart Contracts (`liquivault-contracts`)

**LiquiVault** is an institutional-grade decentralized trade invoice factoring and liquidity protocol built on **Stellar Soroban** (`soroban-sdk = "21.7.7"`, `#![no_std]`).

## Workspace Modules

1. **`contracts/invoice_registry`**: Manages trade receivable tokenization (`mint_invoice`), SHA-256 document metadata commitment (`BytesN<32>`), credit risk tier assignment (`RiskTier::TierA`, `TierB`, `TierC`), verifier-oracle/admin verification (`verify_invoice`), and ownership lifecycle state transitions.
2. **`contracts/liquidity_pool`**: Manages USDC vault deposits (`deposit`), NAV-adjusted LP share minting/burning (`withdraw`), capital deployment to factoring escrows (`allocate_liquidity`), and pro-rata yield distribution (`repay_liquidity`).
3. **`contracts/factoring_escrow`**: Executes atomic swaps of verified unpaid invoices for immediate USDC liquidity at an algorithmic time-and-risk-adjusted discount (`factor_invoice`) and handles maturity settlements (`settle_invoice`).

## Security & Standards

- **Checks-Effects-Interactions (CEI)**: All state mutations (`env.storage()`) are committed prior to cross-contract calls or `token::Client` transfers.
- **Checked Arithmetic**: All 128-bit financial math uses `checked_add`, `checked_sub`, `checked_mul`, and `checked_div`.
- **Strict Access Control**: Enforced via `Address::require_auth()` and role verification across Admin, Verifier Oracle, Merchant, Debtor, and FactoringEscrow.

## Build, Test & Deploy

```bash
# Run all unit and integration tests
cargo test --workspace

# Build optimized Soroban WASM binaries
stellar contract build

# Deploy to Stellar Testnet
stellar contract deploy --wasm target/wasm32-unknown-unknown/release/invoice_registry.wasm --source admin --network testnet
stellar contract deploy --wasm target/wasm32-unknown-unknown/release/liquidity_pool.wasm --source admin --network testnet
stellar contract deploy --wasm target/wasm32-unknown-unknown/release/factoring_escrow.wasm --source admin --network testnet
```

## Tranched Liquidity Pool

The `liquidity_pool` supports two risk tranches:

| Tranche | Deposit / withdraw | Risk | Return |
| :--- | :--- | :--- | :--- |
| **Senior** | `deposit` / `withdraw` | Loses only after junior capital is exhausted | Remaining LP yield (default 70%) |
| **Junior** (first-loss) | `deposit_junior` / `withdraw_junior` | Absorbs defaults first | `junior_yield_bps` of LP yield (default 30%, admin-set, max 80%) |

- With no junior capital the pool behaves exactly like a single-tranche pool.
- Junior withdrawals must leave junior capital of at least 10% of deployed capital, so first-loss capital cannot leave while invoices are outstanding.
- If defaults wipe out the junior tranche, new junior deposits are rejected until stale shares are resolved.
