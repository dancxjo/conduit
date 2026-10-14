import test from 'node:test';
import assert from 'node:assert/strict';
import { buildDeliveryStatus } from '../../tools/ci/pipeline/delivery-status.mjs';
const sha = n => String(n).repeat(40);
const run = (head, conclusion, status = 'completed') => ({ id: Number(head[0]), head_branch: 'dev', head_sha: head, conclusion, status,
  html_url: 'https://example.test/run', run_started_at: '2026-10-13T00:00:00Z', updated_at: '2026-10-13T00:02:00Z' });
const input = () => ({ repository: 'owner/repo', development: sha(3), main: sha(2),
  integrations: [run(sha(3), 'failure'), run(sha(1), 'success')], candidates: [], publications: [],
  release: { testedSourceSha: sha(1), acceptedMainSha: sha(2), treeSha: sha(4), integrationRun: '1' }, deployments: [] });
test('failed dev remains distinct from green, accepted, published and absent deployed source', () => {
  const report = buildDeliveryStatus(input());
  assert.equal(report.current_integration.conclusion, 'failure');
  assert.equal(report.latest_green_integration.source_sha, sha(1));
  assert.equal(report.published_release.accepted_main_sha, sha(2));
  assert.equal(report.latest_successful_pages_deployment, null);
  assert.deepEqual(report.differences, {dev_from_green:true, dev_from_published:true, main_from_published:false, dev_from_deployed_pages:null});
});
test('pending runs have no completed timing or conclusion and stay in queue', () => {
  const data = input(); data.integrations.unshift(run(sha(3), 'success', 'in_progress'));
  const report = buildDeliveryStatus(data);
  assert.equal(report.current_integration.conclusion, null);
  assert.equal(report.current_integration.duration_seconds, null);
  assert.equal(report.queue.integrations.length, 1);
});
test('failed latest Pages attempt does not replace an observed successful deployment', () => {
  const data = input(); data.deployments = [{sha:sha(3),statuses:[{state:'failure'}]}, {sha:sha(1),statuses:[{state:'success',environment_url:'https://example.test/pages'}]}];
  const report = buildDeliveryStatus(data);
  assert.equal(report.latest_pages_attempt.source_sha,sha(3));
  assert.equal(report.latest_successful_pages_deployment.source_sha,sha(1));
});
test('unobserved successful integration and publication remain null; malformed identity refuses', () => {
  const data = input(); data.integrations=[]; data.release=null;
  const report = buildDeliveryStatus(data);
  assert.equal(report.latest_green_integration,null); assert.equal(report.differences.dev_from_published,null);
  data.main='short'; assert.throws(()=>buildDeliveryStatus(data),/exact commit/);
});
test('candidate selection separates skipped jobs and preserves measured wall-clock baseline', () => {
  const data=input();data.candidates=[{...run(sha(3),'success'),jobs:[{name:'hosted',conclusion:'success',status:'completed'},{name:'firmware',conclusion:'skipped'}]}];
  const report=buildDeliveryStatus(data);
  assert.equal(report.timing_baseline.candidates[0].duration_seconds,120);
  assert.deepEqual(report.timing_baseline.candidates[0].selected_jobs.map(j=>j.name),['hosted']);
});
test('a superseded success on a failed deployment is not current deployment evidence', () => {
  const data=input();data.deployments=[{sha:sha(3),statuses:[{state:'failure'},{state:'success'}]}];
  assert.equal(buildDeliveryStatus(data).latest_successful_pages_deployment,null);
});

test('a green manual integration from a feature branch cannot assert green development', () => {
  const data=input(); data.integrations.unshift({...run(sha(3),'success'),head_branch:'feature/experiment',event:'workflow_dispatch'});
  const report=buildDeliveryStatus(data);
  assert.equal(report.current_integration.conclusion,'failure');
  assert.equal(report.latest_green_integration.source_sha,sha(1));
});

test('candidate head identity never substitutes for an unobserved tested merge preview', () => {
  const data=input(); data.candidates=[{...run(sha(3),'success'),event:'pull_request'}];
  const candidate=buildDeliveryStatus(data).timing_baseline.candidates[0];
  assert.equal(candidate.pull_request_head_sha,sha(3));
  assert.equal(candidate.tested_merge_preview_sha,null);
  assert.equal(Object.hasOwn(candidate,'source_sha'),false);
});
