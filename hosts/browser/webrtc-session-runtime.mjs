const FRAME_CAPACITY = 1024;

const requiredExports = [
  "memory",
  "conduit_browser_webrtc_session_start",
  "conduit_browser_webrtc_session_input_ptr",
  "conduit_browser_webrtc_session_input_capacity",
  "conduit_browser_webrtc_session_output_ptr",
  "conduit_browser_webrtc_session_output_len",
  "conduit_browser_webrtc_session_clear_output",
  "conduit_browser_webrtc_session_ingest",
  "conduit_browser_webrtc_session_close_input",
  "conduit_browser_webrtc_session_finish",
];

export async function instantiateWebRtcSession(wasmBytes, role, variant = 0) {
  const { instance } = await WebAssembly.instantiate(wasmBytes, {});
  const api = instance.exports;
  if (requiredExports.some((name) => !(name in api)) ||
      api.conduit_browser_webrtc_session_input_capacity() !== FRAME_CAPACITY) {
    throw new Error("CND-WEBRTC-SESSION-001 incomplete WASM ABI");
  }
  const status = api.conduit_browser_webrtc_session_start(role, variant);
  if (status < 0) throw new Error(`CND-WEBRTC-SESSION-002 start refused ${status}`);
  return { api, status };
}

export function takeWebRtcSessionOutput(runtime) {
  const length = runtime.api.conduit_browser_webrtc_session_output_len();
  if (length === 0) return null;
  if (length > FRAME_CAPACITY) {
    throw new Error(`CND-WEBRTC-SESSION-003 invalid output length ${length}`);
  }
  const bytes = new Uint8Array(
    runtime.api.memory.buffer,
    runtime.api.conduit_browser_webrtc_session_output_ptr(),
    length,
  ).slice();
  const status = runtime.api.conduit_browser_webrtc_session_clear_output();
  if (status < 0) throw new Error(`CND-WEBRTC-SESSION-004 clear refused ${status}`);
  return bytes;
}

export function ingestWebRtcSession(runtime, bytes) {
  if (!(bytes instanceof Uint8Array)) {
    throw new TypeError("session input must be bytes");
  }
  if (bytes.length <= FRAME_CAPACITY) {
    new Uint8Array(
      runtime.api.memory.buffer,
      runtime.api.conduit_browser_webrtc_session_input_ptr(),
      bytes.length,
    ).set(bytes);
  }
  return runtime.api.conduit_browser_webrtc_session_ingest(bytes.length);
}

export function finishWebRtcSession(runtime) {
  return runtime.api.conduit_browser_webrtc_session_finish();
}

export function closeWebRtcSessionInput(runtime) {
  return runtime.api.conduit_browser_webrtc_session_close_input();
}
