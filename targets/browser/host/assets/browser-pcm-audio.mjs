import { BrowserHostEffectRefusal } from "./browser-form-effects.mjs";

export const PCM_CAPTURE_RESOURCE = "conduit.resource/browser-microphone-turn@1";
export const PCM_CAPTURE_POOL = "browser/microphone-turn";
export const PCM_PLAY_RESOURCE = "conduit.resource/browser-audio-output@1";
export const PCM_PLAY_POOL = "browser/audio-output";

const HEADER_BYTES = 29;
const MAXIMUM_FRAME_BYTES = 65_536;
const MAXIMUM_FRAMES = 2_048;
const MAXIMUM_TURN_MILLIS = 15_000;
const MAXIMUM_CAPTURE_BLOCKS = 8_191;
const MAXIMUM_SAFE_GAIN_MILLIONTHS = 50_000;

const refuse = (disposition, detail, message) => {
  throw new BrowserHostEffectRefusal(disposition, detail, message);
};

function encodeS16Frame({ bytes, sampleRate, channels, frames, clockId, startFrame }) {
  if (!(bytes instanceof Uint8Array) || bytes.length !== frames * channels * 2 ||
      !Number.isInteger(sampleRate) || sampleRate < 8_000 || sampleRate > 192_000 ||
      ![1, 2].includes(channels) || !Number.isInteger(frames) || frames < 1 ||
      frames > MAXIMUM_FRAMES || bytes.length > MAXIMUM_FRAME_BYTES ||
      typeof clockId !== "bigint" || clockId < 1n || typeof startFrame !== "bigint") {
    throw new Error("browser microphone produced PCM outside the admitted profile");
  }
  const encoded = new Uint8Array(HEADER_BYTES + bytes.length);
  const view = new DataView(encoded.buffer);
  encoded[0] = 0;
  view.setUint32(1, sampleRate, true);
  encoded[5] = channels - 1;
  view.setUint16(6, frames, true);
  view.setBigUint64(8, clockId, true);
  view.setBigUint64(16, startFrame, true);
  encoded[24] = 0;
  view.setUint32(25, bytes.length, true);
  encoded.set(bytes, HEADER_BYTES);
  return encoded;
}

function decodeFrame(encoded) {
  if (!(encoded instanceof Uint8Array) || encoded.length < HEADER_BYTES) {
    throw new Error("browser playback received malformed PCM");
  }
  const view = new DataView(encoded.buffer, encoded.byteOffset, encoded.byteLength);
  const representation = encoded[0];
  const sampleRate = view.getUint32(1, true);
  const channels = encoded[5] + 1;
  const frames = view.getUint16(6, true);
  const payloadBytes = view.getUint32(25, true);
  const bytesPerSample = [2, 3, 4][representation];
  if (!bytesPerSample || ![1, 2].includes(channels) || sampleRate < 8_000 ||
      sampleRate > 192_000 || frames < 1 || frames > MAXIMUM_FRAMES ||
      payloadBytes !== frames * channels * bytesPerSample ||
      encoded.length !== HEADER_BYTES + payloadBytes || encoded[24] > 1) {
    throw new Error("browser playback received PCM outside the admitted profile");
  }
  return { representation, sampleRate, channels, frames, payload: encoded.subarray(HEADER_BYTES) };
}

function sample(frame, index) {
  const offset = index * [2, 3, 4][frame.representation];
  const view = new DataView(frame.payload.buffer, frame.payload.byteOffset, frame.payload.byteLength);
  if (frame.representation === 0) return view.getInt16(offset, true) / 32768;
  if (frame.representation === 1) {
    let value = frame.payload[offset] | frame.payload[offset + 1] << 8 | frame.payload[offset + 2] << 16;
    if (value & 0x800000) value |= 0xff000000;
    return value / 8388608;
  }
  const value = view.getFloat32(offset, true);
  if (!Number.isFinite(value)) throw new Error("browser playback received non-finite PCM");
  return Math.max(-1, Math.min(1, value));
}

function randomClock(window) {
  const bytes = new Uint8Array(8);
  window.crypto.getRandomValues(bytes);
  let value = new DataView(bytes.buffer).getBigUint64(0, true);
  if (value === 0n) value = 1n;
  return value;
}

export function acquireBrowserPcmAudio({
  window,
  pushToTalkTarget,
  mediaDevices = window.navigator.mediaDevices,
  TrackProcessor = window.MediaStreamTrackProcessor,
  AudioContext = window.AudioContext,
}) {
  if (!window || !pushToTalkTarget?.isConnected) throw new Error("push-to-talk control is unavailable");
  let closed = false, pressed = false, waiter = null, capture = null, context = null, playbackTail = 0;
  const setPressed = value => {
    pressed = value;
    if (value && waiter) { const resolve = waiter; waiter = null; resolve(); }
    if (!value && capture) capture.stopped = true;
  };
  const down = event => { event.preventDefault(); setPressed(true); };
  const up = event => { event.preventDefault(); setPressed(false); };
  pushToTalkTarget.addEventListener("pointerdown", down);
  pushToTalkTarget.addEventListener("pointerup", up);
  pushToTalkTarget.addEventListener("pointercancel", up);
  pushToTalkTarget.addEventListener("lostpointercapture", up);

  const awaitPress = signal => new Promise((resolve, reject) => {
    if (pressed) return resolve();
    const abort = () => { waiter = null; reject(new Error("browser microphone capture cancelled")); };
    waiter = () => { signal.removeEventListener("abort", abort); resolve(); };
    signal.addEventListener("abort", abort, { once: true });
    if (signal.aborted) abort();
  });

  const stopCapture = async () => {
    const current = capture;
    capture = null;
    if (!current) return;
    window.clearTimeout(current.deadline);
    await current.reader.cancel().catch(() => {});
    for (const track of current.stream.getTracks()) track.stop();
  };

  async function startCapture(signal) {
    await awaitPress(signal);
    if (!mediaDevices?.getUserMedia || !TrackProcessor) refuse("failed", 10, "PCM microphone capture is unavailable");
    let stream;
    try {
      stream = await mediaDevices.getUserMedia({ video: false, audio: {
        sampleRate: { min: 8_000, max: 48_000 }, channelCount: { max: 2 },
        echoCancellation: true, noiseSuppression: true,
      } });
    } catch (error) {
      const denied = error?.name === "NotAllowedError" || error?.name === "SecurityError";
      refuse(denied ? "denied" : "failed", denied ? 11 : 12, `Microphone acquisition ${denied ? "denied" : "failed"}`);
    }
    const track = stream.getAudioTracks()[0];
    if (!track) { stream.getTracks().forEach(item => item.stop()); refuse("failed", 13, "Microphone has no audio track"); }
    const reader = new TrackProcessor({ track }).readable.getReader();
    capture = {
      stream, reader, stopped: !pressed, clockId: randomClock(window), startFrame: 0n,
      blocks: 0,
      deadline: window.setTimeout(() => { if (capture) capture.stopped = true; }, MAXIMUM_TURN_MILLIS),
    };
  }

  async function captureFrame(signal) {
    if (closed) throw new Error("browser PCM audio is closed");
    if (!capture) await startCapture(signal);
    if (capture.stopped || capture.blocks >= MAXIMUM_CAPTURE_BLOCKS || signal.aborted) {
      await stopCapture(); return undefined;
    }
    const current = capture;
    const { value, done } = await current.reader.read();
    if (done || !value) { await stopCapture(); refuse("failed", 14, "Microphone track ended"); }
    try {
      const frames = value.numberOfFrames;
      const channels = value.numberOfChannels;
      const options = { planeIndex: 0, format: "s16" };
      const size = value.allocationSize(options);
      if (size > MAXIMUM_FRAME_BYTES) throw new Error("microphone PCM block exceeds bound");
      const bytes = new Uint8Array(size);
      await value.copyTo(bytes, options);
      const encoded = encodeS16Frame({ bytes, sampleRate: value.sampleRate, channels, frames,
        clockId: current.clockId, startFrame: current.startFrame });
      current.startFrame += BigInt(frames);
      current.blocks += 1;
      return encoded;
    } finally {
      value.close();
    }
  }

  async function play(effect, signal) {
    if (closed || typeof effect.frame_hex !== "string" || effect.frame_hex.length % 2 ||
        effect.frame_hex.length > (HEADER_BYTES + MAXIMUM_FRAME_BYTES) * 2 ||
        !Number.isInteger(effect.maximum_gain_millionths) ||
        effect.maximum_gain_millionths < 1 || effect.maximum_gain_millionths > MAXIMUM_SAFE_GAIN_MILLIONTHS) {
      throw new Error("browser PCM playback effect exceeds its admitted envelope");
    }
    const encoded = new Uint8Array(effect.frame_hex.length / 2);
    for (let index = 0; index < encoded.length; index += 1) {
      const value = Number.parseInt(effect.frame_hex.slice(index * 2, index * 2 + 2), 16);
      if (!Number.isInteger(value)) throw new Error("browser PCM playback effect is not hex");
      encoded[index] = value;
    }
    const frame = decodeFrame(encoded);
    if (!context) {
      if (typeof AudioContext !== "function") refuse("failed", 20, "Audio output is unavailable");
      try { context = new AudioContext({ sampleRate: frame.sampleRate }); }
      catch { refuse("failed", 21, "Audio output could not be opened"); }
    }
    try { if (context.state === "suspended") await context.resume(); }
    catch { refuse("denied", 22, "Audio output was denied"); }
    if (context.state !== "running") refuse("denied", 23, "Audio output is not running");
    const buffer = context.createBuffer(frame.channels, frame.frames, frame.sampleRate);
    for (let channel = 0; channel < frame.channels; channel += 1) {
      const output = buffer.getChannelData(channel);
      for (let index = 0; index < frame.frames; index += 1) {
        output[index] = sample(frame, index * frame.channels + channel);
      }
    }
    const source = context.createBufferSource();
    const gain = context.createGain();
    gain.gain.value = effect.maximum_gain_millionths / 1_000_000;
    source.buffer = buffer;
    source.connect(gain); gain.connect(context.destination);
    const start = Math.max(context.currentTime, playbackTail);
    playbackTail = start + buffer.duration;
    await new Promise((resolve, reject) => {
      const abort = () => { source.stop(); reject(new Error("browser PCM playback cancelled")); };
      signal.addEventListener("abort", abort, { once: true });
      source.addEventListener("ended", () => { signal.removeEventListener("abort", abort); resolve(); }, { once: true });
      source.start(start);
    }).catch(error => refuse("failed", 24, error.message));
    source.disconnect(); gain.disconnect();
  }

  async function perform(effect, signal) {
    if (effect.effect_kind === "audio-capture") return captureFrame(signal);
    if (effect.effect_kind === "pcm-playback") return play(effect, signal);
    throw new Error("unsupported browser PCM audio effect");
  }

  async function close() {
    if (closed) return;
    closed = true; setPressed(false);
    pushToTalkTarget.removeEventListener("pointerdown", down);
    pushToTalkTarget.removeEventListener("pointerup", up);
    pushToTalkTarget.removeEventListener("pointercancel", up);
    pushToTalkTarget.removeEventListener("lostpointercapture", up);
    await stopCapture();
    await context?.close();
  }

  return Object.freeze({ capacity: Object.freeze({ capture: 1, playback: 1 }), perform, close });
}
