import test from "node:test";
import assert from "node:assert/strict";
import { bindBrowserRuntimeBridge } from "./browser-runtime-bridge.mjs";

function runtime(revision = 1) {
  const memory = new WebAssembly.Memory({ initial: 1 });
  let outputLength = 0;
  return {
    memory,
    conduit_browser_runtime_abi_revision: () => revision,
    conduit_test_input_ptr: () => 0,
    conduit_test_input_capacity: () => 64,
    conduit_test_output_ptr: () => 0,
    conduit_test_output_len: () => outputLength,
    conduit_test_output_capacity: () => 64,
    conduit_test_start: () => 0,
    setOutput(bytes) {
      new Uint8Array(memory.buffer, 0, bytes.length).set(bytes);
      outputLength = bytes.length;
    },
  };
}

test("refuses unsupported runtime ABI revision", () => {
  assert.throws(() => bindBrowserRuntimeBridge(runtime(2), { context: "bridge proof" }), (error) =>
    error?.code === "IncompatibleRuntimeAbi"
    && error?.required_runtime_abi === "conduit.browser/runtime-abi@1"
    && error?.runtime_abi_revision === 2
    && Array.isArray(error?.supported_runtime_abi_revisions)
    && error.supported_runtime_abi_revisions.includes(1));
});

test("rejects malformed input bytes", () => {
  const bridge = bindBrowserRuntimeBridge(runtime(), { context: "bridge proof" });
  assert.throws(
    () => bridge.writeInput("not-bytes", {
      pointerExport: "conduit_test_input_ptr",
      capacityExport: "conduit_test_input_capacity",
      label: "test input",
    }),
    /must be encoded bytes/,
  );
});

test("rejects output beyond bounds", () => {
  const api = runtime();
  api.setOutput(Uint8Array.from([1, 2, 3, 4, 5, 6, 7, 8, 9]));
  const bridge = bindBrowserRuntimeBridge(api, { context: "bridge proof" });
  assert.throws(
    () => bridge.readOutputBytes({
      pointerExport: "conduit_test_output_ptr",
      lengthExport: "conduit_test_output_len",
      minimum: 1,
      maximum: 8,
      label: "test output",
    }),
    /test output exceeds its bound/,
  );
});

test("rejects malformed output capacity export", () => {
  const api = runtime();
  api.conduit_test_output_capacity = () => -1;
  api.setOutput(Uint8Array.from([1]));
  const bridge = bindBrowserRuntimeBridge(api, { context: "bridge proof" });
  assert.throws(
    () => bridge.readOutputBytes({
      pointerExport: "conduit_test_output_ptr",
      lengthExport: "conduit_test_output_len",
      capacityExport: "conduit_test_output_capacity",
      minimum: 1,
      maximum: 8,
      label: "test output",
    }),
    /test output exceeds its bound/,
  );
});

test("refreshes memory views after linear memory growth", () => {
  const api = runtime();
  const bridge = bindBrowserRuntimeBridge(api, { context: "bridge proof" });
  bridge.writeInput(Uint8Array.from([1]), {
    pointerExport: "conduit_test_input_ptr",
    capacityExport: "conduit_test_input_capacity",
    label: "test input",
  });
  const initialBuffer = api.memory.buffer;
  api.memory.grow(1);
  bridge.writeInput(Uint8Array.from([2, 3]), {
    pointerExport: "conduit_test_input_ptr",
    capacityExport: "conduit_test_input_capacity",
    label: "test input",
  });
  assert.notEqual(api.memory.buffer, initialBuffer);
  assert.deepEqual([...new Uint8Array(api.memory.buffer, 0, 2)], [2, 3]);
});

test("refuses duplicate start", () => {
  const bridge = bindBrowserRuntimeBridge(runtime(), { context: "bridge proof" });
  bridge.start({
    inputBytes: Uint8Array.from([1]),
    input: {
      pointerExport: "conduit_test_input_ptr",
      capacityExport: "conduit_test_input_capacity",
      minimum: 1,
      label: "test input",
    },
    invoke: () => 0,
    label: "proof lifecycle",
  });
  assert.throws(
    () => bridge.start({
      inputBytes: Uint8Array.from([2]),
      input: {
        pointerExport: "conduit_test_input_ptr",
        capacityExport: "conduit_test_input_capacity",
        minimum: 1,
        label: "test input",
      },
      invoke: () => 0,
      label: "proof lifecycle",
    }),
    /duplicate start refused/,
  );
});

test("allows retry when start status is not accepted", () => {
  const bridge = bindBrowserRuntimeBridge(runtime(), { context: "bridge proof" });
  bridge.start({
    inputBytes: Uint8Array.from([1]),
    input: {
      pointerExport: "conduit_test_input_ptr",
      capacityExport: "conduit_test_input_capacity",
      minimum: 1,
      label: "test input",
    },
    invoke: () => 1,
    acceptedStatuses: [0],
    label: "proof lifecycle",
  });
  bridge.start({
    inputBytes: Uint8Array.from([2]),
    input: {
      pointerExport: "conduit_test_input_ptr",
      capacityExport: "conduit_test_input_capacity",
      minimum: 1,
      label: "test input",
    },
    invoke: () => 0,
    acceptedStatuses: [0],
    label: "proof lifecycle",
  });
});
