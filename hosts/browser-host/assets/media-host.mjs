const INPUT_CAPACITY = 64 * 1024;
const encoder = new TextEncoder();
const decoder = new TextDecoder();

function requireStatus(status, operation) {
  if (status < 0) throw new Error(`browser media ${operation} refused ${status}`);
}

function write(api, bytes) {
  if (bytes.length > api.conduit_browser_media_input_capacity()) throw new Error("browser media input exceeds admitted capacity");
  new Uint8Array(api.memory.buffer, api.conduit_browser_media_input_ptr(), bytes.length).set(bytes);
}

function evidence(api) {
  const bytes = new Uint8Array(api.memory.buffer, api.conduit_browser_media_evidence_ptr(), api.conduit_browser_media_evidence_len());
  return JSON.parse(decoder.decode(bytes));
}

function refusal(error) {
  if (error?.name === "NotAllowedError") return 1;
  if (error?.name === "NotFoundError") return 4;
  if (error?.name === "OverconstrainedError" || error?.name === "TypeError") return 5;
  if (error?.name === "AbortError") return 2;
  return 8;
}

async function boundedTrackValue(track, Processor = globalThis.MediaStreamTrackProcessor) {
  if (!Processor) throw new DOMException("track processor unsupported", "NotSupportedError");
  const processor = new Processor({ track });
  const reader = processor.readable.getReader();
  const { value, done } = await reader.read();
  if (done || !value) throw new DOMException("track ended before a value", "AbortError");
  try {
    const options = "numberOfFrames" in value ? { planeIndex: 0, format: value.format } : undefined;
    const size = value.allocationSize?.(options) ?? 0;
    if (size <= 0 || size > INPUT_CAPACITY) throw new RangeError("media value exceeds admitted bound");
    const bytes = new Uint8Array(size);
    await value.copyTo(bytes, options);
    return bytes;
  } finally {
    value.close?.();
    await reader.cancel();
  }
}

export function createBrowserMediaHost({
  api, hostId, bootId,
  mediaDevices = navigator.mediaDevices,
  TrackProcessor = globalThis.MediaStreamTrackProcessor,
  status = document.querySelector("#media-status"),
  output = document.querySelector("#media-evidence"),
}) {
  const required = ["conduit_browser_media_input_ptr", "conduit_browser_media_input_capacity", "conduit_browser_media_start_acquisition", "conduit_browser_media_effect_kind", "conduit_browser_media_complete_acquisition", "conduit_browser_media_start_use", "conduit_browser_media_submit_value", "conduit_browser_media_release_value", "conduit_browser_media_close", "conduit_browser_media_device_lost", "conduit_browser_media_track_ended", "conduit_browser_media_evidence_ptr", "conduit_browser_media_evidence_len"];
  if (required.some(name => !(name in api)) || api.conduit_browser_media_input_capacity() !== INPUT_CAPACITY) throw new Error("browser media ABI is incomplete");
  let stream = null;
  const publish = () => {
    const value = evidence(api);
    output.textContent = JSON.stringify(value);
    status.textContent = `${value.phase}${value.terminal ? `: ${value.terminal}` : ""}`;
    return value;
  };
  async function acquire(kind) {
    if (stream) throw new Error("one admitted browser media operation is already active");
    const identity = encoder.encode(hostId + bootId);
    write(api, identity);
    const camera = kind === "camera";
    requireStatus(api.conduit_browser_media_start_acquisition(hostId.length, bootId.length, camera ? 1 : 2, 1, 1, camera ? 64 : 8000, camera ? 64 : 48000, camera ? 64 : 0, camera ? 64 : 0, camera ? 30 : 1), "acquisition admission");
    publish();
    let acquired = false;
    try {
      stream = await mediaDevices.getUserMedia(camera ? { video: { width: { exact: 64 }, height: { exact: 64 }, frameRate: { max: 30 } }, audio: false } : { video: false, audio: { sampleRate: { min: 8000, max: 48000 }, channelCount: { max: 1 } } });
      const track = camera ? stream.getVideoTracks()[0] : stream.getAudioTracks()[0];
      if (!track) throw new DOMException("no matching track", "NotFoundError");
      track.addEventListener("ended", () => { api.conduit_browser_media_track_ended(); publish(); }, { once: true });
      const settings = track.getSettings();
      const handle = encoder.encode(`track/${crypto.randomUUID()}`);
      write(api, handle);
      requireStatus(api.conduit_browser_media_complete_acquisition(0, handle.length, camera ? settings.width : settings.sampleRate, camera ? settings.height : 0, camera ? Math.max(1, Math.round(settings.frameRate ?? 1)) : settings.channelCount, 256), "acquisition completion");
      acquired = true;
      requireStatus(api.conduit_browser_media_start_use(1), "media use Plan");
      const bytes = await boundedTrackValue(track, TrackProcessor);
      write(api, bytes);
      requireStatus(api.conduit_browser_media_submit_value(bytes.length), "media value");
      publish();
      requireStatus(api.conduit_browser_media_release_value(), "media value release");
      requireStatus(api.conduit_browser_media_close(), "media closure");
      for (const member of stream.getTracks()) member.stop();
      stream = null;
      return publish();
    } catch (error) {
      if (!acquired) {
        requireStatus(api.conduit_browser_media_complete_acquisition(refusal(error), 0, 0, 0, 0, 64), "acquisition refusal");
        if (stream) for (const member of stream.getTracks()) member.stop();
        stream = null;
      } else {
        api.conduit_browser_media_device_lost();
        for (const member of stream.getTracks()) member.stop();
        stream = null;
      }
      publish();
      throw error;
    }
  }
  function terminate() {
    if (!stream) return;
    api.conduit_browser_media_device_lost();
    for (const member of stream.getTracks()) member.stop();
    stream = null;
    publish();
  }
  globalThis.addEventListener("pagehide", terminate, { once: true });
  return Object.freeze({ acquire, evidence: () => evidence(api), terminate });
}
