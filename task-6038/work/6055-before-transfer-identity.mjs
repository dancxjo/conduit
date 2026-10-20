// Chromium can report ERR_ABORTED after a fetch body has been consumed. Keep
// those notifications as evidence, but require the same complete HTTP transfer
// in Resource Timing. Partial, ambiguous and unrelated failures remain fatal.
export async function drainRequestFailures(pending) {
  const records = [];
  // Array iteration includes entries appended while an earlier record awaits
  // its response. Promise.all(pending) would snapshot and omit those failures.
  for (const record of pending) records.push(await record);
  return records;
}

export function classifyRequestFailures(failures, { timeOrigin, resources }) {
  const errors = [];
  const completedAbortNotifications = [];
  const used = new Set();
  for (const failure of failures) {
    const candidates = resources.filter(resource =>
      resource.name === failure.url && resource.initiatorType === 'fetch' &&
      // Match the HTTP send time, excluding browser queueing. The APIs round
      // independently. Playwright uses CDP receiveHeadersEnd for responseStart;
      // Resource Timing records the first response byte, before headers finish.
      failure.requestStart >= 0 && failure.responseStart >= failure.requestStart &&
      Math.abs(timeOrigin + resource.requestStart - failure.startTime - failure.requestStart) <= 1 &&
      resource.responseStart >= resource.requestStart &&
      timeOrigin + resource.responseStart <= failure.startTime + failure.responseStart + 1);
    const resource = candidates.length === 1 ? candidates[0] : null;
    if (failure.errorText === 'net::ERR_ABORTED' && failure.method === 'GET' &&
        failure.resourceType === 'fetch' && failure.status === 200 &&
        Number.isSafeInteger(failure.contentLength) && failure.contentLength > 0 &&
        resource && !used.has(resource) && resource.responseStatus === 200 &&
        failure.responseEnd > failure.responseStart &&
        resource.responseEnd > resource.responseStart && resource.responseStart > 0 &&
        resource.encodedBodySize === failure.contentLength &&
        resource.decodedBodySize === failure.contentLength) {
      used.add(resource);
      completedAbortNotifications.push({ ...failure, resourceTiming: resource });
    } else {
      errors.push(`${failure.url}: ${failure.errorText}`);
    }
  }
  return { errors, completedAbortNotifications };
}
