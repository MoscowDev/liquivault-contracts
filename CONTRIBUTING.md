# Contributing to LiquiVault Smart Contracts (`liquivault-contracts`)

Thank you for your interest in contributing to **LiquiVault**, an institutional-grade decentralized trade invoice factoring and liquidity protocol on **Stellar Soroban**! We actively welcome contributors participating in **Drips Wave** and the broader Stellar ecosystem.

---

## Prerequisites

- **Rust Toolchain**: `rustup` with stable compiler (`>= 1.78`)
- **WASM Target**: `rustup target add wasm32-unknown-unknown`
- **Stellar CLI**: `cargo install --locked stellar-cli --features opt`

---

## Getting Started

1. **Fork & Clone the Repository**
   ```bash
   git clone https://github.com/<your-username>/liquivault-contracts.git
   cd liquivault-contracts
   ```

2. **Create a Feature Branch**
   Use a descriptive branch name referencing the issue number:
   ```bash
   git checkout -b feat/issue-12-multi-currency-support
   ```

3. **Run the Workspace Test Suite**
   ```bash
   cargo test --workspace
   ```

4. **Check Formatting & Lints**
   ```bash
   cargo fmt --all -- --check
   cargo clippy --workspace --all-targets -- -D warnings
   ```

---

## Smart Contract Security & Engineering Standards

All pull requests modifying Soroban contracts (`invoice_registry`, `liquidity_pool`, `factoring_escrow`) must adhere to:

1. **`#![no_std]` Compatibility**: Do not use standard library allocations in contract execution paths.
2. **Checks-Effects-Interactions (CEI)**: Validate all authorization (`Address::require_auth()`) and invariants first, commit all `env.storage()` state mutations second, and perform cross-contract or SEP-41 `token::Client` transfers last.
3. **Checked Arithmetic**: Always use `.checked_add()`, `.checked_sub()`, `.checked_mul()`, and `.checked_div()`, returning a typed contract error (`RegistryError`, `PoolError`, or `EscrowError`) on overflow/underflow.
4. **Storage TTL Management**: Extend instance and persistent storage TTLs whenever state records are created or updated.
5. **Unit & Integration Tests**: Every new contract function or error branch must include unit tests in the crate's `src/test.rs`.

---

## Submitting a Pull Request

1. Ensure all tests pass (`cargo test --workspace`).
2. Reference the GitHub Issue in your PR description using `Closes #<issue-number>`.
3. Complete the checklist in `.github/PULL_REQUEST_TEMPLATE.md`.
