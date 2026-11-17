import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';

const GATED = new Set(['high', 'critical']);
const accepted = JSON.parse(readFileSync(new URL('../audit-accepted.json', import.meta.url), 'utf8'));

function report() {
  try {
    return execFileSync('npm', ['audit', '--json'], { encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
  } catch (error) {
    if (typeof error.stdout === 'string' && error.stdout) return error.stdout;
    throw error;
  }
}

const audit = JSON.parse(report());
if (!audit.vulnerabilities) {
  console.error(JSON.stringify(audit.error ?? audit, null, 2));
  process.exit(1);
}

const advisories = new Map();
for (const entry of Object.values(audit.vulnerabilities)) {
  for (const via of entry.via) {
    if (typeof via !== 'object' || !GATED.has(via.severity)) continue;
    const id = via.url.split('/').pop();
    advisories.set(id, `${via.severity} ${via.name} ${via.range}: ${via.title} (${via.url})`);
  }
}

const blocking = [];
for (const [id, line] of advisories) {
  if (Object.hasOwn(accepted, id)) console.log(`accepted ${line}`);
  else blocking.push(line);
}
for (const id of Object.keys(accepted)) {
  if (!advisories.has(id))
    console.log(`::warning::${id} is no longer reported; drop it from audit-accepted.json`);
}

if (blocking.length > 0) {
  for (const line of blocking) console.error(line);
  console.error(`${blocking.length} high or critical advisories are not in audit-accepted.json`);
  process.exit(1);
}
console.log('No high or critical advisory outside audit-accepted.json');
