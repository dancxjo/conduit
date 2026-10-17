import assert from "node:assert/strict";
import test from "node:test";
import { openBrowserSavedChoice } from "../../targets/browser/host/assets/browser-saved-choice.mjs";
const text = bytes => new TextDecoder().decode(bytes);
const configuration = {
  label: "Naming tradition", key: "naming-tradition", initial: "one",
  options: [{ identity: "one", label: "First" }, { identity: "two", label: "Second" }], view: [],
};
function fixture(record = null, write = async () => {}) {
  const states = [];
  const writes = [];
  let change;
  const storage = {
    async readJson(key) { assert.equal(key, "saved-choice/naming-tradition"); return record; },
    async writeJson(key, value) { writes.push({ key, value }); await write(value); record = value; },
  };
  const adapter = openBrowserSavedChoice({ configuration, storage, render(_configuration, _container, choose) {
    change = choose;
    return { state(...value) { states.push(value); }, close() { states.push(["closed"]); } };
  } });
  return { adapter, states, writes, change: value => change(value) };
}
test("missing storage saves the authored initial identity before its first emission", async () => {
  let release;
  const gate = new Promise(resolve => { release = resolve; });
  const host = fixture(null, () => gate);
  let emitted = false;
  const first = host.adapter.next().then(value => { emitted = true; return value; });
  await Promise.resolve();
  await Promise.resolve();
  assert.equal(emitted, false);
  assert.equal(host.writes.length, 1);
  assert.equal(host.states.at(-1)[2], "Saving…");
  release();
  assert.equal(text(await first), "one");
  assert.equal(host.states.at(-1)[2], "Saved in this browser");
  host.adapter.close();
});
test("restores an exact saved option; subsequent selection commits before emission", async () => {
  const host = fixture({ schema: "input/saved-choice@1", identity: "two" });
  assert.equal(text(await host.adapter.next()), "two");
  assert.equal(host.writes.length, 0);
  const next = host.adapter.next();
  assert.equal(host.states.at(-1)[1], true);
  await host.change("one");
  assert.equal(text(await next), "one");
  assert.deepEqual(host.writes[0].value, { schema: "input/saved-choice@1", identity: "one" });
  host.adapter.close();
});
test("corrupt and unknown retained choices refuse distinctly instead of silently substituting defaults", async () => {
  for (const [record, code] of [[{ schema: "wrong", identity: "one" }, "CorruptSavedChoice"],
    [{ schema: "input/saved-choice@1", identity: "missing" }, "UnknownSavedChoice"]]) {
    const host = fixture(record);
    await assert.rejects(host.adapter.next(), { code });
    assert.equal(host.writes.length, 0);
    assert.match(host.states.at(-1)[2], /Not saved/);
    host.adapter.close();
  }
});
test("save failure never emits a successful selection or claims saved", async () => {
  const host = fixture({ schema: "input/saved-choice@1", identity: "one" }, async () => { throw new Error("quota"); });
  await host.adapter.next();
  const next = host.adapter.next();
  const rejected = assert.rejects(next, { code: "ChoiceSaveFailed" });
  await host.change("two");
  await rejected;
  assert.deepEqual(host.states.at(-1), ["one", false, "Not saved: ChoiceSaveFailed"]);
  host.adapter.close();
});
test("pending cancellation and finite request pressure remain distinct", async () => {
  const host = fixture({ schema: "input/saved-choice@1", identity: "one" });
  await host.adapter.next();
  const controller = new AbortController();
  const waiting = host.adapter.next(controller.signal);
  const cancelled = assert.rejects(waiting, { code: "ChoiceCancelled" });
  await assert.rejects(host.adapter.next(), { code: "ChoicePressure" });
  controller.abort();
  await cancelled;
  await host.change("two");
  assert.equal(host.writes.length, 0);
});
test("duplicate authored identities and an unlisted initial option fail admission", () => {
  const render = () => { throw new Error("must not manifest invalid configuration"); };
  for (const invalid of [{ ...configuration, options: [configuration.options[0], configuration.options[0]] },
    { ...configuration, initial: "unknown" }]) {
    assert.throws(() => openBrowserSavedChoice({ configuration: invalid, storage: {}, render }), { code: "InvalidChoiceConfiguration" });
  }
});

test("saved choice admission refuses text that UTF-8 would replace before rendering", () => {
  const render = () => assert.fail("malformed text must refuse before rendering");
  for (const invalid of ['\ud800', '\udfff', 'prefix\ud800suffix']) {
    for (const candidate of [
      { ...configuration, label: invalid },
      { ...configuration, key: invalid },
      { ...configuration, initial: invalid, options: [{ identity: invalid, label: 'Choice' }] },
      { ...configuration, options: [{ identity: 'one', label: invalid }] },
    ]) {
      assert.throws(() => openBrowserSavedChoice({ configuration: candidate, storage: {}, render }),
        { code: 'InvalidChoiceConfiguration' });
    }
  }
});

test("malformed retained Unicode refuses without saving a replacement", async () => {
  const host = fixture({ schema: 'input/saved-choice@1', identity: '\ud800' });
  await assert.rejects(host.adapter.next(), { code: 'CorruptSavedChoice' });
  assert.equal(host.writes.length, 0);
  host.adapter.close();
});

test("saved choices retain and emit exact scalar Unicode identities", async () => {
  const identity = 'é😀';
  const writes = [];
  const adapter = openBrowserSavedChoice({ configuration: { ...configuration, initial: identity,
    options: [{ identity, label: '多言語 😀' }] }, storage: {
    async readJson() { return null; }, async writeJson(_key, record) { writes.push(record); },
  }, render() { return { state() {}, close() {} }; } });
  assert.equal(text(await adapter.next()), identity);
  assert.deepEqual(writes, [{ schema: 'input/saved-choice@1', identity }]);
  adapter.close();
});

test("pending restore keeps the admitted initial choice and renderer configuration", async () => {
  const supplied = structuredClone(configuration);
  let finishRead;
  const read = new Promise(resolve => { finishRead = resolve; });
  const writes = [];
  let rendered;
  const adapter = openBrowserSavedChoice({ configuration: supplied, storage: {
    readJson() { return read; }, async writeJson(key, record) { writes.push({ key, record }); },
  }, render(configuration) { rendered = configuration; return { state() {}, close() {} }; } });
  const pending = adapter.next();
  supplied.initial = 'two'; supplied.key = 'different';
  supplied.options[0].label = 'Changed'; supplied.options.push({ identity: 'forged', label: 'Forged' });
  supplied.view.push({ forged: true });
  finishRead(null);
  assert.equal(text(await pending), 'one');
  assert.deepEqual(writes, [{ key: 'saved-choice/naming-tradition', record: { schema: 'input/saved-choice@1', identity: 'one' } }]);
  assert.deepEqual(rendered, configuration);
  adapter.close();
});
