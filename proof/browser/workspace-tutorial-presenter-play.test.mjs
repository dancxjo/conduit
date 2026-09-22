import test from "node:test";
import assert from "node:assert/strict";
import { prepareWorkspaceTutorialPresenterPlay } from "../../products/workspace/browser/workspace-tutorial-presenter-play.mjs";

function fixture() {
  const events = [];
  const localAdvertisement = { host_id: "host/browser", boot_id: "boot/browser", offer_generation: 1 };
  const remoteAdvertisement = { host_id: "host/presenter", boot_id: "boot/presenter", offer_generation: 4 };
  const plan = { plan_id: "plan/tutorial-presenter", fragments: [
    { ...localAdvertisement, placements: [] }, { ...remoteAdvertisement, placements: [] },
  ] };
  const line = { schema: "conduit.creche/joined-host-line@1",
    async prepareRemote(value) { events.push("prepare"); assert.equal(value, plan);
      return { identity: { ...remoteAdvertisement, plan_id: plan.plan_id, active_play_id: "play/presenter" }, hello_frames: [[1]] }; },
    async releaseRemote() { events.push("release"); } };
  const joined = { ...remoteAdvertisement, advertisement: remoteAdvertisement, line };
  const presenter = { observations: () => [{ class_id: "presentation" }],
    bind(id) { events.push(`bind:${id}`); }, async perform() {}, close() { events.push("presenter-close"); } };
  const remote = { identity: { ...localAdvertisement, plan_id: plan.plan_id, active_play_id: "play/browser" }, close() { events.push("remote-close"); } };
  return { api: {}, localAdvertisement, joined, plan, request() {},
    outputRoot: { ownerDocument: { defaultView: {} } }, events,
    acquire() { events.push("acquire"); return presenter; },
    openRemote(options) { events.push("open"); assert.deepEqual(options.observations, [{ class_id: "presentation" }]); return remote; } };
}

test("Workspace tutorial Presenter binds one exact browser and planned Presenter Host", async () => {
  const f = fixture();
  let driven = 0;
  const play = await prepareWorkspaceTutorialPresenterPlay({ ...f,
    driveRemote: async ({ remote }) => { driven++; return { disposition: "completed", active_play_id: remote.identity.active_play_id }; } });
  assert.equal(play.identity.active_play_id, "play/browser");
  assert.deepEqual(await play.run(), { disposition: "completed", active_play_id: "play/browser" });
  await play.close();
  assert.equal(driven, 1);
  assert.deepEqual(f.events, ["acquire", "prepare", "open", "bind:play/browser", "remote-close", "presenter-close", "release"]);
});

test("Workspace tutorial Presenter refuses stale peer truth before acquiring browser resources", async () => {
  const f = fixture();
  f.joined.advertisement.offer_generation = 5;
  await assert.rejects(() => prepareWorkspaceTutorialPresenterPlay(f), /one exact browser and Presenter Host/);
  assert.deepEqual(f.events, []);
});
