// Cooperative local Host ownership. A new Boot may reconcile the prior local
// Boot only after acquiring this lock; another live page is not an offline Host.
export async function acquireBrowserBodyContinuity() {
  if (!globalThis.navigator?.locks?.request) throw new Error('This Host cannot acquire durable Body ownership');
  let release;
  const closed = new Promise(resolve => { release = resolve; });
  await new Promise((resolve, reject) => {
    navigator.locks.request('conduit.application/creche-host-state@1/body-session', { ifAvailable: true }, async lock => {
      if (!lock) { reject(new Error('This Body is open in another window. Return there or close it before opening here.')); return; }
      resolve();
      await closed;
    }).catch(reject);
  });
  // The browser releases the lock if this document or process is destroyed.
  // Do not release on pagehide: an existing document can return from bfcache.
  return Object.freeze({ close: release });
}
