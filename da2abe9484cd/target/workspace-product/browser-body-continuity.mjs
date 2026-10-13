// Cooperative local host ownership. A new Boot may reconcile the prior local
// Boot only after acquiring this lock; another live page is not an offline Host.
export async function acquireBrowserBodyContinuity(identity = 'conduit.application/creche-host-state@1/body-session') {
  if (typeof identity !== 'string' || identity.length === 0 || new TextEncoder().encode(identity).length > 512) throw new TypeError('Invalid Body continuity lock identity');
  if (!globalThis.navigator?.locks?.request) throw new Error('This host cannot acquire durable body ownership');
  let release;
  let completed;
  const closed = new Promise(resolve => { release = resolve; });
  await new Promise((resolve, reject) => {
    completed = navigator.locks.request(identity, { ifAvailable: true }, async lock => {
      if (!lock) { reject(new Error('This body is open in another window. Return there or close it before opening here.')); return; }
      resolve();
      await closed;
    }).catch(reject);
  });
  // The browser releases the lock if this document or process is destroyed.
  // Do not release on pagehide: an existing document can return from bfcache.
  return Object.freeze({ async close() { release(); await completed; } });
}
