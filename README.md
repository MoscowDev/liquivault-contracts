# LiquiVault Smart Contracts (`liquivault-contracts`)

[![License](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](../LICENSE)
[![Stellar Protocol](https://img.shields.io/badge/Stellar_Protocol-21%20%7C%2022-brightgreen.svg)](https://stellar.org)
[![Soroban SDK](https://img.shields.io/badge/Soroban_SDK-21.7.7-orange.svg)](https://soroban.stellar.org)
[![Rust](https://img.shields.io/badge/Rust-1.80%2B-red.svg)](https://www.rust-lang.org)
[![Test Suite](https://img.shields.io/badge/Tests-19%2F19%20Passed-success.svg)](../docs/EVIDENCE.md)

**Soroban contracts for invoice factoring with a senior/junior tranched liquidity pool** (pre-audit, testnet only) (`soroban-sdk = "21.7.7"`, `#![no_std]`).

---

## 1. Workspace Contracts

```text
contracts/
├── invoice_registry/   # Receivable tokenization, credit risk tiers, and lifecycle states
├── liquidity_pool/     # Senior (lvUSDC) and Junior (jUSDC) liquidity tranches & yield
└── factoring_escrow/   # Algorithmic discount engine, atomic factoring swaps, and default write-offs
```

### 1.1 `invoice_registry`
The central registry for tokenized trade receivables and underwriting verification:
- `mint_invoice(merchant, debtor, face_value, currency, maturity, invoice_ref, metadata_hash)`: Mints a new receivable in `Issued` status with an immutable SHA-256 metadata hash commitment (`BytesN<32>`).
- `verify_invoice(invoice_id, risk_tier)`: Restricted to the authorized Verifier Oracle or Admin. Assigns credit risk (`TierA`, `TierB`, `TierC`) and transitions status to `Verified`. Self-verification by merchants or debtors is permanently blocked (Finding F-1).
- `mark_factored(invoice_id, escrow)`: Restricted to authorized factoring escrow. Transfers legal ownership to the escrow contract.
- `mark_settled(invoice_id)`: Restricted to authorized factoring escrow upon debtor repayment.
- `mark_defaulted(invoice_id)`: Restricted to authorized factoring escrow upon write-off.

### 1.2 `liquidity_pool`
A dual-tranche automated USDC vault:
- **Senior Tranche (`lvUSDC`)**:
  - `deposit(from, amount)`: Mints pro-rata shares at dynamic NAV share price.
  - `withdraw(from, shares)`: Burns shares for underlying USDC bounded by idle pool liquidity.
  - Absorbs losses only after the junior tranche is 100% depleted.
- **Junior First-Loss Tranche (`jUSDC`)**:
  - `deposit_junior(from, amount)`: Mints first-loss shares.
  - `withdraw_junior(from, shares)`: Governed by the **10% Deployed Capital Coverage Rule**. Rejects redemptions if remaining junior assets drop below 10% of active deployed advances (`JuniorCoverageTooLow`).
  - Absorbs 100% of initial defaults up to total junior capital.
  - Earns an outsized yield share (`junior_yield_bps`, default 30%, max 80%).
- `allocate_liquidity(amount, to)`: Transfers advance payout to factoring merchant upon escrow request.
- `repay_liquidity(principal, gross_yield)`: Processes debtor repayments, deducts treasury reserve fee, and distributes net yield pro-rata.
- `write_off_default(principal)`: Reduces pool capital during defaults, absorbing through junior first.

### 1.3 `factoring_escrow`
The core factoring and settlement coordinator:
- `calculate_discount(face_value, maturity, current_time, risk_tier)`: Pure calculation of time-weighted interest and origination fees.
- `factor_invoice(invoice_id)`: Atomically validates invoice, marks factored in registry, commits escrow record, and routes net USDC payout to merchant.
- `settle_invoice(invoice_id, payer)`: Accepts face value repayment, routes principal and yield to pool, and settles the invoice.
- `mark_defaulted(invoice_id)`: Triggers write-off after `maturity + grace_period_seconds` (admin-only).

---

## 2. Invariant & Security Standards

- **Checks-Effects-Interactions (CEI)**: All internal state mutations (`env.storage()`) precede external cross-contract calls or `token::Client` transfers.
- **Checked Arithmetic**: All calculations use 128-bit checked math (`checked_add`, `checked_sub`, `checked_mul`, `checked_div`).
- **Strict Role Verification**: Enforced via `Address::require_auth()` across Admin, Verifier, Merchant, Debtor, and Escrow.
- **Automated State TTL Extension**: All persistent entries are extended by 60 days on every access. See [Storage & TTL Policy](../docs/storage.md).

---

## 3. Build & Test Commands

```bash
# Run all workspace unit and integration tests (19 tests)
cargo test --workspace

# Run linter and formatting checks
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings

# Build Soroban WASM artifacts (crates are rlib, so request cdylib explicitly)
for c in invoice_registry liquidity_pool factoring_escrow; do
  cargo rustc -p $c --crate-type cdylib --target wasm32v1-none --release
done
```

---

## 4. Documentation Links

- [Protocol Architecture Specification](../docs/architecture.md)
- [Verified Test Evidence & Proofs](../docs/EVIDENCE.md)
- [Contract Storage & TTL Lifecycle Policy](../docs/storage.md)
- [Security Policy & Threat Model](../docs/SECURITY.md)
- [Testnet Deployment Runbook](../docs/deployment.md)
