# Isolated local integration

The deployment runner supports a localhost-only standalone mode. It builds the normal contract WASM and exercises the signed setup/deposit/mint/verifier/factor/settle lifecycle with exact accounting assertions. It creates isolated local identities and never replaces `deployments/testnet.json`.

Start a dedicated Docker container using an official Stellar Quickstart image. Bind only localhost, use port 18000 for the network and keep an existing instance separate. The tested local image ID was `4c8bad1ef734` (6 October 2026 image); image IDs are machine-specific. Pin a published digest when reproducing elsewhere. Use the [official Quickstart instructions](https://github.com/stellar/quickstart) to obtain a current image.

```sh
docker run --detach --name liquivault-local-check --cpus 2 --memory 3g \
  --publish 127.0.0.1:18000:8000 \
  stellar/quickstart --local --limits testnet
node scripts/deploy-testnet.mjs local-check-001 --local
```

Use a fresh run ID when starting a fresh network. Public deployment evidence is written to `.stellar/proofs/<run-id>/deployment.json`; keys and resumable checkpoints stay alongside it and must remain ignored. The runner configures the `liquivault-local` CLI alias with RPC `http://127.0.0.1:18000/rpc` and passphrase `Standalone Network ; February 2017`. Funding is awaited before dependent deployment steps. An unresolved checkpoint must be inspected before retrying; do not blindly replay a possibly confirmed transaction.

The local pass on 12 November 2026 confirmed 19 transactions and the same lifecycle/accounting assertions as the testnet runner. Local hashes are not publicly queryable testnet evidence. This is CLI signing with operator-controlled local keys; it is not live Freighter signing or independent validation.

Stop only the container you started after testing:

```sh
docker stop liquivault-local-check
```

## Archival finding

### Verified replacement: continuous-ingestion fixture

The recovery integration now passes from three fresh local networks, including a rerun on 13 November 2026. With a built sibling SDK and Docker, Node.js, Rust/WASM and Stellar CLI installed, run this explicitly from the contracts checkout:

```sh
node scripts/run-archival-fixture.mjs unique-recovery-run-id
```

The runner pins `stellar/quickstart@sha256:4c8bad1ef7341205b898f83d9489321da80c7bd74183100fc8e2a39a5938c7d5`, uses Protocol 28 (matching the image's supported RPC hosts), waits for a healthy RPC and successful deployment simulation, seeds the lifecycle, waits for natural expiry, and runs the sibling SDK's `examples/archival-recovery.mjs`. It binds only localhost port 18000 and refuses to replace a network already there. It stops only its own container in cleanup; generated evidence and test identities remain in ignored `.stellar/archival/<run-id>/`.

This is an **instrumented fixture**, not production WASM. `prepare-archival-fixture.mjs` copies sources into an ignored workspace and changes only TTL thresholds/targets and the withdrawal-lock constant, recording every patch. Network persistence is 256 ledgers; thresholds are 32, extension targets 256, and the fixture withdrawal lock is one second. Canonical contract sources and public testnet records are unchanged. The evidence distinguishes the base source commit from instrumented source/WASM hashes.

Success requires observed expiry of both pool instance and LP position, restoration keys for both, rejected signing that leaves them expired, a confirmed withdrawal that restores both, zero remaining senior shares and an exact token-balance delta. Current RPC can return expired entries with `liveUntilLedgerSeq: 0`; absence from the response is not required. The local recovery receipt records the hash **before submission** and refuses automatic replay of an uncertain write.

The [published receipts and scope](https://github.com/MoscowDev/LiquiVault/blob/main/docs/gates/README.md) document the verified results. They are not public-testnet transactions, a production timing test, manual-recovery coverage, or independent-person validation.

### Earlier failed approach

An experimental Core `MANUAL_CLOSE` / `RUN_STANDALONE` jump beyond the instance and position TTLs did not produce a ledger stream that the bundled RPC ingested. That attempt stopped before signing or submitting a withdrawal. Do not use a ledger-sequence jump as a verified recovery recipe. The continuous-ingestion fixture above supersedes this failed approach.
