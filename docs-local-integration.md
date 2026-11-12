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

An experimental Core `MANUAL_CLOSE` / `RUN_STANDALONE` jump beyond the instance and position TTLs did not produce a ledger stream that the bundled RPC ingested. The recovery attempt stopped before signing or submitting a withdrawal. Do not describe the local lifecycle as an archival/restoration success, and do not use a ledger-sequence jump as a verified recovery recipe. A recovery harness must retain continuous RPC ingestion and demonstrate expired entries, the actual restoration footprint and a confirmed subsequent operation. That work remains open.
