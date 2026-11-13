// Explicit opt-in: creates and stops only its own localhost-only Docker container.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';
const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const runId = process.argv[2];
assert.match(runId ?? '', /^[a-zA-Z0-9_-]{1,40}$/);
const sdkRoot = resolve(root, '..', 'liquivault-sdk');
assert.ok(existsSync(join(sdkRoot, 'dist', 'index.mjs')), 'Build the sibling SDK first');
const require = createRequire(join(sdkRoot, 'package.json'));
const { rpc, Keypair, hash, Asset, Operation, TransactionBuilder } = require('@stellar/stellar-sdk');
async function run(command, args, cwd = root) {
  const child = spawn(command, args, { cwd, stdio: ['ignore', 'pipe', 'pipe'], windowsHide: true });
  child.stdout.pipe(process.stdout); child.stderr.pipe(process.stderr);
  await new Promise((resolve, reject) => {
    child.once('error', reject);
    child.once('close', code => code === 0 ? resolve() : reject(new Error(`${command} exited ${code}`)));
  });
}
const server = new rpc.Server('http://127.0.0.1:18000/rpc', { allowHttp: true });
try {
  await server.getNetwork();
  throw new Error('Local port 18000 already serves a network. Stop your previous test container explicitly; this runner will not replace it.');
} catch (error) {
  if (error.message?.startsWith('Local port')) throw error;
}
await run(process.execPath, [join(root, 'scripts', 'prepare-archival-fixture.mjs'), runId]);
const workspace = join(root, '.stellar', 'archival', runId);
const manifest = JSON.parse(readFileSync(join(workspace, 'fixture-manifest.json'), 'utf8'));
const container = `liquivault-archival-${runId}`;
let started = false;
try {
  await run('docker', ['run', '--detach', '--name', container, '--cpus', '2', '--memory', '3g',
    '--publish', '127.0.0.1:18000:8000', '--mount',
    `type=bind,source=${join(workspace, 'short-ttl.json')},target=/opt/stellar-default/local/core/etc/config-settings/p28/short-ttl.json,readonly`,
    manifest.image, '--local', '--protocol-version', '28', '--limits', 'short-ttl']);
  started = true;
  const passphrase = 'Standalone Network ; February 2017';
  let ready = false;
  for (let attempt = 0; attempt < 180; attempt++) {
    try {
      const network = await server.getNetwork();
      assert.equal(network.passphrase, passphrase); assert.equal(network.protocolVersion, 28);
      assert.equal((await server.getHealth()).status, 'healthy');
      const address = Keypair.fromRawEd25519Seed(hash(Buffer.from(passphrase))).publicKey();
      const tx = new TransactionBuilder(await server.getAccount(address), { fee: '100', networkPassphrase: passphrase })
        .addOperation(Operation.createStellarAssetContract({ asset: new Asset('LVPROBE', address) })).setTimeout(60).build();
      assert.ok(rpc.Api.isSimulationSuccess(await server.simulateTransaction(tx)), 'Host simulation is not ready');
      ready = true; break;
    } catch { /* Warm-up is not a passing integration result. */ }
    if (attempt % 15 === 0) console.log('Waiting for supported protocol, initial history and RPC simulation readiness');
    await new Promise(resolve => setTimeout(resolve, 1000));
  }
  assert.ok(ready, 'Local network did not become ready');
  await run(process.execPath, ['scripts/deploy-testnet.mjs', 'seed', '--local'], join(workspace, 'fixture'));
  await run(process.execPath, [join(sdkRoot, 'examples', 'archival-recovery.mjs'), workspace], sdkRoot);
  console.log(`Confirmed instrumented recovery receipt: ${join(workspace, 'recovery-state.json')}`);
} finally {
  if (started) await run('docker', ['stop', container]);
}
