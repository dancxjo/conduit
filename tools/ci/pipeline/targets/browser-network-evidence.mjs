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

function validTransferIdentity(value) {
  if (typeof value !== 'string' || value.length > 64 ||
      !/^[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}:[1-9][0-9]*$/.test(value)) return false;
  return Number.isSafeInteger(Number(value.split(':')[1]));
}

function resourceTransferIdentity(resource) {
  const entries = (resource.serverTiming ?? []).filter(entry => entry.name === 'conduit-transfer');
  return entries.length === 1 && validTransferIdentity(entries[0].description)
    ? entries[0].description : null;
}

export function classifyRequestFailures(failures, { resources }) {
  const errors = [];
  const completedAbortNotifications = [];
  const used = new Set();
  for (const failure of failures) {
    // The trusted local proof server stamps each HTTP response with an exact
    // identity in both its header and Server Timing. Wall and monotonic browser
    // clock conversions cannot establish that two records are the same transfer.
    const candidates = validTransferIdentity(failure.transferIdentity) ? resources.filter(resource =>
      resource.name === failure.url && resource.initiatorType === 'fetch' &&
      resourceTransferIdentity(resource) === failure.transferIdentity) : [];
    const resource = candidates.length === 1 ? candidates[0] : null;
    if (failure.errorText === 'net::ERR_ABORTED' && failure.method === 'GET' &&
        failure.resourceType === 'fetch' && failure.status === 200 &&
        Number.isSafeInteger(failure.contentLength) && failure.contentLength > 0 &&
        resource && !used.has(failure.transferIdentity) && resource.responseStatus === 200 &&
        failure.requestStart >= 0 && failure.responseStart >= failure.requestStart &&
        failure.responseEnd > failure.responseStart &&
        resource.requestStart >= 0 && resource.responseStart >= resource.requestStart &&
        resource.responseEnd > resource.responseStart && resource.responseStart > 0 &&
        resource.encodedBodySize === failure.contentLength &&
        resource.decodedBodySize === failure.contentLength) {
      used.add(failure.transferIdentity);
      completedAbortNotifications.push({ ...failure, resourceTiming: resource });
    } else {
      errors.push(`${failure.url}: ${failure.errorText}`);
    }
  }
  return { errors, completedAbortNotifications };
}
