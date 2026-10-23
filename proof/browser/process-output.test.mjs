import test from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { processOutput, onceCleanup, preparationState } from './process-output.mjs';
function fixture() {
  let now = 0, scheduled;
  const child = Object.assign(new EventEmitter(), {
    stdout: new EventEmitter(), stderr: new EventEmitter(), pid: 123, exitCode: null, signalCode: null,
  });
  const clock = { now: () => now, set: (fn, delay) => { scheduled = { fn, at: now + delay }; return 1; },
    clear: () => { scheduled = undefined; } };
  const budget = { started: 0, deadline: 20_000, records: [] };
  return { child, clock, budget, at: value => { now = value; },
    expire: () => { now = scheduled.at; scheduled.fn(); },
    wait: () => processOutput(child, / is running/, 'Host', budget, clock) };
}
test('cold readiness after ten seconds spends the existing absolute encounter budget', async () => {
  const f = fixture(); f.at(1500); const pending = f.wait();
  f.at(11_500); f.child.stdout.emit('data', Buffer.from('host is running\n'));
  assert.equal((await pending)[0], ' is running');
  assert.equal(f.budget.records[0].encounter_elapsed_ms, 11_500);
  assert.equal(f.child.stdout.listenerCount('data'), 0);
});
test('phase start cannot reset absolute deadline and late output cannot succeed', async () => {
  const f = fixture(); f.at(12_000); const pending = f.wait();
  f.expire(); f.child.stdout.emit('data', Buffer.from('host is running\n'));
  await assert.rejects(pending, /encounter deadline expired/);
  assert.equal(f.budget.records[0].encounter_elapsed_ms, 20_000);
  assert.equal(f.budget.records[0].pid, 123);
});
test('readiness at the deadline refuses even before the timer callback runs', async () => {
  const f = fixture(); const pending = f.wait(); f.at(20_000);
  f.child.stdout.emit('data', Buffer.from('host is running\n'));
  await assert.rejects(pending, /deadline expired/);
});
test('already exhausted budget refuses without listeners remaining', async () => {
  const f = fixture(); f.at(20_001); await assert.rejects(f.wait(), /deadline expired/);
  assert.equal(f.child.listenerCount('exit'), 0);
});
test('raw diagnostics stay bounded while readiness after noisy output is recognized', async () => {
  const f = fixture(); const pending = f.wait();
  f.child.stderr.emit('data', Buffer.from('x'.repeat(40_000)));
  f.child.stdout.emit('data', Buffer.from('host is running\n')); await pending;
  assert.equal(f.budget.records[0].output.length, 16 * 1024);
  assert.equal(f.budget.records[0].output_truncated, true);
});
test('process exit and spawn errors retain diagnostics', async () => {
  for (const event of ['exit', 'error']) {
    const f = fixture(); const pending = f.wait();
    f.child.stderr.emit('data', Buffer.from('raw failure'));
    f.child.emit(event, event === 'exit' ? 7 : new Error('spawn refused'));
    await assert.rejects(pending, /raw failure/);
  }
});
test('deadline failure and teardown share one cleanup and preserve the original refusal', async () => {
  const f = fixture(); let closed = 0;
  const close = onceCleanup(async () => { closed++; });
  const pending = f.wait().catch(async error => { await close(); throw error; });
  f.expire(); await close(); await assert.rejects(pending, /deadline expired/);
  await close(); assert.equal(closed, 1);
});
test('cleanup failures remain idempotent and observable', async () => {
  let calls = 0; const failure = new Error('cleanup refused');
  const close = onceCleanup(() => { calls++; throw failure; });
  await assert.rejects(close(), error => error === failure);
  await assert.rejects(close(), error => error === failure); assert.equal(calls, 1);
});

test('retirement joins late preparation ownership and refuses late success without spawning', async () => {
  let release, closed = 0, spawned = false;
  const state = preparationState(20_000, () => 0);
  const task = state.run('host', async () => {
    await new Promise(resolve => { release = resolve; });
    state.own(async () => { closed++; });
    state.check(); spawned = true;
  });
  await Promise.resolve();
  const stopped = state.retire(); release();
  await assert.rejects(task, /retired/); await stopped;
  assert.equal(spawned, false); assert.equal(closed, 1);
  assert.equal(state.records[0].outcome, 'refused');
});
test('retirement cancels and settles browser preparation alongside Host cleanup', async () => {
  let rejectBrowser, closed = 0;
  const state = preparationState(20_000, () => 0);
  const task = state.run('browser', () => new Promise((_, reject) => { rejectBrowser = reject; }));
  state.own(() => { closed++; });
  state.cancel = () => rejectBrowser(new Error('page closed'));
  await Promise.resolve(); await state.retire();
  await assert.rejects(task, /page closed/);
  assert.equal(closed, 1); assert.equal(state.records[0].outcome, 'refused');
});
test('preparation returning after absolute expiry cannot report ready', async () => {
  let now = 0, release, closed = 0;
  const state = preparationState(20_000, () => now);
  const task = state.run('host', () => new Promise(resolve => { release = resolve; }));
  await Promise.resolve(); now = 20_001; release({ close: async () => { closed++; } });
  await assert.rejects(task, /deadline expired/); await state.retire();
  assert.equal(closed, 1); assert.equal(state.records[0].outcome, 'refused');
});

test('never-settling preparation has a bounded teardown join and late rejection stays handled', async () => {
  let expire, rejectPreparation, cleared = 0;
  const clock = { set: (fn, ms) => { assert.equal(ms, 6000); expire = fn; return 7; },
    clear: id => { assert.equal(id, 7); cleared++; } };
  const state = preparationState(20_000, () => 0, clock);
  const task = state.run('host', () => new Promise((_, reject) => { rejectPreparation = reject; }));
  await Promise.resolve();
  const retired = state.retire(); await Promise.resolve(); expire();
  await assert.rejects(retired, /cleanup join deadline expired/);
  assert.equal(cleared, 1);
  assert.deepEqual(state.records.at(-1).pending, ['host']);
  rejectPreparation(new Error('late preparation refusal'));
  await assert.rejects(task, /late preparation refusal/);
  let closed = 0;
  await state.own(() => { closed++; })();
  assert.equal(closed, 1);
});
