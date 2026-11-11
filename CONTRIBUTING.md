# Contributing to the contracts

Fork this repository, branch from `main`, read the relevant contract and [security policy](SECURITY.md), and open a scoped PR linked to an unfinished issue. Do not publish sensitive exploit details in public issues.

## Prerequisites

A current Rust toolchain, `wasm32v1-none`, and Git. Node.js 22.13+ and Stellar CLI 28.1+ are only needed for the portable signed testnet demonstration. No other LiquiVault checkout is required for contract builds/tests.

```sh
git clone https://github.com/MoscowDev/liquivault-contracts.git
cd liquivault-contracts
rustup target add wasm32v1-none
cargo test --workspace --locked
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -A clippy::inconsistent_digit_grouping -A clippy::too_many_arguments -D warnings
```

Bash deployment artifacts:

```sh
for c in invoice_registry liquidity_pool factoring_escrow; do
  cargo rustc -p "$c" --crate-type cdylib --target wasm32v1-none --release --locked
done
```

PowerShell equivalent:

```powershell
foreach ($contract in @('invoice_registry','liquidity_pool','factoring_escrow')) {
  cargo rustc -p $contract --crate-type cdylib --target wasm32v1-none --release --locked
  if ($LASTEXITCODE -ne 0) { throw "WASM build failed" }
}
```

## Review expectations

Use checked financial arithmetic and typed errors. Test authorization, boundaries and registry/escrow/pool consistency for consequential changes. Explain economic effects and storage-schema compatibility. Follow CI's formatting/clippy commands; the two allowances above cover existing test-number style and public contract signatures.

Do not commit secrets, `.stellar/`, `target/`, or arbitrary generated artifacts. Intentional host-test snapshots are tracked. Signed testnet evidence must identify source, WASM, network, assertions and transactions; simulation alone must not be called a deployment.

Use the issue template for non-sensitive tasks with clear acceptance criteria and verification commands. Maintainers review code, tests and docs; passing CI is not an audit. See the [shared contributor guide](https://github.com/MoscowDev/LiquiVault/blob/main/CONTRIBUTING.md) and [official Wave guidance](https://docs.drips.network/wave/maintainers/participating-in-a-wave/).
