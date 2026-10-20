import { copyFileSync, lstatSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import path from 'node:path';

// Retain successful source/Mask correspondence and actual Chromium interaction
// traces. CI's successful pinned browser invocation owns the test verdict.
export function retainPortableFaceEvidence(results, destination, sourceSha) {
  if (!/^[0-9a-f]{40}$/.test(sourceSha ?? '')) throw new Error('Portable control evidence needs its exact source');
  const names = readdirSync(results, { recursive: true });
  const captures = names.filter(name => path.basename(name) === 'portable-mask-correspondence.json');
  if (captures.length !== 1) throw new Error('Expected one portable Mask correspondence receipt');
  const capture = path.join(results, captures[0]);
  if (!lstatSync(capture).isFile() || lstatSync(capture).size > 2 * 1024 * 1024) throw new Error('Portable correspondence bound');
  const proof = JSON.parse(readFileSync(capture, 'utf8'));
  const enhanced = proof.enhanced?.encounter, basic = proof.basic?.encounter;
  if (proof.schema !== 'conduit.browser/portable-mask-correspondence@1'
    || !enhanced?.face?.identity || JSON.stringify(enhanced.face) !== JSON.stringify(basic?.face)
    || enhanced.view?.mask_plan_id === basic.view?.mask_plan_id || enhanced.view?.show_id === basic.view?.show_id) {
    throw new Error('Portable correspondence changed source or omitted distinct Plans/Shows');
  }
  for (const encounter of [enhanced, basic]) {
    const view = encounter.view;
    if (view.face_id !== encounter.face.identity || view.face_revision !== String(encounter.face.revision)
      || view.show_state !== 'available' || view.interactions_admitted !== true
      || !view.mask_plan_id || !view.mask_play_id || !view.show_id) throw new Error('Portable correspondence lacks an acknowledged Play/Show');
  }
  const placement = encounter => {
    const fragments = encounter.planned_mask?.plan?.fragments;
    if (fragments?.length !== 1 || fragments[0].placements?.length !== 1) throw new Error('Portable selected placement shape');
    return fragments[0].placements[0];
  };
  const selected = placement(enhanced), generic = placement(basic);
  if (selected.kind_id !== 'face/component-mask'
    || selected.implementation_id !== 'implementation/browser-portable-face-component@1'
    || selected.capability_id !== 'capability/browser-portable-face-component@1'
    || !/^artifact\/browser-portable-face-component@1\/sha256\/[0-9a-f]{64}$/.test(selected.artifact_id)
    || generic.kind_id !== 'face/browser-dom-mask'
    || generic.implementation_id !== 'implementation/browser-dom-mask') throw new Error('Portable Back identity receipt differs');
  const bundle = createHash('sha256').update('conduit.browser/portable-face-bundle@1\n');
  for (const role of ['controls', 'model', 'combobox', 'component', 'style']) {
    const file = role === 'style' ? 'portable-face-look.css' : `portable-face-${role}.mjs`;
    for (const bytes of [Buffer.from(role), readFileSync(path.join('targets/browser/host/assets', file))]) {
      const length = Buffer.alloc(4); length.writeUInt32BE(bytes.length); bundle.update(length); bundle.update(bytes);
    }
  }
  const bundleSha256 = bundle.digest('hex');
  if (selected.artifact_id !== `artifact/browser-portable-face-component@1/sha256/${bundleSha256}`) throw new Error('Portable Back differs from current source graph');
  const traces = names.filter(name => path.basename(name) === 'trace.zip'
    && name.split(path.sep).some(part => part.startsWith('portable-face-controls-'))).sort();
  if (traces.length < 10 || traces.length > 64) throw new Error('Portable interaction trace count');
  const files = [[captures[0], 'correspondence.json'], ...traces.map((name, index) => [name, `trace-${index}.zip`])];
  let total = 0;
  for (const [name] of files) {
    const stat = lstatSync(path.join(results, name));
    if (!stat.isFile() || stat.size > 32 * 1024 * 1024) throw new Error('Portable trace resource bound');
    total += stat.size;
  }
  if (total > 64 * 1024 * 1024) throw new Error('Portable evidence bundle bound');
  mkdirSync(destination);
  const retained = files.map(([name, target]) => {
    const file = path.join(results, name); copyFileSync(file, path.join(destination, target));
    return { source: name, path: target, bytes: lstatSync(file).size,
      sha256: createHash('sha256').update(readFileSync(file)).digest('hex') };
  });
  writeFileSync(path.join(destination, 'receipt.json'), JSON.stringify({
    schema: 'conduit.browser/portable-controls-retained-proof@1', source_sha: sourceSha,
    bundle_sha256: bundleSha256,
    scope: 'Chromium browser; source-owned zero-Body Creche encounter; no native or hardware claim',
    files: retained,
  }, null, 2) + '\n');
}
