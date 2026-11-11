import { readdirSync, readFileSync, existsSync } from 'node:fs';
import { resolve, dirname, extname, relative } from 'node:path';
const root = resolve(process.cwd());
const ignored = new Set(['.git', 'node_modules', 'target', '.next', 'dist', '.stellar']);
function files(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap(entry => {
    const path = resolve(directory, entry.name);
    if (ignored.has(entry.name)) return [];
    if (entry.isDirectory()) return existsSync(resolve(path, '.git')) ? [] : files(path);
    return extname(path) === '.md' ? [path] : [];
  });
}
const problems = [];
const documents = files(root);
for (const file of documents) {
  const text = new TextDecoder('utf-8', { fatal: true }).decode(readFileSync(file));
  if (/\uFFFD|\u00e2\u20ac|\u00c3\u00a9/.test(text)) problems.push(`${relative(root, file)}: corrupted text encoding`);
  for (const match of text.matchAll(/\]\(([^)]+)\)/g)) {
    const href = match[1].replace(/^<|>$/g, '').split(/\s+"/)[0];
    if (/^[a-z][a-z0-9+.-]*:/i.test(href) || href.startsWith('//') || href.startsWith('#')) continue;
    const path = decodeURIComponent(href.split('#')[0]);
    if (!path) continue;
    const target = resolve(dirname(file), path);
    if (relative(root, target).startsWith('..') || !existsSync(target)) problems.push(`${relative(root, file)}: invalid standalone link ${href}`);
  }
}
if (problems.length) { console.error(problems.join('\n')); process.exitCode = 1; }
else console.log(`Checked UTF-8 and relative links in ${documents.length} Markdown files.`);
