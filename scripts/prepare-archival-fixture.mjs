// Generates an isolated, explicitly instrumented workspace. Canonical sources are never changed.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { copyFileSync, cpSync, existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';
const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const runId = process.argv[2];
assert.match(runId ?? '', /^[a-zA-Z0-9_-]{1,40}$/);
const image = process.env.LIQUIVAULT_QUICKSTART_IMAGE ?? 'stellar/quickstart@sha256:4c8bad1ef7341205b898f83d9489321da80c7bd74183100fc8e2a39a5938c7d5';
const workspace = join(root, '.stellar', 'archival', runId);
assert.ok(!existsSync(workspace), 'Use a fresh fixture run ID; existing evidence is never overwritten');
const settings = spawnSync('docker', ['run', '--rm', '--entrypoint', '/bin/cat', image,
  '/opt/stellar-default/local/core/etc/config-settings/p27/testnet.json'], { encoding: 'utf8', windowsHide: true, timeout: 120000 });
assert.equal(settings.status, 0, 'Cannot read compatible settings from the selected Quickstart image');
const configuration = JSON.parse(settings.stdout);
const archival = configuration.updated_entry.find(entry => entry.state_archival)?.state_archival;
assert.ok(archival);
archival.min_persistent_ttl = 256;
archival.min_temporary_ttl = 16;
mkdirSync(workspace, { recursive: true });
const fixture = join(workspace, 'fixture');
mkdirSync(join(fixture, 'scripts'), { recursive: true });
for (const file of ['Cargo.toml', 'Cargo.lock']) copyFileSync(join(root, file), join(fixture, file));
copyFileSync(join(root, 'scripts', 'deploy-testnet.mjs'), join(fixture, 'scripts', 'deploy-testnet.mjs'));
const patches = [];
for (const name of ['invoice_registry', 'liquidity_pool', 'factoring_escrow']) {
  const target = join(fixture, 'contracts', name);
  mkdirSync(target, { recursive: true });
  copyFileSync(join(root, 'contracts', name, 'Cargo.toml'), join(target, 'Cargo.toml'));
  cpSync(join(root, 'contracts', name, 'src'), join(target, 'src'), { recursive: true });
  const path = join(target, 'src', 'lib.rs');
  let source = readFileSync(path, 'utf8');
  for (const [constant, value] of [['INSTANCE_LIFETIME_THRESHOLD', 32], ['INSTANCE_BUMP_AMOUNT', 256], ['PERSISTENT_LIFETIME_THRESHOLD', 32], ['PERSISTENT_BUMP_AMOUNT', 256], ...(name === 'liquidity_pool' ? [['WITHDRAWAL_LOCK_SECONDS', 1]] : [])]) {
    const expression = new RegExp(`(const ${constant}: u(?:32|64) = )([0-9_]+)(;)`);
    const match = source.match(expression);
    assert.ok(match, `Missing fixture constant ${name}/${constant}`);
    patches.push({ contract: name, constant, original: Number(match[2].replaceAll('_', '')), fixture: value });
    source = source.replace(expression, `$1${value}$3`);
  }
  writeFileSync(path, source);
}
writeFileSync(join(workspace, 'short-ttl.json'), JSON.stringify(configuration, null, 2) + '\n');
const base = spawnSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8', windowsHide: true });
assert.equal(base.status, 0, 'Cannot establish the base source commit');
assert.match(base.stdout.trim(), /^[0-9a-f]{40}$/);
const manifest = { schemaVersion: 1, scope: 'instrumented-standalone-only', baseContractCommit: base.stdout.trim(),
  image, protocolVersion: 28, settingsTemplateProtocol: 27, canonicalSourcesModified: false, patches,
  settingsSha256: createHash('sha256').update(JSON.stringify(configuration)).digest('hex') };
writeFileSync(join(workspace, 'fixture-manifest.json'), JSON.stringify(manifest, null, 2) + '\n');
console.log(JSON.stringify({ workspace, fixture, settings: join(workspace, 'short-ttl.json'), manifest }, null, 2));
