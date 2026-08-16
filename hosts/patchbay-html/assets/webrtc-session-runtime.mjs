const FRAME_CAPACITY = 1024;

const requiredExports = [
  "memory",
  "conduit_browser_webrtc_session_start_fixture",
  "conduit_browser_webrtc_session_start_granted",
  "conduit_browser_webrtc_session_input_ptr",
  "conduit_browser_webrtc_session_input_capacity",
  "conduit_browser_webrtc_session_maximum_frame_bytes",
  "conduit_browser_webrtc_session_maximum_in_flight_items",
  "conduit_browser_webrtc_session_output_ptr",
  "conduit_browser_webrtc_session_output_len",
  "conduit_browser_webrtc_session_clear_output",
  "conduit_browser_webrtc_session_ingest",
  "conduit_browser_webrtc_session_offer",
  "conduit_browser_webrtc_session_pressure",
  "conduit_browser_webrtc_session_deliver",
  "conduit_browser_webrtc_session_value_ptr",
  "conduit_browser_webrtc_session_value_len",
  "conduit_browser_webrtc_session_next_sequence",
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
  const status = api.conduit_browser_webrtc_session_start_fixture(role, variant);
  if (status < 0) throw new Error(`CND-WEBRTC-SESSION-002 start refused ${status}`);
  return { api, status };
}

export async function instantiateGrantedWebRtcSession(wasmBytes, grant) {
  const { instance } = await WebAssembly.instantiate(wasmBytes, {});
  const api = instance.exports;
  if (requiredExports.some((name) => !(name in api)) ||
      api.conduit_browser_webrtc_session_input_capacity() !== FRAME_CAPACITY) {
    throw new Error("CND-WEBRTC-SESSION-001 incomplete WASM ABI");
  }
  const role = grant?.role === "source" ? 0 : grant?.role === "sink" ? 1 : -1;
  const arrayHello = grant?.session_hello;
  const validByteArray = Array.isArray(arrayHello) && arrayHello.every(
    (byte) => Number.isInteger(byte) && byte >= 0 && byte <= 255,
  );
  const hello = arrayHello instanceof Uint8Array
    ? arrayHello
    : validByteArray
      ? Uint8Array.from(arrayHello)
      : null;
  if (role < 0 || hello === null || hello.length === 0 || hello.length > FRAME_CAPACITY) {
    throw new Error("CND-WEBRTC-SESSION-006 invalid Body grant");
  }
  new Uint8Array(
    api.memory.buffer,
    api.conduit_browser_webrtc_session_input_ptr(),
    hello.length,
  ).set(hello);
  const status = api.conduit_browser_webrtc_session_start_granted(role, hello.length);
  if (status < 0) throw new Error(`CND-WEBRTC-SESSION-007 granted start refused ${status}`);
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

export function webRtcSessionLineLimits(runtime) {
  const maximumMessageBytes = runtime.api.conduit_browser_webrtc_session_maximum_frame_bytes();
  const maximumReceivedMessages = runtime.api.conduit_browser_webrtc_session_maximum_in_flight_items();
  const maximumBufferedBytes = maximumMessageBytes * maximumReceivedMessages;
  if (!Number.isSafeInteger(maximumMessageBytes) || maximumMessageBytes <= 0 ||
      !Number.isSafeInteger(maximumReceivedMessages) || maximumReceivedMessages <= 0 ||
      !Number.isSafeInteger(maximumBufferedBytes)) {
    throw new Error("CND-WEBRTC-SESSION-008 invalid admitted Line limits");
  }
  return Object.freeze({ maximumMessageBytes, maximumBufferedBytes, maximumReceivedMessages });
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

function writeInput(runtime, bytes) {
  if (!(bytes instanceof Uint8Array) || bytes.length > FRAME_CAPACITY) {
    throw new Error("CND-WEBRTC-SESSION-005 invalid input");
  }
  new Uint8Array(
    runtime.api.memory.buffer,
    runtime.api.conduit_browser_webrtc_session_input_ptr(),
    bytes.length,
  ).set(bytes);
}

export function offerWebRtcSessionValue(runtime, bytes) {
  writeInput(runtime, bytes);
  return runtime.api.conduit_browser_webrtc_session_offer(bytes.length);
}

export function pressureWebRtcSessionOffer(runtime, bytes) {
  writeInput(runtime, bytes);
  return runtime.api.conduit_browser_webrtc_session_pressure(bytes.length);
}

export function deliverWebRtcSessionValue(runtime) {
  return runtime.api.conduit_browser_webrtc_session_deliver();
}

export function webRtcSessionValue(runtime) {
  const length = runtime.api.conduit_browser_webrtc_session_value_len();
  return new Uint8Array(
    runtime.api.memory.buffer,
    runtime.api.conduit_browser_webrtc_session_value_ptr(),
    length,
  ).slice();
}

export function webRtcSessionNextSequence(runtime) {
  return Number(runtime.api.conduit_browser_webrtc_session_next_sequence());
}
