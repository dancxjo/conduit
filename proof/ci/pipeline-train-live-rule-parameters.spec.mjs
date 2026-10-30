import test from 'node:test';
import assert from 'node:assert/strict';
import { createPolicyObserverApi, validateMachineWriterObservation } from '../../tools/ci/pipeline/train-machine-policy.mjs';
import { machinePolicyFixture } from './helpers/train-machine-policy-fixture.mjs';

function fixture(change = () => {}) {
  const f = machinePolicyFixture();
  f.ruleset.node_id = 'RRS_live_fixture';
  delete f.ruleset.rules[0].parameters;
  delete f.branchRules[0].parameters;
  const graph = { data: { node: { databaseId: f.config.rulesetId,
    source: { databaseId: f.config.repositoryId, nameWithOwner: f.repository },
    rules: { pageInfo: { hasNextPage: false }, nodes: [
      { type: 'UPDATE', parameters: { updateAllowsFetchAndMerge: false } },
    ] } } } };
  change(graph, f);
  const calls = [];
  const observer = createPolicyObserverApi('TRAIN_OBSERVER', f.config, {
    env: { TRAIN_OBSERVER: 'never-print-observer' },
    command(program, args, options) {
      calls.push(args);
      assert.equal(program, 'gh');
      assert.equal(options.env.GH_TOKEN, 'never-print-observer');
      assert.equal(options.env.TRAIN_OBSERVER, undefined);
      if (args[1] === 'graphql') {
        assert.equal(args.length, 4); assert.equal(args[2], '-f');
        assert.match(args[3], /^query=query \{ node\(id: "RRS_live_fixture"\)/);
        assert.doesNotMatch(args[3], /mutation/);
        return { status: 0, stdout: JSON.stringify(graph) };
      }
      const value = args[1].includes('/rulesets/') ? f.ruleset : f.branchRules;
      return { status: 0, stdout: JSON.stringify(value) };
    },
  });
  return { f, observer, calls };
}

test('live REST omission requires independently authenticated exact stored false for both observations', () => {
  const { f, observer, calls } = fixture();
  f.ruleset = observer(`repos/${f.repository}/rulesets/${f.config.rulesetId}`);
  f.branchRules = observer(`repos/${f.repository}/rules/branches/dev?per_page=100`);
  assert.equal(validateMachineWriterObservation(f).rulesetId, f.config.rulesetId);
  assert.equal(calls.filter(args => args[1] === 'graphql').length, 2);
});

for (const [name, change] of [
  ['stored true', g => { g.data.node.rules.nodes[0].parameters.updateAllowsFetchAndMerge = true; }],
  ['missing boolean', g => { delete g.data.node.rules.nodes[0].parameters.updateAllowsFetchAndMerge; }],
  ['null parameters', g => { g.data.node.rules.nodes[0].parameters = null; }],
  ['wrong rule', g => { g.data.node.rules.nodes[0].type = 'CREATION'; }],
  ['additional rule', g => { g.data.node.rules.nodes.push(g.data.node.rules.nodes[0]); }],
  ['truncated rules', g => { g.data.node.rules.pageInfo.hasNextPage = true; }],
  ['wrong repository', g => { g.data.node.source.databaseId++; }],
  ['foreign repository name', g => { g.data.node.source.nameWithOwner = 'other/repo'; }],
  ['wrong ruleset', g => { g.data.node.databaseId++; }],
  ['missing node', g => { g.data.node = null; }],
  ['partial GraphQL error', g => { g.errors = [{ message: 'private detail' }]; }],
]) test(`refuses REST omission with ${name}`, () => {
  const { f, observer } = fixture(change);
  for (const endpoint of [`repos/${f.repository}/rulesets/${f.config.rulesetId}`,
    `repos/${f.repository}/rules/branches/dev?per_page=100`]) {
    assert.throws(() => observer(endpoint), error =>
      /independent stored update rule/.test(error.message) && !error.message.includes('private detail'));
  }
});

test('observer refuses arbitrary GraphQL and REST mutation endpoints', () => {
  const { observer, calls } = fixture();
  for (const endpoint of ['graphql', 'stored-update-rule', 'repos/other/repo/rulesets/404']) {
    assert.throws(() => observer(endpoint), /unsupported/);
  }
  assert.equal(calls.length, 0);
});
