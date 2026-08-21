const FRAME_BYTES = 1024;
const LINE_FORWARD = 1;
const LINE_RETURN = 2;
const STATUS_COMPLETE = 1;

function requireStatus(status, operation) {
  if (status < 0) throw new Error(`CND-TEXT-LIVE ${operation} failed ${status}`);
}

function writeInput(api, bytes) {
  if (!(bytes instanceof Uint8Array) || bytes.length > FRAME_BYTES) {
    throw new Error("CND-TEXT-LIVE invalid bounded input");
  }
  new Uint8Array(
    api.memory.buffer,
    api.conduit_browser_text_lab_input_ptr(),
    bytes.length,
  ).set(bytes);
}

export async function instantiateTextLabLive(wasmBytes, url) {
  const { instance } = await WebAssembly.instantiate(wasmBytes, {});
  const api = instance.exports;
  const required = [
    "memory",
    "conduit_browser_text_lab_input_ptr",
    "conduit_browser_text_lab_input_capacity",
    "conduit_browser_text_lab_start",
    "conduit_browser_text_lab_ingest",
    "conduit_browser_text_lab_sent",
    "conduit_browser_text_lab_output_line",
    "conduit_browser_text_lab_expected_line",
    "conduit_browser_text_lab_output_ptr",
    "conduit_browser_text_lab_output_len",
    "conduit_browser_text_lab_status",
  ];
  if (required.some((name) => !(name in api)) ||
      api.conduit_browser_text_lab_input_capacity() !== FRAME_BYTES) {
    throw new Error("CND-TEXT-LIVE incomplete WASM ABI");
  }
  const encodedUrl = new TextEncoder().encode(url);
  writeInput(api, encodedUrl);
  requireStatus(api.conduit_browser_text_lab_start(encodedUrl.length), "start");
  return Object.freeze({ api });
}

export async function runTextLabLive(runtime, forward, openReturn) {
  const { api } = runtime;
  let returned = null;
  let sentFrames = 0;
  let receivedFrames = 0;
  const line = async (identity) => {
    if (identity === LINE_FORWARD) return forward;
    if (identity !== LINE_RETURN) throw new Error("CND-TEXT-LIVE invalid Line identity");
    if (returned === null) returned = await openReturn();
    return returned;
  };
  while (api.conduit_browser_text_lab_status() !== STATUS_COMPLETE) {
    const outputLine = api.conduit_browser_text_lab_output_line();
    if (outputLine !== 0) {
      const length = api.conduit_browser_text_lab_output_len();
      if (length <= 0 || length > FRAME_BYTES) {
        throw new Error("CND-TEXT-LIVE invalid output frame");
      }
      const bytes = new Uint8Array(
        api.memory.buffer,
        api.conduit_browser_text_lab_output_ptr(),
        length,
      ).slice();
      const sent = (await line(outputLine)).sendBinary(bytes);
      if (!sent.ok) throw new Error(`${sent.code} ${sent.detail}`);
      sentFrames += 1;
      requireStatus(api.conduit_browser_text_lab_sent(), "sent receipt");
      continue;
    }
    const expectedLine = api.conduit_browser_text_lab_expected_line();
    if (expectedLine === 0) throw new Error("CND-TEXT-LIVE runtime omitted expected Line");
    const bytes = await (await line(expectedLine)).receiveBinary();
    writeInput(api, bytes);
    requireStatus(api.conduit_browser_text_lab_ingest(expectedLine, bytes.length), "ingest");
    receivedFrames += 1;
  }
  return Object.freeze({ returned, sentFrames, receivedFrames });
}
