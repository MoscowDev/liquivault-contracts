#!/usr/bin/env node
// Signed testnet proof. Secrets stay in ignored, isolated Stellar CLI config.
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { readFileSync, writeFileSync, mkdirSync, existsSync, readdirSync } from 'node:fs';
import { resolve, dirname, relative, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import assert from 'node:assert/strict';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const runId = process.argv[2] ?? new Date().toISOString().replace(/[^0-9]/g, '').slice(0, 14);
if (!/^[a-zA-Z0-9_-]{1,40}$/.test(runId)) throw new Error('Invalid run ID');
const runDir = resolve(root, '.stellar', 'proofs', runId);
const config = join(runDir, 'config');
mkdirSync(config, { recursive: true });
const checkpoint = join(runDir, 'progress.json');
const progress = existsSync(checkpoint) ? JSON.parse(readFileSync(checkpoint, 'utf8')) : { runId, steps: {}, transactions: [] };
const rpcUrl = 'https://soroban-testnet.stellar.org';
const horizonUrl = 'https://horizon-testnet.stellar.org';
const cli = process.env.STELLAR ?? 'stellar';
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
const save = () => writeFileSync(checkpoint, JSON.stringify(progress, null, 2) + '\n');

function execute(command, args, allowFailure = false) {
  const result = spawnSync(command, args, { cwd: root, encoding: 'utf8', windowsHide: true, maxBuffer: 8 * 1024 * 1024 });
  if (result.status !== 0 && !allowFailure) {
    // Commands use aliases, never private keys in argv. Do not print key-generation output.
    const detail = args.includes('generate') ? 'Key generation failed' : (result.stderr ?? '').slice(-3000);
    throw new Error(`${command} failed (${result.status}): ${detail}`);
  }
  return { stdout: (result.stdout ?? '').trim(), stderr: result.stderr ?? '', status: result.status };
}
function stellar(args) { return execute(cli, ['--config-dir', config, ...args]); }
function parse(value) { try { return JSON.parse(value); } catch { return value; } }
async function jsonFetch(url, options) {
  const response = await fetch(url, { ...options, signal: AbortSignal.timeout(30000) });
  if (!response.ok) throw new Error(`HTTP ${response.status} at ${url.split('?')[0]}`);
  return response.json();
}
async function latestTx(address) {
  const page = await jsonFetch(`${horizonUrl}/accounts/${address}/transactions?order=desc&limit=1`);
  return page._embedded.records[0];
}
async function confirmedTx(address, previousHash) {
  for (let attempt = 0; attempt < 30; attempt++) {
    const tx = await latestTx(address);
    if (tx?.source_account === address && tx.hash !== previousHash) {
      assert.equal(tx.successful, true, 'Ledger transaction failed');
      return { hash: tx.hash, ledger: tx.ledger, source: address };
    }
    await sleep(1000);
  }
  throw new Error('Transaction evidence unavailable. Inspect checkpoint/accounts before rerunning; do not repeat a possibly confirmed operation.');
}
async function step(name, action, source) {
  if (Object.hasOwn(progress.steps, name)) return progress.steps[name];
  if (progress.inFlight) throw new Error(`Unresolved operation ${progress.inFlight.name}; inspect chain state and checkpoint before retrying.`);
  console.log(name);
  const previousHash = source ? (await latestTx(source))?.hash : undefined;
  progress.inFlight = { name, source, previousHash, startedAt: new Date().toISOString() };
  save();
  const output = action();
  const transaction = source ? await confirmedTx(source, previousHash) : undefined;
  const value = parse(output.stdout);
  if (transaction) progress.transactions.push({ step: name, ...transaction });
  progress.steps[name] = value;
  delete progress.inFlight;
  save();
  return value;
}
function read(id, method, args = []) {
  return parse(stellar(['contract', 'invoke', '--id', id, '--source', 'admin', '--network', 'testnet', '--send', 'no', '--', method, ...args]).stdout);
}
function invoke(id, alias, method, args = []) {
  return stellar(['contract', 'invoke', '--id', id, '--source', alias, '--network', 'testnet', '--send', 'yes', '--', method, ...args]);
}
function collectSources(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap(entry => {
    const path = join(directory, entry.name);
    return entry.isDirectory() ? collectSources(path) : /\.(rs|toml)$/.test(path) ? [path] : [];
  });
}
const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');
const sourceFiles = [...collectSources(join(root, 'contracts')), join(root, 'Cargo.toml'), join(root, 'Cargo.lock')]
  .sort().map(path => ({ path: relative(root, path).replaceAll('\\', '/'), sha256: sha256(readFileSync(path)) }));
const sourceDigest = sha256(JSON.stringify(sourceFiles));
if (progress.sourceDigest && progress.sourceDigest !== sourceDigest) throw new Error('Contract sources changed during a checkpointed deployment; use a new run ID.');
progress.sourceDigest = sourceDigest;
save();

for (const name of ['invoice_registry', 'liquidity_pool', 'factoring_escrow']) {
  console.log(`Build ${name}`);
  execute('cargo', ['rustc', '-p', name, '--crate-type', 'cdylib', '--target', 'wasm32v1-none', '--release', '--locked']);
}
const accounts = {};
for (const alias of ['admin', 'verifier', 'treasury', 'merchant', 'debtor', 'lp']) {
  const existing = execute(cli, ['--config-dir', config, 'keys', 'address', alias], true);
  if (existing.status !== 0) stellar(['keys', 'generate', alias, '--network', 'testnet']);
  accounts[alias] = stellar(['keys', 'address', alias]).stdout.trim();
  await step(`fund-${alias}`, () => stellar(['keys', 'fund', alias, '--network', 'testnet']));
}
const asset = `LVTEST:${accounts.admin}`;
const token = stellar(['contract', 'id', 'asset', '--asset', asset, '--network', 'testnet']).stdout.trim();
await step('deploy-test-asset', () => stellar(['contract', 'asset', 'deploy', '--asset', asset, '--source', 'admin', '--network', 'testnet']), accounts.admin);
for (const alias of ['treasury', 'merchant', 'debtor', 'lp']) {
  await step(`trust-${alias}`, () => stellar(['tx', 'new', 'change-trust', '--source', alias, '--line', asset, '--network', 'testnet']), accounts[alias]);
}
await step('mint-lp-tokens', () => invoke(token, 'admin', 'mint', ['--to', accounts.lp, '--amount', '1000000000000']), accounts.admin);
await step('mint-debtor-tokens', () => invoke(token, 'admin', 'mint', ['--to', accounts.debtor, '--amount', '150000000000']), accounts.admin);

const contracts = { usdcToken: token };
const wasmHashes = {};
for (const [key, name] of [['invoiceRegistry', 'invoice_registry'], ['liquidityPool', 'liquidity_pool'], ['factoringEscrow', 'factoring_escrow']]) {
  const path = join(root, 'target', 'wasm32v1-none', 'release', `${name}.wasm`);
  wasmHashes[key] = sha256(readFileSync(path));
  contracts[key] = await step(`deploy-${name}`, () => stellar(['contract', 'deploy', '--wasm', path, '--source', 'admin', '--network', 'testnet']), accounts.admin);
  assert.match(contracts[key], /^C[A-Z2-7]{55}$/);
}
await step('initialize-registry', () => invoke(contracts.invoiceRegistry, 'admin', 'initialize', ['--admin', accounts.admin, '--verifier', accounts.verifier, '--factoring_escrow', contracts.factoringEscrow]), accounts.admin);
await step('initialize-pool', () => invoke(contracts.liquidityPool, 'admin', 'initialize', ['--admin', accounts.admin, '--usdc_token', token, '--factoring_escrow', contracts.factoringEscrow, '--treasury', accounts.treasury, '--reserve_factor_bps', '1000']), accounts.admin);
await step('initialize-escrow', () => invoke(contracts.factoringEscrow, 'admin', 'initialize', ['--admin', accounts.admin, '--invoice_registry', contracts.invoiceRegistry, '--liquidity_pool', contracts.liquidityPool, '--usdc_token', token, '--rate_config', JSON.stringify({ grace_period_seconds: 604800, origination_fee_bps: 50, tier_a_annual_bps: 600, tier_b_annual_bps: 1000, tier_c_annual_bps: 1500 })]), accounts.admin);
await step('deposit-senior', () => invoke(contracts.liquidityPool, 'lp', 'deposit', ['--provider', accounts.lp, '--amount', '500000000000']), accounts.lp);
await step('deposit-junior', () => invoke(contracts.liquidityPool, 'lp', 'deposit_junior', ['--provider', accounts.lp, '--amount', '200000000000']), accounts.lp);
if (!progress.maturity) { progress.maturity = Math.floor(Date.now() / 1000) + 86400 * 60; save(); }
const invoiceId = await step('mint-invoice', () => invoke(contracts.invoiceRegistry, 'merchant', 'mint_invoice', ['--merchant', accounts.merchant, '--debtor', accounts.debtor, '--currency', token, '--face_value', '100000000000', '--maturity_timestamp', String(progress.maturity), '--metadata_hash', sha256(`LiquiVault testnet proof ${runId}`), '--invoice_number', `PROOF-${runId}`, '--risk_tier', '1']), accounts.merchant);
if (!Object.hasOwn(progress.steps, 'verify-invoice')) assert.equal(read(contracts.invoiceRegistry, 'get_invoice', ['--invoice_id', String(invoiceId)]).status, 0);
await step('verify-invoice', () => invoke(contracts.invoiceRegistry, 'verifier', 'verify_invoice', ['--caller', accounts.verifier, '--invoice_id', String(invoiceId), '--risk_tier', '1']), accounts.verifier);
if (!Object.hasOwn(progress.steps, 'factor-invoice')) assert.equal(read(contracts.invoiceRegistry, 'get_invoice', ['--invoice_id', String(invoiceId)]).status, 1);
const quote = await step('factor-invoice', () => invoke(contracts.factoringEscrow, 'merchant', 'factor_invoice', ['--merchant', accounts.merchant, '--invoice_id', String(invoiceId)]), accounts.merchant);
const merchantBalance = read(token, 'balance', ['--id', accounts.merchant]);
assert.equal(BigInt(merchantBalance), BigInt(quote.net_payout));
if (!Object.hasOwn(progress.steps, 'settle-invoice')) assert.equal(read(contracts.invoiceRegistry, 'get_invoice', ['--invoice_id', String(invoiceId)]).status, 2);
const settlement = await step('settle-invoice', () => invoke(contracts.factoringEscrow, 'debtor', 'settle_invoice', ['--payer', accounts.debtor, '--invoice_id', String(invoiceId)]), accounts.debtor);
const pool = read(contracts.liquidityPool, 'get_pool_state');
const junior = read(contracts.liquidityPool, 'get_junior_state');
const invoice = read(contracts.invoiceRegistry, 'get_invoice', ['--invoice_id', String(invoiceId)]);
const gross = BigInt(quote.total_discount);
const cut = gross * 1000n / 10000n;
const lpYield = gross - cut;
const juniorBps = 200000000000n * 10000n / 700000000000n;
const juniorYield = lpYield * juniorBps / 10000n;
assert.equal(invoice.status, 3);
assert.equal(BigInt(pool.deployed_capital), 0n);
assert.equal(BigInt(settlement[0]), lpYield);
assert.equal(BigInt(settlement[1]), cut);
assert.equal(BigInt(read(token, 'balance', ['--id', accounts.treasury])), cut);
assert.equal(BigInt(pool.total_assets), 500000000000n + lpYield - juniorYield);
assert.equal(BigInt(junior.total_assets), 200000000000n + juniorYield);
assert.equal(BigInt(read(contracts.liquidityPool, 'get_junior_yield_bps')), 3000n);

const record = {
  network: 'testnet', deployedAt: new Date().toISOString(), runId,
  provenance: { contractSourceCommit: execute('git', ['log', '-1', '--format=%H', '--', 'contracts', 'Cargo.toml', 'Cargo.lock']).stdout,
    sourceDigest, sourceFiles, wasmSha256: wasmHashes, stellarCli: execute(cli, ['--version']).stdout,
    rust: execute('rustc', ['--version']).stdout },
  contracts, testUsdcAsset: asset,
  accounts: { admin: accounts.admin, verifier: accounts.verifier, treasury: accounts.treasury, merchant: accounts.merchant, debtor: accounts.debtor, liquidityProvider: accounts.lp },
  transactions: progress.transactions,
  smokeTest: { invoiceId: Number(invoiceId), factoringQuote: quote, merchantUsdcAfterFactoring: String(merchantBalance),
    settlement_lpYield_protocolCut: settlement, invoiceAfterSettlement: invoice, poolState: pool, juniorState: junior,
    effectiveJuniorYieldBps: Number(juniorBps), assertionsPassed: true }
};
const output = join(root, 'deployments', 'testnet.json');
mkdirSync(dirname(output), { recursive: true });
if (existsSync(output)) {
  const old = JSON.parse(readFileSync(output, 'utf8'));
  const history = join(root, 'deployments', `testnet-${old.deployedAt.slice(0, 10)}.json`);
  if (!existsSync(history)) writeFileSync(history, JSON.stringify(old, null, 2) + '\n');
}
writeFileSync(output, JSON.stringify(record, null, 2) + '\n');
console.log(`Verified signed testnet proof: ${output}`);
