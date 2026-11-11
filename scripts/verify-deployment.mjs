import { readFileSync } from 'node:fs';
import assert from 'node:assert/strict';
const record = JSON.parse(readFileSync(new URL('../deployments/testnet.json', import.meta.url), 'utf8'));
assert.equal(record.network, 'testnet');
assert.equal(record.smokeTest.assertionsPassed, true);
assert.match(record.provenance.contractSourceCommit, /^[0-9a-f]{40}$/);
for (const value of Object.values(record.provenance.wasmSha256)) assert.match(value, /^[0-9a-f]{64}$/);
for (const value of Object.values(record.contracts)) assert.match(value, /^C[A-Z2-7]{55}$/);
assert.ok(record.transactions.length >= 19);
assert.equal(new Set(record.transactions.map(tx => tx.hash)).size, record.transactions.length);
for (const tx of record.transactions) {
  assert.match(tx.hash, /^[0-9a-f]{64}$/);
  assert.ok(Number.isSafeInteger(tx.ledger) && tx.ledger > 0);
}
for (const step of ['mint-invoice', 'verify-invoice', 'factor-invoice', 'settle-invoice']) {
  assert.ok(record.transactions.some(tx => tx.step === step));
}
const proof = record.smokeTest;
const quote = proof.factoringQuote;
const gross = BigInt(quote.total_discount);
const lpYield = BigInt(proof.settlement_lpYield_protocolCut[0]);
const protocolCut = BigInt(proof.settlement_lpYield_protocolCut[1]);
assert.equal(gross, BigInt(quote.time_discount_amount) + BigInt(quote.origination_fee_amount));
assert.equal(BigInt(quote.face_value), BigInt(quote.net_payout) + gross);
assert.equal(protocolCut, gross * 1000n / 10000n);
assert.equal(gross, lpYield + protocolCut);
assert.equal(BigInt(proof.merchantUsdcAfterFactoring), BigInt(quote.net_payout));
assert.equal(proof.invoiceAfterSettlement.status, 3);
assert.equal(BigInt(proof.poolState.deployed_capital), 0n);
const juniorBps = 200000000000n * 10000n / 700000000000n;
const juniorYield = lpYield * juniorBps / 10000n;
assert.equal(BigInt(proof.juniorState.total_assets), 200000000000n + juniorYield);
assert.equal(BigInt(proof.poolState.total_assets), 500000000000n + lpYield - juniorYield);
assert.equal(BigInt(proof.effectiveJuniorYieldBps), juniorBps);
console.log(`Verified public proof ${record.runId}: ${record.transactions.length} transactions and exact token-unit accounting.`);
