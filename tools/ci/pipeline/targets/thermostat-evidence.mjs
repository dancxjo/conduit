import { copyFileSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import path from 'node:path';

// Retain executed local Body evidence with its exact sealed runtime and source.
export function retainThermostatEvidence(results, destination, runtime, sourceHead) {
  if (!/^[a-f0-9]{40}$/.test(sourceHead ?? '')) throw new Error('Thermostat proof requires exact source head');
  const reports = readdirSync(results, { recursive: true })
    .filter(name => path.basename(name) === 'thermostat-runtime-evidence.json');
  if (reports.length !== 1) throw new Error('Expected one current local Thermostat browser proof');
  const source = path.dirname(path.join(results, reports[0]));
  const bytes = readFileSync(path.join(source, 'thermostat-runtime-evidence.json'));
  if (bytes.length > 16 * 1024 * 1024) throw new Error('Thermostat proof exceeds its capture bound');
  const proof = JSON.parse(bytes);
  const digest = createHash('sha256').update(readFileSync(runtime)).digest('hex');
  if (proof.schema !== 'conduit.browser/thermostat-local-proof@1'
      || proof.source_head !== sourceHead || proof.runtime_sha256 !== digest
      || !proof.browser?.includes('Chrome/')
      || proof.accessibility?.violations?.length !== 0 || !(proof.accessibility.passes > 0)
      || proof.source !== readFileSync('plots/thermostat/main.conduit', 'utf8')
      || !proof.initial?.body_plan_id || !proof.initial?.active_play_id
      || proof.selectedBack?.implementation_id !== 'browser/kernel-thermostat-combine@1'
      || proof.recovered?.bodyId !== proof.awake?.bodyId
      || proof.independent?.bodyId === proof.recovered?.bodyId
      || proof.exhausted?.presentation?.revision !== 256
      || ['stale', 'forged', 'show_id', 'face_id', 'range', 'bound'].some(key => proof[key]?.accepted !== false)) {
    throw new Error('Thermostat receipt lacks exact runtime, local execution or refusal evidence');
  }
  mkdirSync(destination);
  for (const name of ['thermostat-runtime-evidence.json', '01-thermostat-born.png',
    '02-thermostat-limit.png', '03-thermostat-patchbay.png']) copyFileSync(path.join(source, name), path.join(destination, name));
  writeFileSync(path.join(destination, 'identity.json'), `${JSON.stringify({
    schema: 'conduit.browser/thermostat-proof-identity@1', source_head: sourceHead,
    runtime_sha256: digest, receipt_sha256: createHash('sha256').update(bytes).digest('hex'),
  }, null, 2)}\n`);
}
