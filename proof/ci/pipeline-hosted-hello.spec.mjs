import test from 'node:test';
import assert from 'node:assert/strict';
import { requireHostedHelloOutput } from '../../tools/ci/pipeline/targets/hosted-hello.mjs';
const host = 'host std-host-1 boot boot-example profile rust-std protocol 1';
const face = 'FACE-TEXT bytes=13 hex=48454c4c4f2c20574f524c442e';
const plan = 'plan abc complete';
test('exact semantic hello admits optional labels and native line endings', () => {
  for (const caption of ['HELLO, WORLD.', 'Greeting: HELLO, WORLD.']) {
    for (const newline of ['\n', '\r\n']) assert.doesNotThrow(() => requireHostedHelloOutput([host, face, caption, plan].join(newline)));
  }
});
test('a friendly caption cannot substitute for exact emitted bytes', () => {
  for (const wrong of ['', face.replace('bytes=13', 'bytes=12'), face.replace('442e', '442f')]) {
    assert.throws(() => requireHostedHelloOutput([host, wrong, 'Greeting: HELLO, WORLD.', plan].join('\n')));
  }
});
test('missing completion or wrong host still blocks retention', () => {
  for (const output of [[host, face], [host.replace('rust-std', 'other'), face, plan], [face, plan]]) {
    assert.throws(() => requireHostedHelloOutput(output.join('\n')));
  }
});
