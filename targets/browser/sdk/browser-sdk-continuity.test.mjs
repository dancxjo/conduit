import assert from "node:assert/strict";
import test from "node:test";
import { applicationContinuity, createSdkContinuity } from "./browser-sdk-continuity.mjs";
import { acquireBrowserBodyContinuity } from "../host/assets/browser-body-continuity.mjs";
const digest = `sha256:${"a".repeat(64)}`;
const app = identity => ({ identity, stateCompatibility: { identity: "state", version: 1 }, packageDigest: digest });
const refusal = (code, message) => Object.assign(new Error(message), { code });

test("application and compatibility identities isolate state while compatible packages retain it", async () => {
  const first = await applicationContinuity(app("one"), digest);
  assert.notEqual(first.identity, (await applicationContinuity(app("two"), digest)).identity);
  assert.equal(first.identity, (await applicationContinuity({ ...app("one"), packageDigest: `sha256:${"b".repeat(64)}` }, digest)).identity);
  assert.equal(first.identity, (await applicationContinuity({ ...app("one"), stateCompatibility: { identity: "state", version: 2 } }, digest)).identity);
  assert.notEqual(first.identity, (await applicationContinuity({ ...app("one"), stateCompatibility: { identity: "other", version: 1 } }, digest)).identity);
  assert.equal((await applicationContinuity(undefined, digest)).identity, "conduit.browser/sdk-body-continuity@1");
  for (const invalid of [null, {}, { ...app("one"), packageDigest: "fake" }, app(""), { ...app("one"), stateCompatibility: { identity: "state", version: 0 } }]) {
    await assert.rejects(applicationContinuity(invalid, digest), TypeError);
  }
});

function session(events, extra = {}) {
  return createSdkContinuity({ storage: {
    async clearApplication() { events.push("clear"); }, close() { events.push("storage-close"); },
  }, identity: "application", refusal, async acquire(identity) {
    events.push(`acquire:${identity}`);
    return { async close() { events.push("release"); } };
  }, ...extra });
}

test("ownership is lazy and a session caches its one Body without duplicate restore", async () => {
  const events = [];
  const continuity = session(events);
  const body = { async close() { events.push("body-close-and-settle"); } };
  assert.deepEqual(events, []);
  assert.equal(await continuity.run("recover", async () => null), null);
  assert.equal(await continuity.run("birth", async () => body), body);
  assert.equal(await continuity.run("recover", async () => assert.fail("duplicate restore")), body);
  await assert.rejects(continuity.run("birth", async () => assert.fail("duplicate birth")), { code: "BodyAlreadyOpen" });
  await continuity.close(true);
  assert.deepEqual(events, ["acquire:application/body-continuity", "body-close-and-settle", "clear", "storage-close", "release"]);
  assert.throws(() => continuity.run("recover", async () => body), { code: "HostClosed" });
});

test("failed termination cannot clear state or release ownership", async () => {
  const events = [];
  const continuity = session(events);
  await continuity.run("birth", async () => ({ async close() { throw new Error("persistence failure"); } }));
  await assert.rejects(continuity.close(true), /persistence failure/);
  assert.deepEqual(events, ["acquire:application/body-continuity"]);
});

test("forgetting unopened state acquires ownership; refused ownership cannot reset", async () => {
  const events = [];
  const continuity = session(events, { async acquire() { throw refusal("Busy", "another owner"); } });
  await assert.rejects(continuity.close(true), { code: "Busy" });
  assert.deepEqual(events, []);
});

test("cooperative lock excludes competitors, releases explicitly, and retains Workspace default", async () => {
  const original = Object.getOwnPropertyDescriptor(globalThis, "navigator");
  const occupied = new Set();
  Object.defineProperty(globalThis, "navigator", { configurable: true, value: { locks: {
    async request(name, options, callback) {
      assert.deepEqual(options, { ifAvailable: true });
      if (occupied.has(name)) return callback(null);
      occupied.add(name);
      try { return await callback({ name }); } finally { occupied.delete(name); }
    },
  } } });
  try {
    const first = await acquireBrowserBodyContinuity("one");
    await assert.rejects(acquireBrowserBodyContinuity("one"), /another window/);
    const second = await acquireBrowserBodyContinuity("two");
    await first.close();
    const successor = await acquireBrowserBodyContinuity("one");
    await successor.close(); await second.close();
    const workspace = await acquireBrowserBodyContinuity();
    assert(occupied.has("conduit.application/creche-host-state@1/body-session"));
    await workspace.close();
    assert.equal(occupied.size, 0);
  } finally {
    if (original) Object.defineProperty(globalThis, "navigator", original);
    else delete globalThis.navigator;
  }
});

test("close waits for an in-flight birth and its Body settlement before releasing", async () => {
  const events = [];
  const continuity = session(events);
  let finishBirth;
  let entered;
  const started = new Promise(resolve => { entered = resolve; });
  const result = continuity.run("birth", () => {
    entered();
    return new Promise(resolve => { finishBirth = resolve; });
  });
  await started;
  const closed = continuity.close();
  assert.deepEqual(events, ["acquire:application/body-continuity"]);
  finishBirth({ async close() { events.push("body-settled"); } });
  await result; await closed;
  assert.deepEqual(events, ["acquire:application/body-continuity", "body-settled", "storage-close", "release"]);
  await assert.rejects(continuity.close(true), { code: "HostClosed" });
});
