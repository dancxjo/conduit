import test from 'node:test';
import assert from 'node:assert/strict';
import { runCheckpointMasks } from '../conformance/face-waist/committed-todo-exchange-runner.mjs';

const empty = 'running 0 tests\ntest result: ok. 0 passed; 0 failed;\n';
const selected = 'running 1 test\ntest result: ok. 1 passed; 0 failed;\n';
const passed = { status: 0, stdout: empty + selected + empty, stderr: '' };
function exercise(results = [passed, passed]) {
  const calls = [];
  const invoke = () => runCheckpointMasks({ CONDUIT_CROSS_MASK_TODO_EXCHANGE: '/same/exchange' }, {
    stdout() {}, stderr() {},
    spawn(program, args, options) {
      calls.push({ program, args, env: options.env });
      return results[calls.length - 1];
    },
  });
  return { calls, invoke };
}
test('both proofs retain one Cargo context and owner-before-browser execution', () => {
  const { calls, invoke } = exercise();
  invoke();
  assert.equal(calls.length, 2);
  assert.equal(calls[0].program, 'cargo');
  assert.equal(calls[1].program, 'cargo');
  const context = ['test', '--locked', '-p', 'conduit', '--bin', 'conduit', '-p', 'conduit-browser-runtime', '--lib'];
  assert.deepEqual(calls[0].args, [...context, 'verified_three_item_todo_and_completion_preserve_nonvisual_meaning']);
  assert.deepEqual(calls[1].args, [...context, 'checked_browser_mask_consumes_actual_committed_todo_exchange', '--', '--ignored']);
  assert.deepEqual(calls[0].env, calls[1].env);
});
test('failed compilation or owner execution stops before the consumer', () => {
  for (const result of [{ status: 1 }, { status: null, error: new Error('launch failed') }]) {
    const { calls, invoke } = exercise([result, passed]);
    assert.throws(invoke);
    assert.equal(calls.length, 1);
  }
});
test('zero, duplicate or multiple selected tests refuse, including across harnesses', () => {
  for (const stdout of [empty, selected + selected, 'running 2 tests\ntest result: ok. 2 passed; 0 failed;\n']) {
    const { calls, invoke } = exercise([{ status: 0, stdout }, passed]);
    assert.throws(invoke, /exactly one/);
    assert.equal(calls.length, 1);
  }
});
test('failed or empty browser proof remains a failure without retry', () => {
  for (const browser of [{ status: 1 }, { status: 0, stdout: empty }]) {
    const { calls, invoke } = exercise([passed, browser]);
    assert.throws(invoke);
    assert.equal(calls.length, 2);
  }
});
