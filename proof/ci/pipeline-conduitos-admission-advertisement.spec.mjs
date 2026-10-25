import assert from 'node:assert/strict';
import test from 'node:test';
import { readFile } from 'node:fs/promises';
import { deflateSync } from 'node:zlib';
import { parseConduitOsSerialJoin } from '../../targets/conduitos/deployment/browser/image.mjs';

const advertisement = { protocol_version: 1, host_id: 'host/native', boot_id: 'boot/native', offer_generation: 1, profile: 'profile/native', resources: [], capabilities: [{ semantic_contract: { configuration: [{ default_value: { Structured: { profile: 'exact/profile', canonical_value: [0, 128, 255] } } }] } }], planner_capabilities: [] };
const prepared = { spore_id: 'spore/one', image_id: 'image/one', invitation_id: 'invitation/one', body_id: 'body/one', invitation_nonce: Array(32).fill(1) };
function encoded(bytes = Buffer.from(JSON.stringify(advertisement))) {
  return { encoding: 'conduit.body/admission-advertisement-deflate-json@1', expanded_bytes: bytes.length, zlib_hex: deflateSync(bytes).toString('hex') };
}
function observation(compact = true) {
  return { schema: `conduit.conduitos/serial-spawn-observation@${compact ? 2 : 1}`, protocol: compact ? 2 : 1, ...prepared, invitation_nonce: undefined, host_id: advertisement.host_id, boot_id: advertisement.boot_id, advertisement: compact ? encoded() : structuredClone(advertisement), nonce: prepared.invitation_nonce, signature: Array(64).fill(2), membership_claimed: false, expiry_checked_by_body: true };
}
const parse = join => parseConduitOsSerialJoin(`CONDUIT_SPORE_JOIN ${JSON.stringify(join)}\n`, { prepared });

test('serial legacy and version-two preserve the entire public advertisement JSON', async () => {
  for (const compact of [false, true]) {
    const value = await parse(observation(compact));
    assert.deepEqual(value.advertisement, advertisement);
    assert.deepEqual(value.signature, Array(64).fill(2));
    assert.equal(value.membership_claimed, false);
  }
});
for (const [name, mutate] of [
  ['unknown encoding', x => { x.encoding = 'unknown'; }],
  ['unknown field', x => { x.extra = true; }],
  ['zero extent', x => { x.expanded_bytes = 0; }],
  ['oversized extent', x => { x.expanded_bytes = 65537; }],
  ['short declared extent', x => { x.expanded_bytes -= 1; }],
  ['long declared extent', x => { x.expanded_bytes += 1; }],
  ['bad hex', x => { x.zlib_hex = 'zz'; }],
  ['odd hex', x => { x.zlib_hex += '0'; }],
  ['truncated stream', x => { x.zlib_hex = x.zlib_hex.slice(0, -2); }],
  ['trailing data', x => { x.zlib_hex += '00'; }],
  ['concatenated streams', x => { x.zlib_hex += x.zlib_hex; }],
  ['compressed capacity', x => { x.zlib_hex = '00'.repeat(24577); }],
]) {
  test(`serial compact refuses ${name}`, async () => {
    const join = observation(); mutate(join.advertisement);
    await assert.rejects(parse(join), { code: 'BootObservationMalformed' });
  });
}
test('serial compact stops a valid compressed expansion bomb at the declared extent', async () => {
  const join = observation(); join.advertisement = encoded(Buffer.alloc(65537, 32)); join.advertisement.expanded_bytes = 65536;
  await assert.rejects(parse(join), { code: 'BootObservationMalformed' });
});
test('serial compact retains exact Host, nonce and nonmembership refusals after reconstruction', async () => {
  for (const mutate of [x => { x.host_id = 'host/other'; }, x => { x.nonce = Array(32).fill(3); }, x => { x.membership_claimed = true; }]) {
    const join = observation(); mutate(join); await assert.rejects(parse(join), { code: 'BootObservationStale' });
  }
});

test('serial join carrier refuses oversized UTF8 before parsing or decompression', async () => {
  const join = observation(); join.extra = 'é'.repeat(25000);
  await assert.rejects(parse(join), { code: 'BootObservationMalformed' });
});
test('serial schema selects legacy raw versus version-two compact advertisement', async () => {
  const mixedLegacy = observation(); mixedLegacy.schema = 'conduit.conduitos/serial-spawn-observation@1'; mixedLegacy.protocol = 1;
  await assert.rejects(parse(mixedLegacy), { code: 'BootObservationStale' });
  const mixedCompact = observation(false); mixedCompact.schema = 'conduit.conduitos/serial-spawn-observation@2'; mixedCompact.protocol = 2;
  await assert.rejects(parse(mixedCompact), { code: 'BootObservationMalformed' });
});

test('browser refuses numeric u64 generation outside exact JS integer range', async () => {
  const bytes = Buffer.from(JSON.stringify(advertisement).replace('"offer_generation":1', '"offer_generation":18446744073709551615'));
  const join = observation(); join.advertisement = encoded(bytes);
  await assert.rejects(parse(join), { code: 'BootObservationMalformed' });
  const legacy = JSON.stringify(observation(false)).replace('"offer_generation":1', '"offer_generation":18446744073709551615');
  await assert.rejects(parseConduitOsSerialJoin(`CONDUIT_SPORE_JOIN ${legacy}\n`, { prepared }), { code: 'BootObservationMalformed' });
});

// The product copies this adapter under its workspace deployment directory.
// Resolve only those deployment import locations for this source-level Node
// interleaving test; the adapter and all imported production functions are real.
async function loadSourceAdapter() {
  const url = new URL('../../targets/conduitos/deployment/browser/creche-adapter.mjs', import.meta.url);
  let source = await readFile(url, 'utf8');
  for (const [specifier, target] of [
    ['../../../creche-spore-bundle.mjs', '../../targets/browser/workspace/creche-spore-bundle.mjs'],
    ['../../../creche-native-disk.mjs', '../../targets/browser/workspace/creche-native-disk.mjs'],
    ['./image.mjs', '../../targets/conduitos/deployment/browser/image.mjs'],
  ]) source = source.replace(JSON.stringify(specifier), JSON.stringify(new URL(target, import.meta.url).href));
  source = source.replaceAll('import.meta.url', JSON.stringify(url.href));
  return import(`data:text/javascript;base64,${Buffer.from(source).toString('base64')}`);
}

for (const action of ['cancel', 'abort', 'replace-loader']) {
  test(`Crèche refuses a completed held decode after ${action}`, async () => {
    const { createConduitOsAdapter, CONDUITOS_X86_64_PROFILE: profile } = await loadSourceAdapter();
    const binding = { prepared: { ...prepared, image_content_digest: 'sha256:image' }, nativeSpore: { content_digest: 'sha256:artifact', bytes: new Uint8Array(1) } };
    const loaderReceipt = { loader_id: 'fixture/vm', target_id: profile.target.id, machine: profile.machine, architecture: profile.architecture, image_content_digest: 'sha256:image', artifact_sha256: 'sha256:artifact', artifact_bytes: 1, carrier: 'downloadable-disk-image', explicit_authority: true, load_completed: true, serial_output: `CONDUIT_SPORE_JOIN ${JSON.stringify(observation())}\n` };
    const adapter = createConduitOsAdapter({ profile, loader: async () => loaderReceipt });
    await adapter.realize({ mode: 'make-new', binding });
    let enteredResolve; let release;
    const entered = new Promise(resolve => { enteredResolve = resolve; });
    const gate = new Promise(resolve => { release = resolve; });
    const Original = globalThis.DecompressionStream;
    globalThis.DecompressionStream = class HeldDecompressionStream {
      constructor(format) {
        const decoder = new Original(format);
        return { writable: decoder.writable, readable: decoder.readable.pipeThrough(new TransformStream({ async transform(chunk, controller) { enteredResolve(); await gate; controller.enqueue(chunk); } })) };
      }
    };
    try {
      const controller = new AbortController();
      const pending = adapter.observe({ mode: 'make-new', binding, signal: controller.signal });
      await entered;
      if (action === 'cancel') await adapter.cancel({ mode: 'make-new', operation: 'observe' });
      if (action === 'abort') controller.abort();
      if (action === 'replace-loader') await adapter.realize({ mode: 'make-new', binding });
      release();
      await assert.rejects(pending, { code: 'Cancelled' });
    } finally { release(); globalThis.DecompressionStream = Original; }
  });
}

for (const token of ['1e-400', '9007199254740990.1', '1e0', '1.0', '-0']) {
  test(`browser refuses noncanonical integer numeric token ${token} before JSON rounding`, async () => {
    const raw = JSON.stringify(advertisement).replace('"offer_generation":1', `"offer_generation":${token}`);
    const join = observation(); join.advertisement = encoded(Buffer.from(raw));
    await assert.rejects(parse(join), { code: 'BootObservationMalformed' });
    const legacy = JSON.stringify(observation(false)).replace('"offer_generation":1', `"offer_generation":${token}`);
    await assert.rejects(parseConduitOsSerialJoin(`CONDUIT_SPORE_JOIN ${legacy}\n`, { prepared }), { code:'BootObservationMalformed' });
  });
}

test('actual Rust product advertisement survives compact browser custody byte for byte', async () => {
  const raw = await readFile(new URL('../fixtures/native-admission/product-advertisement.json', import.meta.url), 'utf8');
  const expected = JSON.parse(raw);
  const join = observation(); join.host_id = expected.host_id; join.boot_id = expected.boot_id;
  join.advertisement = encoded(Buffer.from(raw));
  const decoded = await parse(join);
  assert.equal(JSON.stringify(decoded.advertisement), raw);
  assert.equal(Buffer.byteLength(raw), 51780);
  assert.equal(decoded.advertisement.offer_generation, 1);
  const highGeneration = raw.replace('"offer_generation":1,', '"offer_generation":18446744073709551615,');
  assert.notEqual(highGeneration, raw);
  join.advertisement = encoded(Buffer.from(highGeneration));
  await assert.rejects(parse(join), { code: 'BootObservationMalformed' });
});
