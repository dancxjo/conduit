import test from "node:test";
import assert from "node:assert/strict";
import { prepareWorkspaceVoicePlay } from "../../products/workspace/browser/workspace-voice-play.mjs";

function fixture() {
  const events = [];
  const localAdvertisement = { host_id: "host/browser", boot_id: "boot/browser", offer_generation: 1 };
  const remoteAdvertisement = { host_id: "host/voice", boot_id: "boot/voice", offer_generation: 4 };
  const plan = { plan_id: "plan/voice", fragments: [
    { ...localAdvertisement, placements: [] }, { ...remoteAdvertisement, placements: [] },
  ] };
  const line = { schema: "conduit.creche/joined-host-line@1",
    async prepareRemote(value) { events.push("prepare"); assert.equal(value, plan);
      return { identity: { ...remoteAdvertisement, plan_id: plan.plan_id, active_play_id: "play/voice" }, hello_frames: [[1]] }; },
    async releaseRemote() { events.push("release"); } };
  const joined = { ...remoteAdvertisement, advertisement: remoteAdvertisement, line };
  const voice = { observations: () => [{ class_id: "microphone" }],
    bind(id) { events.push(`bind:${id}`); }, async perform() {}, async close() { events.push("voice-close"); } };
  const remote = { identity: { ...localAdvertisement, plan_id: plan.plan_id, active_play_id: "play/browser" }, close() { events.push("remote-close"); } };
  return { api: {}, localAdvertisement, joined, plan, outputRoot: { ownerDocument: { defaultView: {} } }, events,
    acquireVoice() { events.push("acquire"); return voice; },
    openRemote(options) { events.push("open"); assert.deepEqual(options.observations, [{ class_id: "microphone" }]); return remote; } };
}

test("Workspace voice composition binds exact browser and joined-Host fragments", async () => {
  const f = fixture();
  let driven = 0;
  const play = await prepareWorkspaceVoicePlay({ ...f,
    driveRemote: async ({ remote }) => { driven++; return { disposition: "completed", active_play_id: remote.identity.active_play_id }; } });
  assert.equal(play.identity.active_play_id, "play/browser");
  assert.deepEqual(await play.run(), { disposition: "completed", active_play_id: "play/browser" });
  await play.close();
  assert.equal(driven, 1);
  assert.deepEqual(f.events, ["acquire", "prepare", "open", "bind:play/browser", "remote-close", "voice-close", "release"]);
});

test("Workspace voice composition refuses stale peer identity before acquiring resources", async () => {
  const f = fixture();
  f.joined.boot_id = "stale";
  await assert.rejects(() => prepareWorkspaceVoicePlay(f), /one exact browser and Voice Host/);
  assert.deepEqual(f.events, []);
});

test("Workspace voice composition refuses stale peer offer generation before acquiring resources", async () => {
  const f = fixture();
  f.joined.advertisement.offer_generation = 5;
  await assert.rejects(() => prepareWorkspaceVoicePlay(f), /one exact browser and Voice Host/);
  assert.deepEqual(f.events, []);
});

test("Workspace voice composition refuses stale local offer generation before acquiring resources", async () => {
  const f = fixture();
  f.localAdvertisement.offer_generation = 2;
  await assert.rejects(() => prepareWorkspaceVoicePlay(f), /one exact browser and Voice Host/);
  assert.deepEqual(f.events, []);
});
