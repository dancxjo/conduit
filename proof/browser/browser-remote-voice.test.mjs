import test from "node:test";
import assert from "node:assert/strict";
import { acquireBrowserRemoteVoice } from "../../targets/browser/host/assets/browser-remote-voice.mjs";

function fixture() {
  const performed = [];
  const window = { crypto };
  const document = { createElement() { return Object.assign(new EventTarget(), {
    dataset: {}, isConnected: false, remove() { this.isConnected = false; },
  }); } };
  const outputRoot = { isConnected: true, ownerDocument: document, children: [],
    append(element) { element.isConnected = true; this.children.push(element); } };
  const host = { host_id: "host/browser", boot_id: "boot/browser", offer_generation: 3 };
  const fragment = { ...host, placements: [
    { placement_id: "capture", resources: [{ class_id: "conduit.resource/browser-microphone-turn@1", pool_id: "browser/microphone-turn", units: 1 }] },
    { placement_id: "play", resources: [{ class_id: "conduit.resource/browser-audio-output@1", pool_id: "browser/audio-output", units: 1 }] },
  ] };
  const acquirePcm = () => ({ capacity: { capture: 1, playback: 1 },
    async perform(effect) { performed.push(effect); return effect.effect_kind === "audio-capture" ? Uint8Array.of(1, 2) : undefined; },
    async close() {} });
  return { window, outputRoot, host, fragment, acquirePcm, performed };
}

test("remote voice owns exact planned microphone and safe playback resources", async () => {
  const f = fixture();
  const voice = acquireBrowserRemoteVoice(f);
  assert.equal(f.outputRoot.children[0].textContent, "Hold to talk");
  assert.deepEqual(voice.observations().map(item => [item.class_id, item.unreserved_units]), [
    ["conduit.resource/browser-microphone-turn@1", 1],
    ["conduit.resource/browser-audio-output@1", 1],
  ]);
  voice.bind("play/browser");
  assert.throws(() => voice.observations(), /reserved/);
  const signal = new AbortController().signal;
  assert.deepEqual(await voice.perform({ effect_kind: "audio-capture", placement_id: "capture",
    host_id: f.host.host_id, boot_id: f.host.boot_id, active_play_id: "play/browser" }, signal), Uint8Array.of(1, 2));
  await voice.perform({ effect_kind: "pcm-playback", placement_id: "play",
    host_id: f.host.host_id, boot_id: f.host.boot_id, active_play_id: "play/browser",
    maximum_gain_millionths: 50_000 }, signal);
  assert.equal(f.performed.length, 2);
  await voice.close();
  assert.equal(f.outputRoot.children[0].isConnected, false);
});

test("remote voice refuses stale identity, excess resources, and cross-placement effects", async () => {
  const f = fixture();
  f.fragment.boot_id = "stale";
  assert.throws(() => acquireBrowserRemoteVoice(f), /invalid/);
  f.fragment.boot_id = f.host.boot_id;
  f.fragment.placements[0].resources[0].units = 2;
  assert.throws(() => acquireBrowserRemoteVoice(f), /unsupported/);
  f.fragment.placements[0].resources[0].units = 1;
  const voice = acquireBrowserRemoteVoice(f); voice.bind("play/browser");
  await assert.rejects(() => voice.perform({ effect_kind: "pcm-playback", placement_id: "capture",
    host_id: f.host.host_id, boot_id: f.host.boot_id, active_play_id: "play/browser" }, new AbortController().signal), /differs/);
  await voice.close();
});
