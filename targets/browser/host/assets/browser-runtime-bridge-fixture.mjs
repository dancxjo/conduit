// Shared finite ABI mock for bridge conformance; not a runtime admission proof.
export const ABI = new TextEncoder().encode("conduit.browser/runtime-abi");

export function runtime({ revision = 1, identity = ABI, inputCapacity = 64, inputPointer = 0, outputCapacity = 1024, outputPointer = 0 } = {}) {
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
