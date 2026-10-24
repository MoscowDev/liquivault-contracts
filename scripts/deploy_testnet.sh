#!/usr/bin/env bash
# Deploys LiquiVault to Stellar Testnet, wires the contracts together, and runs an
# end-to-end smoke test (deposit -> mint -> verify -> factor -> settle).
#
# Usage (from liquivault-contracts/):  ./scripts/deploy_testnet.sh
# Requires: stellar-cli >= 22, Rust with the wasm32v1-none target.
# Output:   deployments/testnet.json (contract IDs, accounts, smoke-test results)
set -euo pipefail

STELLAR="${STELLAR:-stellar}"
NET="testnet"
TARGET="wasm32v1-none"
WASM_DIR="target/$TARGET/release"
OUT="deployments/testnet.json"

log() { echo -e "\n==> $*"; }
invoke() { local id=$1 src=$2; shift 2; "$STELLAR" contract invoke --id "$id" --source "$src" --network "$NET" -- "$@"; }

# ---------------------------------------------------------------------------
log "Building WASM (crates stay rlib for host tests; cdylib is requested here)"
for c in invoice_registry liquidity_pool factoring_escrow; do
  cargo rustc -p "$c" --crate-type cdylib --target "$TARGET" --release
done

# ---------------------------------------------------------------------------
log "Creating and funding testnet identities"
for k in lv-admin lv-verifier lv-treasury lv-merchant lv-debtor lv-lp; do
  "$STELLAR" keys address "$k" >/dev/null 2>&1 || "$STELLAR" keys generate "$k" --network "$NET"
  "$STELLAR" keys fund "$k" --network "$NET" >/dev/null 2>&1 || true
done
ADMIN=$("$STELLAR" keys address lv-admin)
VERIFIER=$("$STELLAR" keys address lv-verifier)
TREASURY=$("$STELLAR" keys address lv-treasury)
MERCHANT=$("$STELLAR" keys address lv-merchant)
DEBTOR=$("$STELLAR" keys address lv-debtor)
LP=$("$STELLAR" keys address lv-lp)

# ---------------------------------------------------------------------------
log "Issuing test USDC (classic asset USDC:<admin> wrapped as a Stellar Asset Contract)"
ASSET="USDC:$ADMIN"
USDC=$("$STELLAR" contract id asset --asset "$ASSET" --network "$NET")
"$STELLAR" contract asset deploy --asset "$ASSET" --source lv-admin --network "$NET" >/dev/null 2>&1 || true
for k in lv-treasury lv-merchant lv-debtor lv-lp; do
  "$STELLAR" tx new change-trust --source "$k" --line "$ASSET" --network "$NET" >/dev/null
done
invoke "$USDC" lv-admin mint --to "$LP" --amount 1000000000000      # 100,000 USDC
invoke "$USDC" lv-admin mint --to "$DEBTOR" --amount 150000000000   #  15,000 USDC

# ---------------------------------------------------------------------------
log "Deploying contracts"
deploy() { "$STELLAR" contract deploy --wasm "$WASM_DIR/$1.wasm" --source lv-admin --network "$NET"; }
REGISTRY=$(deploy invoice_registry)
POOL=$(deploy liquidity_pool)
ESCROW=$(deploy factoring_escrow)

log "Initializing and cross-binding"
invoke "$REGISTRY" lv-admin initialize --admin "$ADMIN" --verifier "$VERIFIER" --factoring_escrow "$ESCROW"
invoke "$POOL" lv-admin initialize --admin "$ADMIN" --usdc_token "$USDC" --factoring_escrow "$ESCROW" \
  --treasury "$TREASURY" --reserve_factor_bps 1000
invoke "$ESCROW" lv-admin initialize --admin "$ADMIN" --invoice_registry "$REGISTRY" \
  --liquidity_pool "$POOL" --usdc_token "$USDC" \
  --rate_config '{"grace_period_seconds":604800,"origination_fee_bps":50,"tier_a_annual_bps":600,"tier_b_annual_bps":1000,"tier_c_annual_bps":1500}'

# ---------------------------------------------------------------------------
log "Smoke test: senior + junior deposits"
invoke "$POOL" lv-lp deposit --provider "$LP" --amount 500000000000          # 50,000 senior
invoke "$POOL" lv-lp deposit_junior --provider "$LP" --amount 200000000000   # 20,000 junior

log "Smoke test: merchant mints a 10,000 USDC invoice due in 60 days"
MATURITY=$(( $(date +%s) + 86400 * 60 ))
INVOICE_ID=$(invoke "$REGISTRY" lv-merchant mint_invoice --merchant "$MERCHANT" --debtor "$DEBTOR" \
  --currency "$USDC" --face_value 100000000000 --maturity_timestamp "$MATURITY" \
  --metadata_hash "$(printf 'LiquiVault testnet smoke invoice' | sha256sum | cut -d' ' -f1)" \
  --invoice_number "INV-TESTNET-001" --risk_tier 1 | tr -d '"')

log "Smoke test: verifier oracle approves invoice $INVOICE_ID as Tier B"
invoke "$REGISTRY" lv-verifier verify_invoice --caller "$VERIFIER" --invoice_id "$INVOICE_ID" --risk_tier 1

log "Smoke test: merchant factors the invoice"
QUOTE=$(invoke "$ESCROW" lv-merchant factor_invoice --merchant "$MERCHANT" --invoice_id "$INVOICE_ID")
MERCHANT_BAL=$(invoke "$USDC" lv-admin balance --id "$MERCHANT" | tr -d '"')

log "Smoke test: debtor settles"
SETTLE=$(invoke "$ESCROW" lv-debtor settle_invoice --payer "$DEBTOR" --invoice_id "$INVOICE_ID")
POOL_STATE=$(invoke "$POOL" lv-admin get_pool_state)
JUNIOR_STATE=$(invoke "$POOL" lv-admin get_junior_state)
INVOICE=$(invoke "$REGISTRY" lv-admin get_invoice --invoice_id "$INVOICE_ID")

# ---------------------------------------------------------------------------
mkdir -p deployments
cat > "$OUT" <<EOF
{
  "network": "testnet",
  "deployedAt": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "contracts": {
    "invoiceRegistry": "$REGISTRY",
    "liquidityPool": "$POOL",
    "factoringEscrow": "$ESCROW",
    "usdcToken": "$USDC"
  },
  "testUsdcAsset": "$ASSET",
  "accounts": {
    "admin": "$ADMIN",
    "verifier": "$VERIFIER",
    "treasury": "$TREASURY",
    "merchant": "$MERCHANT",
    "debtor": "$DEBTOR",
    "liquidityProvider": "$LP"
  },
  "smokeTest": {
    "invoiceId": $INVOICE_ID,
    "factoringQuote": $QUOTE,
    "merchantUsdcAfterFactoring": "$MERCHANT_BAL",
    "settlement_lpYield_protocolCut": $SETTLE,
    "invoiceAfterSettlement": $INVOICE,
    "poolState": $POOL_STATE,
    "juniorState": $JUNIOR_STATE
  }
}
EOF
log "Done. Wrote $OUT"
cat "$OUT"
