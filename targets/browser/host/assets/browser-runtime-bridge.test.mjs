import test from "node:test";
import assert from "node:assert/strict";
import { bindBrowserRuntimeBridge } from "./browser-runtime-bridge.mjs";

const ABI = new TextEncoder().encode("conduit.browser/runtime-abi");

function runtime({ revision = 1, identity = ABI, inputCapacity = 64, inputPointer = 0, outputCapacity = 1024, outputPointer = 0 } = {}) {
  const extent = (pointer, capacity) => Number.isSafeInteger(pointer) && Number.isSafeInteger(capacity)
    && pointer >= 0 && capacity >= 0 && pointer + capacity <= 64 * 1024 * 1024
    ? pointer + capacity : 0;
  const requiredBytes = Math.max(1024 + identity.length, extent(inputPointer, inputCapacity), extent(outputPointer, outputCapacity));
  const memory = new WebAssembly.Memory({ initial: Math.ceil(requiredBytes / (64 * 1024)) });
  const state = { workspaceOutputLength: 0, bodyOutputLength: 0, projectionOutputLength: 0 };
  const api = {
    memory,
    conduit_browser_runtime_abi_revision: () => revision,
    conduit_browser_runtime_abi_identity_ptr: () => 1024,
    conduit_browser_runtime_abi_identity_len: () => identity.length,
    conduit_workspace_input_ptr: () => inputPointer,
    conduit_workspace_input_capacity: () => inputCapacity,
    conduit_workspace_output_ptr: () => outputPointer,
    conduit_workspace_output_len: () => state.workspaceOutputLength,
    conduit_workspace_output_capacity: () => outputCapacity,
    conduit_workspace_request: () => 0,
    conduit_creche_input_ptr: () => 0,
    conduit_creche_input_capacity: () => 1024,
    conduit_creche_output_ptr: () => 0,
    conduit_creche_output_len: () => 0,
    conduit_creche_current: () => 1,
    conduit_creche_durable_snapshot: () => 1,
    conduit_creche_attach_here: () => 0,
    conduit_creche_restore_durable: () => 0,
    conduit_browser_body_input_ptr: () => 0,
    conduit_browser_body_input_capacity: () => 1024,
    conduit_browser_body_start: () => 0,
    conduit_browser_plot_input_ptr: () => 0,
    conduit_browser_plot_input_capacity: () => 1024,
    conduit_browser_plot_output_ptr: () => 0,
    conduit_browser_plot_output_len: () => state.bodyOutputLength,
    conduit_browser_plot_acknowledge_cancellation: () => 0,
    conduit_browser_plot_complete_effect: () => 0,
    conduit_browser_plot_refuse_effect: () => 0,
    conduit_browser_projection_input_ptr: () => 4096,
    conduit_browser_projection_input_capacity: () => 1024,
    conduit_browser_projection_output_ptr: () => 8192,
    conduit_browser_projection_output_len: () => state.projectionOutputLength,
    conduit_browser_project_patchbay: () => 0,
    setWorkspaceOutput(bytes) {
      new Uint8Array(memory.buffer, 0, bytes.length).set(bytes);
      state.workspaceOutputLength = bytes.length;
    },
    setWorkspaceOutputLength(length) { state.workspaceOutputLength = length; },
    setBodyOutput(bytes) {
      new Uint8Array(memory.buffer, 0, bytes.length).set(bytes);
      state.bodyOutputLength = bytes.length;
    },
    setProjectionOutput(bytes) {
      new Uint8Array(memory.buffer, 8192, bytes.length).set(bytes);
      state.projectionOutputLength = bytes.length;
    },
  };
  new Uint8Array(memory.buffer, 1024, identity.length).set(identity);
  return api;
}

test("refuses unsupported runtime ABI revision", () => {
  assert.throws(() => bindBrowserRuntimeBridge(runtime({ revision: 2 }), { context: "bridge proof" }), (error) =>
    error?.code === "IncompatibleRuntimeAbi" && error.runtime_abi_revision === 2);
});

test("refuses malformed runtime ABI revision export", () => {
  const api = runtime();
  api.conduit_browser_runtime_abi_revision = () => "broken";
  assert.throws(() => bindBrowserRuntimeBridge(api, { context: "bridge proof" }), /malformed runtime ABI revision export/);
});

test("refuses ABI identity mismatch", () => {
  const api = runtime({ identity: new TextEncoder().encode("wrong/runtime-abi") });
  assert.throws(() => bindBrowserRuntimeBridge(api, { context: "bridge proof" }), (error) =>
    error?.code === "IncompatibleRuntimeAbi" && error.runtime_abi_revision === null);
});

test("workspace request retires input even when invocation grows memory", () => {
  const api = runtime({ inputPointer: 256 });
  api.conduit_workspace_request = (length) => {
    api.memory.grow(1);
    api.setWorkspaceOutput(new TextEncoder().encode(JSON.stringify({ ok: true })));
    return 0;
  };
  const bridge = bindBrowserRuntimeBridge(api, { context: "bridge proof" });
  bridge.workspaceRequest({ action: "Proof" });
  assert.deepEqual([...new Uint8Array(api.memory.buffer, 256, 16)], new Array(16).fill(0));
});

test("workspace request snapshots a shared input/output arena before retiring input", () => {
  const pointer = 256;
  const api = runtime({ inputPointer: pointer, outputPointer: pointer });
  const expected = { schema: "conduit.workspace/proof@1", state: "admitted" };
  api.conduit_workspace_request = () => {
    const bytes = new TextEncoder().encode(JSON.stringify(expected));
    new Uint8Array(api.memory.buffer, pointer, bytes.length).set(bytes);
    api.setWorkspaceOutputLength(bytes.length);
    return 0;
  };
  const bridge = bindBrowserRuntimeBridge(api, { context: "bridge proof" });
  const result = bridge.workspaceRequest({ action: "Proof" });
  assert.deepEqual(result.outputJson, expected);
  assert.deepEqual([...new Uint8Array(api.memory.buffer, pointer, 16)], new Array(16).fill(0));
});

test("owner Face Mask bridge forwards exact u64 response bytes into WASM", () => {
  const api = runtime();
  const inputPointer = 2048;
  const outputPointer = 32768;
  let outputLength = 0;
  let observedFrame = "";
  api.conduit_browser_owner_face_input_ptr = () => inputPointer;
  api.conduit_browser_owner_face_input_capacity = () => 16 * 1024;
  api.conduit_browser_owner_face_output_ptr = () => outputPointer;
  api.conduit_browser_owner_face_output_capacity = () => 64 * 1024;
  api.conduit_browser_owner_face_output_len = () => outputLength;
  api.conduit_browser_owner_face_prepare = (basisLength, frameLength) => {
    const bytes = new Uint8Array(api.memory.buffer, inputPointer + basisLength, frameLength);
    observedFrame = new TextDecoder().decode(bytes);
    const output = new TextEncoder().encode(JSON.stringify({ schema: "conduit.browser/owner-face-mask@1", face_revision: "9007199254740993" }));
    new Uint8Array(api.memory.buffer, outputPointer, output.length).set(output);
    outputLength = output.length;
    return 0;
  };
  const bridge = bindBrowserRuntimeBridge(api, { context: "owner Face proof" });
  const rawFrame = new TextEncoder().encode('{"kind":"face-snapshot-response","protocol":1,"response":{"revision":9007199254740993}}');
  const view = bridge.ownerFacePrepare({ body_id: "body/one", host_id: "host/browser", boot_id: "boot/current" }, rawFrame);
  assert.match(observedFrame, /9007199254740993/);
  assert.equal(view.face_revision, "9007199254740993");
  assert.ok(new Uint8Array(api.memory.buffer, inputPointer, rawFrame.length).every(byte => byte === 0));
});

test("owner Face interaction bridge retains exact u64 emission bytes", () => {
  const api = runtime();
  const inputPointer = 2048;
  const outputPointer = 32768;
  const emission = new TextEncoder().encode('{"show":{"face_revision":9007199254740993},"interaction":{"face_revision":9007199254740993}}');
  api.conduit_browser_owner_face_input_ptr = () => inputPointer;
  api.conduit_browser_owner_face_input_capacity = () => 16 * 1024;
  api.conduit_browser_owner_face_output_ptr = () => outputPointer;
  api.conduit_browser_owner_face_output_capacity = () => 64 * 1024;
  api.conduit_browser_owner_face_output_len = () => emission.length;
  api.conduit_browser_owner_face_interact = length => {
    assert.match(new TextDecoder().decode(new Uint8Array(api.memory.buffer, inputPointer, length)), /9007199254740993/);
    new Uint8Array(api.memory.buffer, outputPointer, emission.length).set(emission);
    return 0;
  };
  const bridge = bindBrowserRuntimeBridge(api, { context: "owner action proof" });
  const raw = bridge.ownerFaceInteract({ face_revision: "9007199254740993" });
  assert.deepEqual(raw, emission);
  assert.ok(new Uint8Array(api.memory.buffer, inputPointer, 32).every(byte => byte === 0));
});

test("rejects malformed workspace input capacity", () => {
  const api = runtime();
  api.conduit_workspace_input_capacity = () => Number.NaN;
  const bridge = bindBrowserRuntimeBridge(api, { context: "bridge proof" });
  assert.throws(() => bridge.workspaceRequest({ action: "Proof" }), /Workspace input exceeds its bound/);
});

test("rejects malformed workspace input pointer", () => {
  const api = runtime();
  api.conduit_workspace_input_ptr = () => Number.MAX_SAFE_INTEGER;
  const bridge = bindBrowserRuntimeBridge(api, { context: "bridge proof" });
  assert.throws(() => bridge.workspaceRequest({ action: "Proof" }), /Workspace input exceeds its bound/);
});

test("rejects malformed workspace output capacity", () => {
  const api = runtime({ outputCapacity: Number.NaN });
  const bridge = bindBrowserRuntimeBridge(api, { context: "bridge proof" });
  assert.throws(() => bridge.workspaceRequest({ action: "Proof" }), /Workspace output exceeds its bound/);
});

test("rejects workspace output pointers outside current memory", () => {
  const api = runtime({ outputPointer: Number.MAX_SAFE_INTEGER });
  const bridge = bindBrowserRuntimeBridge(api, { context: "bridge proof" });
  assert.throws(() => bridge.workspaceRequest({ action: "Proof" }), /Workspace output exceeds its bound/);
});

test("rejects malformed workspace output lengths", () => {
  const api = runtime();
  api.conduit_workspace_output_len = () => -1;
  const bridge = bindBrowserRuntimeBridge(api, { context: "bridge proof" });
  assert.throws(() => bridge.workspaceRequest({ action: "Proof" }), /Workspace output exceeds its bound/);
});

test("workspace binary request permits empty output", () => {
  const api = runtime();
  api.conduit_workspace_request = () => 0;
  const bridge = bindBrowserRuntimeBridge(api, { context: "bridge proof" });
  const result = bridge.workspaceRequest({ action: "BinaryProof" });
  assert.equal(result.status, 0);
  assert.equal(result.outputBytes.length, 0);
  assert.equal(result.outputJson, null);
});

test("Patchbay projection uses the product-neutral ABI and retires source bytes", () => {
  const api = runtime();
  const expected = { schema: "conduit.patchbay/checked-plot-projection@1", gears: [], cords: [] };
  let invocation;
  api.conduit_browser_project_patchbay = (length, sequence) => {
    invocation = { length, sequence };
    api.setProjectionOutput(new TextEncoder().encode(JSON.stringify(expected)));
    return 0;
  };
  const bridge = bindBrowserRuntimeBridge(api, { context: "Patchbay projection proof" });
  const result = bridge.projectPatchbay("plot clock {}", 7n);
  assert.deepEqual(result.outputJson, expected);
  assert.equal(invocation.sequence, 7n);
  assert.deepEqual(
    [...new Uint8Array(api.memory.buffer, 4096, invocation.length)],
    new Array(invocation.length).fill(0),
  );
});

test("workspace binary request preserves opaque non-UTF-8 output", () => {
  const api = runtime();
  api.conduit_workspace_request = () => {
    api.setWorkspaceOutput(Uint8Array.of(0xff, 0x00, 0xfe));
    return 0;
  };
  const bridge = bindBrowserRuntimeBridge(api, { context: "bridge proof" });
  const result = bridge.workspaceRequest({ action: "BinaryProof" }, { binary: true });
  assert.deepEqual([...result.outputBytes], [0xff, 0x00, 0xfe]);
  assert.equal(result.outputJson, null);
});

test("workspace request admits a bounded whole-Body snapshot larger than one operation", () => {
  const outputCapacity = 16 * 2 * 1024 * 1024 + 256 * 1024;
  const api = runtime({ outputCapacity });
  const expected = { schema: "conduit.workspace/proof@1", retained: "x".repeat(2 * 1024 * 1024 + 256 * 1024) };
  api.conduit_workspace_request = () => {
    api.setWorkspaceOutput(new TextEncoder().encode(JSON.stringify(expected)));
    return 0;
  };
  const bridge = bindBrowserRuntimeBridge(api, { context: "bridge proof" });
  assert.deepEqual(bridge.workspaceRequest({ action: "Proof" }).outputJson, expected);
});

test("initial workload review preserves its four exact fields and retires source bytes", () => {
  const api = runtime();
  const inputPointer = 256;
  const outputPointer = 2048;
  const inputCapacity = 1024;
  let outputLength = 0;
  let lengths;
  api.conduit_creche_input_ptr = () => inputPointer;
  api.conduit_creche_input_capacity = () => inputCapacity;
  api.conduit_creche_output_ptr = () => outputPointer;
  api.conduit_creche_output_len = () => outputLength;
  api.conduit_creche_review_initial_workload = (...args) => {
    lengths = args;
    const value = { schema: "conduit.creche/plot-workload-review@1", requirements: { kinds: [] } };
    const bytes = new TextEncoder().encode(JSON.stringify(value));
    new Uint8Array(api.memory.buffer, outputPointer, bytes.length).set(bytes);
    outputLength = bytes.length;
    return 0;
  };
  const bridge = bindBrowserRuntimeBridge(api, { context: "Plot review proof" });
  const selected = [{ name: "clock", source_document_id: "sha256:source", checked_plot_id: "sha256:checked" }];
  const result = bridge.crecheReviewInitialWorkload({ host: "host/1", boot: "boot/1", initialPlots: selected, source: "clock {}" });
  assert.equal(lengths.length, 4);
  assert.deepEqual(result.outputJson.requirements.kinds, []);
  assert.deepEqual([...new Uint8Array(api.memory.buffer, inputPointer, lengths.reduce((sum, length) => sum + length, 0))],
    new Array(lengths.reduce((sum, length) => sum + length, 0)).fill(0));
});

test("canonical owner Face observation preserves exact integer bytes and copies output", () => {
  const api = runtime();
  const pointer = 32768;
  const bytes = new TextEncoder().encode('{"face":{"revision":18446744073709551615,"properties":[{"Count":9007199254740993}]}}');
  api.conduit_browser_owner_face_output_ptr = () => pointer;
  api.conduit_browser_owner_face_output_capacity = () => 64 * 1024;
  api.conduit_browser_owner_face_output_len = () => bytes.length;
  api.conduit_browser_owner_face_canonical_snapshot = () => {
    new Uint8Array(api.memory.buffer, pointer, bytes.length).set(bytes);
    return 0;
  };
  const bridge = bindBrowserRuntimeBridge(api, { context: "canonical observation proof" });
  const captured = bridge.ownerFaceCanonicalSnapshot();
  assert.deepEqual(captured, bytes);
  new Uint8Array(api.memory.buffer, pointer, bytes.length).fill(0);
  assert.deepEqual(captured, bytes);
  api.conduit_browser_owner_face_canonical_snapshot = () => -5;
  assert.throws(() => bridge.ownerFaceCanonicalSnapshot(), /snapshot refused/);
});

const sourceOperations = [
  ['check', (bridge, source) => bridge.crecheReviewedInventory(source)],
  ['patchbay', (bridge, source) => bridge.projectPatchbay(source, 1n)],
  ['interaction', (bridge, source) => bridge.crecheAdmitSourceInteraction(source, 1n)],
  ['review', (bridge, source) => bridge.crecheReviewInitialWorkload({ host: 'host/1', boot: 'boot/1', initialPlots: [], source })],
  ['birth', (bridge, source) => bridge.crecheBirth({ host: 'host/1', boot: 'boot/1', friendlyName: 'Body', initialPlots: [], source, sequence: 1n })],
];
function sourceRuntime() {
  const api = runtime();
  let calls = 0;
  const complete = () => {
    calls++;
    new Uint8Array(api.memory.buffer, 0, 2).set([123, 125]);
    api.setProjectionOutput(Uint8Array.of(123, 125));
    return 0;
  };
  for (const name of ['conduit_creche_reviewed_inventory', 'conduit_browser_project_patchbay',
    'conduit_creche_admit_source_interaction', 'conduit_creche_review_initial_workload', 'conduit_creche_birth']) api[name] = complete;
  api.conduit_creche_output_len = () => 2;
  return { api, calls: () => calls };
}
for (const [name, invoke] of sourceOperations) {
  test(`source ${name} refuses non-scalar text before invoking native code`, () => {
    const fixture = sourceRuntime();
    const bridge = bindBrowserRuntimeBridge(fixture.api, { context: "source ingress proof" });
    for (const source of ['\ud800', '\udfff', 1]) assert.throws(() => invoke(bridge, source), /scalar Unicode/);
    assert.equal(fixture.calls(), 0);
  });
  test(`source ${name} preflights its arena before UTF-8 allocation`, () => {
    const fixture = sourceRuntime();
    const bridge = bindBrowserRuntimeBridge(fixture.api, { context: "source ingress proof" });
    const encode = TextEncoder.prototype.encode;
    TextEncoder.prototype.encode = function(value) {
      if (value.length > 1024) throw new Error('oversized encoding attempted');
      return encode.call(this, value);
    };
    try { assert.throws(() => invoke(bridge, 'x'.repeat(1025)), /bound/); }
    finally { TextEncoder.prototype.encode = encode; }
    assert.equal(fixture.calls(), 0);
  });
}

const sourceExports = {
  check: ['conduit_creche_reviewed_inventory', 0, 0],
  patchbay: ['conduit_browser_project_patchbay', 4096, 0],
  interaction: ['conduit_creche_admit_source_interaction', 0, 0],
  review: ['conduit_creche_review_initial_workload', 0, 3],
  birth: ['conduit_creche_birth', 0, 4],
};
for (const [name, invoke] of sourceOperations) {
  test(`source ${name} preserves BOM and scalar UTF-8 bytes exactly`, () => {
    const fixture = sourceRuntime(), source = '\ufeffplot café { "😀" }';
    const [exportName, pointer, sourceIndex] = sourceExports[name];
    const complete = fixture.api[exportName];
    let captured;
    fixture.api[exportName] = (...lengths) => {
      const offset = lengths.slice(0, sourceIndex).reduce((total, length) => total + length, 0);
      captured = new Uint8Array(fixture.api.memory.buffer, pointer + offset, lengths[sourceIndex]).slice();
      return complete();
    };
    invoke(bindBrowserRuntimeBridge(fixture.api, { context: 'source fidelity proof' }), source);
    assert.deepEqual(captured, new TextEncoder().encode(source));
    assert.equal(fixture.calls(), 1);
  });
  if (name === 'review' || name === 'birth') test(`source ${name} accounts for preceding fields before encoding`, () => {
    const fixture = sourceRuntime(), source = 'x'.repeat(name === 'review' ? 1011 : 1007);
    const bridge = bindBrowserRuntimeBridge(fixture.api, { context: 'multipart source proof' });
    const encode = TextEncoder.prototype.encode;
    TextEncoder.prototype.encode = function(value) {
      if (value === source) throw new Error('overbudget source encoding attempted');
      return encode.call(this, value);
    };
    try { assert.throws(() => invoke(bridge, source), /bound/); }
    finally { TextEncoder.prototype.encode = encode; }
    assert.equal(fixture.calls(), 0);
  });
}

for (const mutation of ['wasm-grow', 'source-grow', 'source-transfer']) for (const fail of [false, true]) {
  test(`input retirement keeps its admitted span (${mutation}, failure=${fail})`, () => {
    const api = runtime();
    const buffer = mutation === 'wasm-grow' ? api.memory.buffer : new ArrayBuffer(4, { maxByteLength: 16 });
    const input = mutation === 'wasm-grow' ? new Uint8Array(buffer, 128, 4) : new Uint8Array(buffer);
    input.set([1, 2, 3, 4]);
    new Uint8Array(api.memory.buffer)[4] = 99;
    let calls = 0;
    api.conduit_creche_restore_durable = length => {
      calls++;
      assert.equal(length, 4);
      assert.deepEqual([...new Uint8Array(api.memory.buffer, 0, length)], [1, 2, 3, 4]);
      if (mutation === 'wasm-grow') api.memory.grow(1);
      else if (mutation === 'source-grow') buffer.resize(16);
      else structuredClone(input, { transfer: [input.buffer] });
      if (fail) throw new Error('native failure');
      return 1;
    };
    const bridge = bindBrowserRuntimeBridge(api, { context: 'input retirement proof' });
    if (fail) assert.throws(() => bridge.crecheRestoreDurable(input), /native failure/);
    else assert.equal(bridge.crecheRestoreDurable(input).status, 1);
    assert.equal(calls, 1);
    assert.deepEqual([...new Uint8Array(api.memory.buffer, 0, 4)], [0, 0, 0, 0]);
    assert.equal(new Uint8Array(api.memory.buffer)[4], 99, 'adjacent bytes are outside the admitted input');
  });
}

for (const value of [[256], '1', Uint16Array.of(256), Int8Array.of(-1)]) {
  test(`effect completion refuses implicit byte conversion (${value.constructor.name})`, () => {
    const api = runtime(); let calls = 0;
    api.conduit_browser_plot_complete_effect = () => { calls++; api.setBodyOutput(Uint8Array.of(123, 125)); return 0; };
    const bridge = bindBrowserRuntimeBridge(api, { context: 'effect byte proof' });
    assert.throws(() => bridge.browserPlotCompleteEffect('play/1', 'placement/1', 1, value), /encoded bytes/);
    assert.equal(calls, 0);
  });
}

test('effect completion refuses overcapacity output before allocating its frame', () => {
  const api = runtime(), output = new Uint8Array(1025);
  const bridge = bindBrowserRuntimeBridge(api, { context: 'effect capacity proof' });
  const Original = globalThis.Uint8Array;
  globalThis.Uint8Array = new Proxy(Original, { construct(target, args, type) {
    if (typeof args[0] === 'number' && args[0] > 1024) throw new Error('overcapacity frame allocated');
    return Reflect.construct(target, args, type);
  } });
  try { assert.throws(() => bridge.browserPlotCompleteEffect('play/1', 'placement/1', 1, output), /bound/); }
  finally { globalThis.Uint8Array = Original; }
});

test('effect completion reserves output capacity before encoding identity fields', () => {
  const api = runtime(), output = new Uint8Array(1020);
  const bridge = bindBrowserRuntimeBridge(api, { context: 'effect combined capacity proof' });
  const encode = TextEncoder.prototype.encode;
  TextEncoder.prototype.encode = function(value) {
    if (value === 'play/1') throw new Error('overbudget identity encoded');
    return encode.call(this, value);
  };
  try { assert.throws(() => bridge.browserPlotCompleteEffect('play/1', 'placement/1', 1, output), /bound/); }
  finally { TextEncoder.prototype.encode = encode; }
});

for (const invalid of ['play', 'placement']) test(`effect completion refuses non-scalar ${invalid} identity`, () => {
  const api = runtime(); let calls = 0;
  api.conduit_browser_plot_complete_effect = () => { calls++; api.setBodyOutput(Uint8Array.of(123, 125)); return 0; };
  const bridge = bindBrowserRuntimeBridge(api, { context: 'effect identity proof' });
  assert.throws(() => bridge.browserPlotCompleteEffect(invalid === 'play' ? '\ud800' : 'play/1',
    invalid === 'placement' ? '\udfff' : 'placement/1', 1, Uint8Array.of(42)), /scalar Unicode/);
  assert.equal(calls, 0);
});

test('effect completion preserves exact opaque subview bytes and u32 request sequence', () => {
  const api = runtime(), sequence = 0xffff_ffff;
  const output = Uint8Array.of(99, 255, 0, 254, 99).subarray(1, 4);
  let captured;
  api.conduit_browser_plot_complete_effect = (playLength, placementLength, value, outputLength) => {
    captured = { sequence: value, outputLength,
      output: [...new Uint8Array(api.memory.buffer, playLength + placementLength, outputLength)] };
    api.setBodyOutput(Uint8Array.of(123, 125)); return 0;
  };
  const result = bindBrowserRuntimeBridge(api, { context: 'opaque effect proof' })
    .browserPlotCompleteEffect('play/1', 'placement/1', sequence, output);
  assert.equal(result.status, 0);
  assert.deepEqual(captured, { sequence, outputLength: 3, output: [255, 0, 254] });
  assert.deepEqual([...new Uint8Array(api.memory.buffer, 0, 20)], Array(20).fill(0));
});

function integerEcho(type) {
  return new WebAssembly.Instance(new WebAssembly.Module(Uint8Array.of(
    0,97,115,109,1,0,0,0, 1,6,1,96,1,type,1,type, 3,2,1,0,
    7,8,1,4,101,99,104,111,0,0, 10,6,1,4,0,32,0,11,
  ))).exports.echo;
}
const echo64 = integerEcho(0x7e), echo32 = integerEcho(0x7f);
test('WebAssembly integer conversion can alias out-of-range request identities', () => {
  assert.equal(echo64((1n << 64n) + 1n), 1n);
  assert.equal(echo32(0x1_0000_0001), 1);
});
const sequenceOperations = [
  ['projection', 'u64', 'conduit_browser_project_patchbay', 1, (b,s) => b.projectPatchbay('plot x {}',s)],
  ['source', 'u64', 'conduit_creche_admit_source_interaction', 1, (b,s) => b.crecheAdmitSourceInteraction('plot x {}',s)],
  ['birth', 'u64', 'conduit_creche_birth', 5, (b,s) => b.crecheBirth({ host:'host/1',boot:'boot/1',friendlyName:'Body',initialPlots:[],source:'plot x {}',sequence:s })],
  ['attach', 'u64', 'conduit_creche_attach_here', 2, (b,s) => b.crecheAttachHere(Uint8Array.of(1),Uint8Array.of(2),s)],
  ['cancel', 'u32', 'conduit_browser_plot_acknowledge_cancellation', 2, (b,s) => b.browserPlotAcknowledgeCancellation('play/1','placement/1',s)],
  ['complete', 'u32', 'conduit_browser_plot_complete_effect', 2, (b,s) => b.browserPlotCompleteEffect('play/1','placement/1',s,Uint8Array.of(42))],
  ['refuse', 'u32', 'conduit_browser_plot_refuse_effect', 2, (b,s) => b.browserPlotRefuseEffect('play/1','placement/1',s,1,0)],
];
for (const [name, type, exportName, index, invoke] of sequenceOperations) {
  test(`${name} rejects wrapping or coercing ${type} sequence before native entry`, () => {
    const { api } = sourceRuntime(); let calls = 0;
    api[exportName] = () => { calls++; api.setBodyOutput(Uint8Array.of(123,125)); api.setProjectionOutput(Uint8Array.of(123,125)); return 0; };
    const bridge = bindBrowserRuntimeBridge(api, { context: 'integer admission proof' });
    const invalid = type === 'u64' ? [1n << 64n,-1n,1,'1',true,{valueOf:()=>1n}]
      : [0x1_0000_0001,-1,1.5,1n,'1',true,{valueOf:()=>1}];
    for (const sequence of invalid) assert.throws(() => invoke(bridge,sequence), new RegExp(type));
    assert.equal(calls,0);
  });
  test(`${name} preserves exact legal ${type} sequence endpoints`, () => {
    const { api } = sourceRuntime(); const observed = [];
    api[exportName] = (...args) => {
      observed.push(type === 'u64' ? BigInt.asUintN(64,echo64(args[index])) : echo32(args[index]) >>> 0);
      api.setBodyOutput(Uint8Array.of(123,125)); api.setProjectionOutput(Uint8Array.of(123,125)); return 0;
    };
    const bridge = bindBrowserRuntimeBridge(api, { context: 'integer endpoint proof' });
    const valid = type === 'u64' ? [0n,9007199254740993n,(1n<<64n)-1n] : [0,1,0xffff_ffff];
    for (const sequence of valid) invoke(bridge,sequence);
    assert.deepEqual(observed,valid);
  });
}
for (const field of ['disposition','detail']) test(`effect refusal rejects non-u32 ${field} before native entry`, () => {
  const { api } = sourceRuntime(); let calls = 0;
  api.conduit_browser_plot_refuse_effect = () => { calls++; api.setBodyOutput(Uint8Array.of(123,125)); return 0; };
  const bridge = bindBrowserRuntimeBridge(api, { context: 'refusal integer proof' });
  for (const value of [-1,0x1_0000_0001,1.5,1n,'1',true]) assert.throws(() =>
    bridge.browserPlotRefuseEffect('play/1','placement/1',1,field === 'disposition' ? value : 1,field === 'detail' ? value : 0),/u32/);
  assert.equal(calls,0);
});

test('Patchbay observation reads bounded output without submitting input or inventing a Show', () => {
  const api = runtime();
  const observation = {
    schema: 'conduit.browser/patchbay-observation@1',
    body_plan_id: 'body-plan/current', active_play_id: 'play/current',
    face: { basis: { body_id: 'body/current' } },
    mask_show: null, interaction_admitted: false,
  };
  const bytes = new TextEncoder().encode(JSON.stringify(observation));
  let calls = 0;
  api.conduit_browser_plot_output_ptr = () => 4096;
  api.conduit_browser_plot_output_len = () => bytes.length;
  api.conduit_browser_patchbay_observation = (...args) => {
    assert.deepEqual(args, []);
    calls += 1;
    api.memory.grow(1);
    new Uint8Array(api.memory.buffer, 4096, bytes.length).set(bytes);
    return 1;
  };
  const bridge = bindBrowserRuntimeBridge(api, { context: 'Patchbay observation fixture' });
  assert.deepEqual(bridge.patchbayObservation(), observation);
  assert.equal(calls, 1);
});

test('Patchbay observation preserves refusal and rejects claimed interaction authority', () => {
  for (const [status, output, expected] of [
    [-1, { message: 'exact resident Patchbay owner is not prepared', code: 'OwnerAbsent' }, /exact resident/],
    [1, { schema: 'conduit.browser/patchbay-observation@1', mask_show: {}, interaction_admitted: true }, /Malformed read-only/],
    [1, { schema: 'foreign/observation@1', mask_show: null, interaction_admitted: false }, /Malformed read-only/],
  ]) {
    const api = runtime();
    const bytes = new TextEncoder().encode(JSON.stringify(output));
    api.conduit_browser_plot_output_ptr = () => 4096;
    api.conduit_browser_plot_output_len = () => bytes.length;
    api.conduit_browser_patchbay_observation = () => {
      new Uint8Array(api.memory.buffer, 4096, bytes.length).set(bytes);
      return status;
    };
    const bridge = bindBrowserRuntimeBridge(api, { context: 'Patchbay observation fixture' });
    assert.throws(() => bridge.patchbayObservation(), expected);
  }
});

test('Patchbay disposition forwards exact scope, bounds input and retires bytes on refusal', () => {
  const api = runtime({ inputPointer: 256, inputCapacity: 4096 });
  api.conduit_tour_input_ptr = () => 256;
  api.conduit_tour_input_capacity = () => 4096;
  let outputLength = 0, calls = 0, refused = false;
  api.conduit_browser_plot_output_ptr = () => 8192;
  api.conduit_browser_plot_output_len = () => outputLength;
  const message = { body_id: 'body/current', body_plan_id: 'plan/current', active_play_id: 'play/current',
    owner_checked_plot_id: 'checked/owner', application_revision: 7, outcome: 'Uncertain', evidence_id: 'host/café' };
  api.conduit_browser_patchbay_request_disposition = length => {
    calls += 1;
    assert.deepEqual(JSON.parse(new TextDecoder().decode(new Uint8Array(api.memory.buffer, 256, length))), message);
    api.memory.grow(1);
    const output = refused ? { message: 'stale owner Play', code: 'StaleOwner' }
      : { schema: 'conduit.browser/patchbay-observation@1', mask_show: null, interaction_admitted: false,
        last_owner_disposition: message };
    const bytes = new TextEncoder().encode(JSON.stringify(output));
    new Uint8Array(api.memory.buffer, 8192, bytes.length).set(bytes); outputLength = bytes.length;
    return refused ? -1 : 1;
  };
  const bridge = bindBrowserRuntimeBridge(api, { context: 'Patchbay disposition fixture' });
  assert.deepEqual(bridge.patchbayRequestDisposition(message).last_owner_disposition, message);
  assert(new Uint8Array(api.memory.buffer, 256, new TextEncoder().encode(JSON.stringify(message)).length).every(byte => byte === 0));
  refused = true;
  assert.throws(() => bridge.patchbayRequestDisposition(message), error => error.code === 'StaleOwner');
  assert(new Uint8Array(api.memory.buffer, 256, new TextEncoder().encode(JSON.stringify(message)).length).every(byte => byte === 0));
  assert.throws(() => bridge.patchbayRequestDisposition({ ...message, evidence_id: 'x'.repeat(4096) }), /input bound/);
  assert.equal(calls, 2);
});
