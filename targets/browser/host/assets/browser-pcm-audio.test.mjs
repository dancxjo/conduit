import test from "node:test";
import assert from "node:assert/strict";
import { webcrypto } from "node:crypto";
import { acquireBrowserPcmAudio } from "./browser-pcm-audio.mjs";

class Target extends EventTarget { isConnected = true; }
class Track extends EventTarget {
  stopped = false;
  stop() { this.stopped = true; }
}

function fixture() {
  const target = new Target();
  const track = new Track();
  const audioData = {
    numberOfFrames: 2, numberOfChannels: 1, sampleRate: 16_000,
    allocationSize: () => 4,
    copyTo: async bytes => bytes.set([0, 0, 255, 127]),
    close() {},
  };
  let reads = 0;
  class Processor {
    readable = { getReader: () => ({
      read: async () => reads++ === 0 ? { value: audioData, done: false } : { done: true },
      cancel: async () => {},
    }) };
  }
  const mediaDevices = { getUserMedia: async () => ({
    getAudioTracks: () => [track], getTracks: () => [track],
  }) };
  const window = { crypto: webcrypto, setTimeout, clearTimeout, navigator: { mediaDevices } };
  return { target, track, Processor, mediaDevices, window };
}

test("push-to-talk emits exact bounded PCM then closes on release", async () => {
  const value = fixture();
  const audio = acquireBrowserPcmAudio({ ...value, pushToTalkTarget: value.target,
    TrackProcessor: value.Processor, AudioContext: undefined });
  value.target.dispatchEvent(new Event("pointerdown", { cancelable: true }));
  const frame = await audio.perform({ effect_kind: "audio-capture" }, new AbortController().signal);
  assert.equal(frame.length, 33);
  const view = new DataView(frame.buffer);
  assert.equal(frame[0], 0);
  assert.equal(view.getUint32(1, true), 16_000);
  assert.equal(frame[5], 0);
  assert.equal(view.getUint16(6, true), 2);
  assert.notEqual(view.getBigUint64(8, true), 0n);
  assert.equal(view.getBigUint64(16, true), 0n);
  assert.equal(view.getUint32(25, true), 4);
  value.target.dispatchEvent(new Event("pointerup", { cancelable: true }));
  assert.equal(await audio.perform({ effect_kind: "audio-capture" }, new AbortController().signal), undefined);
  assert.equal(value.track.stopped, true);
  await audio.close();
});

test("push-to-talk closes and releases the microphone at its independent block bound", async () => {
  const value = fixture();
  const audio = acquireBrowserPcmAudio({ ...value, pushToTalkTarget: value.target,
    TrackProcessor: value.Processor, AudioContext: undefined, maximumCaptureBlocks: 1 });
  value.target.dispatchEvent(new Event("pointerdown", { cancelable: true }));
  assert.ok(await audio.perform({ effect_kind: "audio-capture" }, new AbortController().signal));
  assert.equal(await audio.perform({ effect_kind: "audio-capture" }, new AbortController().signal), undefined);
  assert.equal(value.track.stopped, true);
  await audio.close();
});

test("playback applies the fixed safe gain within its admitted envelope", async () => {
  const value = fixture();
  let observedGain = null;
  class Context {
    state = "running"; currentTime = 0; destination = {};
    createBuffer(channels, frames, sampleRate) {
      assert.deepEqual([channels, frames, sampleRate], [1, 2, 16_000]);
      return { duration: 2 / 16_000, getChannelData: () => new Float32Array(2) };
    }
    createGain() { return { gain: { set value(value) { observedGain = value; } }, connect() {}, disconnect() {} }; }
    createBufferSource() {
      const source = new EventTarget();
      return Object.assign(source, { connect() {}, disconnect() {}, stop() {},
        start() { queueMicrotask(() => source.dispatchEvent(new Event("ended"))); } });
    }
    async close() {}
  }
  const audio = acquireBrowserPcmAudio({ ...value, pushToTalkTarget: value.target,
    TrackProcessor: value.Processor, AudioContext: Context });
  const frame = new Uint8Array(33);
  const view = new DataView(frame.buffer);
  frame[0] = 0; view.setUint32(1, 16_000, true); frame[5] = 0; view.setUint16(6, 2, true);
  view.setBigUint64(8, 1n, true); view.setUint32(25, 4, true); frame.set([0, 0, 255, 127], 29);
  await audio.perform({ effect_kind: "pcm-playback", maximum_gain_millionths: 50_000,
    frame_hex: Buffer.from(frame).toString("hex") }, new AbortController().signal);
  assert.equal(observedGain, 0.05);
  await assert.rejects(() => audio.perform({ effect_kind: "pcm-playback",
    maximum_gain_millionths: 50_001, frame_hex: Buffer.from(frame).toString("hex") },
  new AbortController().signal), /admitted envelope/);
  await audio.close();
});

function playbackFixture() {
  const value = fixture();
  const sources = [];
  let context;
  class Context {
    state = "running"; currentTime = 0; destination = {};
    constructor() { context = this; }
    createBuffer() { return { duration: 0.04, getChannelData: () => new Float32Array(2) }; }
    createGain() { return { gain: {}, connect() {}, disconnect() {} }; }
    createBufferSource() {
      const source = Object.assign(new EventTarget(), {
        connect() {}, disconnect() {}, stopped: false,
        start(when) { this.when = when; sources.push(this); },
        stop() { this.stopped = true; },
      });
      return source;
    }
    async close() {}
  }
  const audio = acquireBrowserPcmAudio({ ...value, pushToTalkTarget: value.target,
    TrackProcessor: value.Processor, AudioContext: Context });
  const frame = new Uint8Array(33), view = new DataView(frame.buffer);
  view.setUint32(1, 16_000, true); view.setUint16(6, 2, true);
  view.setBigUint64(8, 1n, true); view.setUint32(25, 4, true);
  const effect = { effect_kind: "pcm-playback", maximum_gain_millionths: 50_000,
    frame_hex: Buffer.from(frame).toString("hex") };
  return { audio, sources, effect, context: () => context, end(index) {
    context.currentTime = sources[index].when + 0.04;
    sources[index].dispatchEvent(new Event("ended"));
  } };
}

test("PCM lookahead schedules adjacent blocks, bounds two sources, and drains finite output", async () => {
  const f = playbackFixture(), controller = new AbortController();
  await f.audio.perform(f.effect, controller.signal);
  let secondDone = false;
  const second = f.audio.perform(f.effect, controller.signal).then(() => { secondDone = true; });
  assert.equal(f.sources[0].when - f.context().currentTime, 0.04);
  assert.deepEqual(f.sources.map(source => source.when), [0.04, 0.08]);
  await Promise.resolve();
  assert.equal(secondDone, false);
  await assert.rejects(f.audio.perform(f.effect, controller.signal), /queue is full/);
  f.end(0);
  await second;
  const third = f.audio.perform(f.effect, controller.signal);
  assert.deepEqual(f.sources.map(source => source.when), [0.04, 0.08, 0.12]);
  f.end(1);
  await third;
  let drained = false;
  const drain = f.audio.perform({ effect_kind: "pcm-playback-drain" }, controller.signal)
    .then(() => { drained = true; });
  await Promise.resolve();
  assert.equal(drained, false);
  f.end(2);
  await drain;
  assert.equal(drained, true);
  await f.audio.close();
});

test("PCM cancellation and host close stop admitted sources immediately", async () => {
  const f = playbackFixture(), first = new AbortController(), second = new AbortController();
  await f.audio.perform(f.effect, first.signal);
  const pending = f.audio.perform(f.effect, second.signal);
  first.abort(); second.abort();
  await assert.rejects(pending, /cancelled/);
  assert.ok(f.sources.every(source => source.stopped));
  await f.audio.close();
  const g = playbackFixture();
  await g.audio.perform(g.effect, new AbortController().signal);
  await g.audio.close();
  assert.equal(g.sources[0].stopped, true);
});

test("PCM drain cancellation stops its whole remaining queue", async () => {
  const f = playbackFixture();
  await f.audio.perform(f.effect, new AbortController().signal);
  const controller = new AbortController();
  const drain = f.audio.perform({ effect_kind: "pcm-playback-drain" }, controller.signal);
  controller.abort();
  await assert.rejects(drain, /cancelled/);
  assert.equal(f.sources[0].stopped, true);
  await f.audio.close();
});
