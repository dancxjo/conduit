import assert from 'node:assert/strict';
import test from 'node:test';
import { classifyRequestFailures, drainRequestFailures } from '../../tools/ci/pipeline/targets/browser-network-evidence.mjs';

// Captured Chromium notification: the bounded reader consumed 30,309 bytes,
// without cancellation; Resource Timing independently records the full 200 body.
const failure = { url: 'http://127.0.0.1/workspace/workspace.mjs',
  errorText: 'net::ERR_ABORTED', method: 'GET', resourceType: 'fetch',
  startTime: 10_058.496, requestStart: 2.609, responseStart: 3.219,
  responseEnd: 10.529, status: 200, contentLength: 30_309 };
const resource = { name: failure.url, initiatorType: 'fetch', startTime: 58.1,
  requestStart: 61.1, responseStart: 61.8, responseEnd: 66.3, responseStatus: 200,
  encodedBodySize: 30_309, decodedBodySize: 30_309 };
const timing = resources => ({ timeOrigin: 10_000, resources });

test('retain a fully delivered abort notification separately from failures', () => {
  const result = classifyRequestFailures([failure], timing([resource]));
  assert.deepEqual(result.errors, []);
  assert.deepEqual(result.completedAbortNotifications, [{ ...failure, resourceTiming: resource }]);
});

test('real failed, truncated, absent and unrelated transfers remain fatal', () => {
  const cases = [
    [{ ...failure, errorText: 'net::ERR_CONNECTION_RESET' }, resource],
    [{ ...failure, method: 'POST' }, resource],
    [{ ...failure, resourceType: 'script' }, resource],
    [{ ...failure, status: 404 }, resource],
    [{ ...failure, requestStart: -1 }, resource],
    [{ ...failure, responseStart: failure.requestStart - 1 }, resource],
    [{ ...failure, responseEnd: -1 }, resource],
    [{ ...failure, contentLength: NaN }, resource],
    [{ ...failure, contentLength: 0 }, resource],
    [{ ...failure, contentLength: 30_310 }, resource],
    [failure, { ...resource, responseStatus: 0 }],
    [failure, { ...resource, responseEnd: 0 }],
    [failure, { ...resource, responseStart: 0 }],
    [failure, { ...resource, encodedBodySize: 30_308 }],
    [failure, { ...resource, decodedBodySize: 30_308 }],
    [failure, { ...resource, name: `${resource.name}?other` }],
    [failure, { ...resource, initiatorType: 'script' }],
    [failure, { ...resource, requestStart: 50 }],
  ];
  for (const [request, entry] of cases) {
    const result = classifyRequestFailures([request], timing([entry]));
    assert.equal(result.errors.length, 1, JSON.stringify([request, entry]));
    assert.equal(result.completedAbortNotifications.length, 0);
  }
  assert.equal(classifyRequestFailures([failure], timing([])).errors.length, 1);
});

test('browser queueing before the HTTP request does not change its identity', () => {
  const queued = { ...failure, startTime: failure.startTime - 6,
    requestStart: failure.requestStart + 6, responseStart: failure.responseStart + 6,
    responseEnd: failure.responseEnd + 6 };
  assert.deepEqual(classifyRequestFailures([queued], timing([resource])).errors, []);
});

test('one complete response cannot excuse an ambiguous or repeated failure', () => {
  const ambiguous = classifyRequestFailures([failure], timing([resource, { ...resource }]));
  assert.equal(ambiguous.errors.length, 1);
  const repeated = classifyRequestFailures([failure, failure], timing([resource]));
  assert.equal(repeated.completedAbortNotifications.length, 1);
  assert.equal(repeated.errors.length, 1);
});

test('a real failure arriving while earlier response evidence awaits is retained', async () => {
  let finishEarlier;
  const pending = [new Promise(resolve => { finishEarlier = resolve; })];
  const collecting = drainRequestFailures(pending);
  const laterFailure = { ...failure, url: `${failure.url}?later`,
    errorText: 'net::ERR_CONNECTION_RESET' };
  pending.push(Promise.resolve(laterFailure));
  finishEarlier(failure);
  const records = await collecting;
  assert.deepEqual(records, [failure, laterFailure]);
  const result = classifyRequestFailures(records, timing([resource]));
  assert.deepEqual(result.errors, [`${laterFailure.url}: net::ERR_CONNECTION_RESET`]);
});

// A raw HTTP diagnostic sends the first header bytes, then completes the
// headers 25 ms later. Chromium consumes all 4096 body bytes. Playwright's
// responseStart is receiveHeadersEnd, whereas Resource Timing marks first byte.
test('complete fragmented headers need not finish within one millisecond of first byte', () => {
  const headers = { ...failure, responseStart: 28, responseEnd: 40 };
  const delivered = { ...resource, responseEnd: 96 };
  assert.deepEqual(classifyRequestFailures([headers], timing([delivered])).errors, []);
});

test('response timing outside the matched HTTP request remains fatal', () => {
  for (const entry of [
    { ...resource, responseStart: resource.requestStart - 5 },
    { ...resource, responseStart: resource.responseStart + 5 },
  ]) {
    const result = classifyRequestFailures([failure], timing([entry]));
    assert.equal(result.errors.length, 1);
    assert.equal(result.completedAbortNotifications.length, 0);
  }
});
