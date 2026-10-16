import test from 'node:test';
import assert from 'node:assert/strict';
import { productCargoContext, runCheckpointMasks } from '../conformance/face-waist/committed-todo-exchange-runner.mjs';

const empty = 'running 0 tests\ntest result: ok. 0 passed; 0 failed;\n';
const selected = 'running 1 test\ntest result: ok. 1 passed; 0 failed;\n';
const passed = { status: 0, stdout: empty + selected + empty, stderr: '' };
function exercise(results = [passed, passed], context) {
  const calls = [];
  const invoke = () => runCheckpointMasks({ CONDUIT_CROSS_MASK_TODO_EXCHANGE: '/same/exchange' }, {
    context, stdout() {}, stderr() {},
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

test('the parent product package and feature context reaches both exact proofs', () => {
  const args = ['test', '--no-fail-fast', '-p', 'conduit', '-p', 'xtask', '-p', 'conduit-ai',
    '--features', 'conduit-ai/kernel-operation-owners,conduit-tongues/speech'];
  const context = productCargoContext(JSON.stringify({ schema: 'conduit.ci/product-cargo-context@1', args }));
  assert.deepEqual(context, ['test', '--locked', ...args.slice(1), '-p', 'conduit-browser-runtime', '--lib', '--bin', 'conduit']);
  const { calls, invoke } = exercise(undefined, context); invoke();
  assert.deepEqual(calls[0].args.slice(0, context.length), context);
  assert.deepEqual(calls[1].args.slice(0, context.length), context);
  assert.equal(calls[0].args.at(-1), 'verified_three_item_todo_and_completion_preserve_nonvisual_meaning');
  assert.deepEqual(calls[1].args.slice(-3), ['checked_browser_mask_consumes_actual_committed_todo_exchange', '--', '--ignored']);
});
test('invalid contexts cannot replace the locked source selection or add a filter', () => {
  for (const args of [[], ['run', '-p', 'conduit'], ['test', '-p', 'foreign'],
    ['test', '-p', 'conduit', 'other_test'], ['test', '-p', 'conduit', '--', '--ignored'],
    ['test', '-p', 'conduit', '--manifest-path', '/foreign/Cargo.toml'],
    ['test', '-p', 'conduit', '--features', 'malformed'], ['test', '-p', 'conduit', '--no-default-features'],
    ['test', '-p', '../foreign'], ['test', ...Array(128).fill('--locked')]]) {
    assert.throws(() => productCargoContext(JSON.stringify({ schema: 'conduit.ci/product-cargo-context@1', args })));
  }
  assert.throws(() => productCargoContext('{'));
  assert.throws(() => productCargoContext(JSON.stringify({schema: 'other', args: ['test', '-p', 'conduit']})));
});
test('an already selected browser runtime and locked flag are retained once', () => {
  const context = productCargoContext(JSON.stringify({schema: 'conduit.ci/product-cargo-context@1',
    args: ['test', '--locked', '-p', 'conduit', '-p', 'conduit-browser-runtime']}));
  assert.equal(context.filter(value => value === '--locked').length, 1);
  assert.equal(context.filter(value => value === 'conduit-browser-runtime').length, 1);
});
