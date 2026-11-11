#!/usr/bin/env bash
# Portable runner performs checkpointing, assertions and public evidence recording.
set -euo pipefail
cd "$(dirname "$0")/.."
exec node scripts/deploy-testnet.mjs "$@"
