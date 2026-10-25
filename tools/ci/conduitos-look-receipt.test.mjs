import { test } from 'node:test';
import assert from 'node:assert/strict';
import { validateNativeLookReceipt } from './conduitos-look-receipt.mjs';
import { NATIVE_LOOK_MATRIX } from './conduitos-look-matrix.mjs';
import { readFile } from 'node:fs/promises';
import { lookFixture } from '../../proof/ci/conduitos-look-fixture.mjs';

const serialTransition = ({ screenshot_checkpoint, ...record }) => `CONDUIT_NATIVE_COMPONENT_REALIZATION ${JSON.stringify(record)}\n`;

const specimen = change => lookFixture({
  commit: 'a'.repeat(40), image: 'b'.repeat(64), host: 'host', boot: 'boot',
}, change);
const validate = ({ proof, manifest, serial }) => validateNativeLookReceipt(proof, manifest, serial);

test('ten enacted Looks and four captured checkpoints retain Face and return to the original Mask plan', () => {
  assert.doesNotThrow(() => validate(specimen()));
});

test('receipt cannot reorder, omit or invent serial realizations', () => {
  for (const mutate of [
    ({ proof }) => proof.look_selection.transitions.reverse(),
    ({ proof }) => proof.look_selection.transitions.pop(),
    ({ proof }) => { proof.look_selection.transitions[1].configuration.recipe_canonical_sha256 = '0'.repeat(64); },
    ({ proof }) => { proof.look_selection.transitions[0].boot_id = 'foreign-boot'; },
    ({ chunks }) => chunks[2].pop(),
    ({ chunks }) => chunks[2].push(chunks[1][1]),
  ]) assert.throws(() => validate(specimen(mutate)));
});

test('self-consistent forged receipts still cannot change style, authority, Face or Show custody', () => {
  for (const mutate of [
    transition => { transition.owner_show_acknowledged = true; },
    transition => { transition.local_scanout_acknowledged = false; },
    transition => { transition.previous_show_id = 'stale-show'; },
    transition => { transition.mask_plan_id = 'foreign-plan'; },
    transition => { transition.expression.expression.palette = 'engineering'; },
    transition => { transition.expression.expression.typography = 'engineering'; },
    transition => { transition.configuration.layout_canonical_sha256 = '0'.repeat(64); },
    transition => { transition.configuration.component.fore_id = '0'.repeat(64); },
    transition => { transition.face_revision = 2; },
    transition => { transition.render.commands = 49; },
  ]) {
    assert.throws(() => validate(specimen(({ proof, chunks }) => {
      mutate(proof.look_selection.transitions[1]);
      chunks[2][1] = serialTransition(proof.look_selection.transitions[1]);
    })));
  }
});

test('checkpoint prefixes cannot borrow later publications or hide Body actions', () => {
  for (const mutate of [
    ({ manifest }) => { [manifest.checkpoints[1], manifest.checkpoints[2]] = [manifest.checkpoints[2], manifest.checkpoints[1]]; },
    ({ manifest }) => { manifest.checkpoints[1].serial_byte_end = 1; },
    ({ manifest }) => { manifest.checkpoints[2].native_receipts.latest_acknowledged_face.show.show_id = 'stale'; },
    ({ manifest }) => { manifest.checkpoints[2].guest_boot_record.image_binding = 'another-image'; },
    ({ chunks }) => { chunks[2].push('CONDUIT_NATIVE_BIRTH {"accepted_effects":1}\n'); },
    ({ chunks }) => { chunks[2].push('CONDUIT_NATIVE_ACTION {"accepted_effects":1}\n'); },
    ({ proof }) => { proof.look_selection.body_actions = 1; },
    ({ proof }) => { proof.look_selection.physical_evidence = true; },
    ({ proof }) => { proof.source_commit = '0'.repeat(40); },
    ({ proof }) => { proof.look_selection.source_commit = '0'.repeat(40); },
    ({ proof }) => { proof.look_selection.image_sha256 = '0'.repeat(64); },
    ({ native, chunks }) => {
      native[2].face.text = ['changed semantic meaning'];
      chunks[2][0] = `CONDUIT_NATIVE_FACE ${JSON.stringify(native[2])}\n`;
    },
  ]) assert.throws(() => validate(specimen(mutate)));
});


test('uncaptured transitions and restores cannot escape verification', () => {
  for (const index of [1, 2, 3, 6, 8, 9]) {
    assert.throws(() => validate(specimen(({ proof, chunks }) => {
      proof.look_selection.transitions[index].expression.expression.typography = 'foreign';
      chunks[index + 1][1] = serialTransition(proof.look_selection.transitions[index]);
    })));
  }
  assert.throws(() => validate(specimen(({ proof }) => {
    proof.look_selection.transitions[1].screenshot_checkpoint = 'invented-capture';
  })));
});

test('later Body activity is not attributed to the earlier zero-Body Look matrix', () => {
  assert.doesNotThrow(() => validate(specimen(({ chunks }) => {
    chunks.at(-1).push('CONDUIT_NATIVE_BIRTH {"accepted_effects":1}\n');
    chunks.at(-1).push('CONDUIT_NATIVE_ACTION {"accepted_effects":1}\n');
  })));
});

test('publisher matrix and representative checkpoints match the actual Rust journey producer', async () => {
  const source = await readFile(new URL('../../targets/conduitos/make/xtask/journey_proof/look_selection.rs', import.meta.url), 'utf8');
  const tuples = [...source.matchAll(/\(\s*"(f[678])",\s*"(compact|spacious)",\s*"(editorial|phosphor)",\s*(100|125),\s*"(editorial|engineering)",\s*"(creche-[^"]+)",\s*\)/g)]
    .map(match => [match[1], match[2], match[3], Number(match[4]), match[5], match[6]]);
  assert.deepEqual(tuples, NATIVE_LOOK_MATRIX.map(row => row.slice(0, 6)));
  const captures = source.slice(source.indexOf('let captured = matches!(')).split(');')[0];
  const names = [...captures.matchAll(/"(creche-[^"]+)"/g)].map(match => match[1]);
  assert.deepEqual(names, NATIVE_LOOK_MATRIX.filter(row => row[6]).map(row => row[5]));
  const stager = await readFile(new URL('./stage-conduitos-pages-evidence.mjs', import.meta.url), 'utf8');
  const staged = [...stager.matchAll(/^  step\('([^']+)'/gm)].map(match => match[1]);
  assert.deepEqual(staged.slice(0, 6), ['front-door-ready', 'creche-ready', ...names]);
});
