// Private retention of a runtime-verified Part record in already selected
// application storage. No new database, generated seed, Host identity, or route.
const MAX_RECORD_BYTES = 3072;
const encoder = new TextEncoder();

export async function openBrowserPartMembershipStore({ membership, storage, expectedBodyId, expectedRoot }) {
  if (!storage || typeof storage.readBytes !== 'function' || typeof storage.writeBytes !== 'function'
    || typeof storage.deleteJson !== 'function' || typeof membership?.exportPartMembership !== 'function'
    || typeof membership.importPartMembership !== 'function' || typeof membership.retireMembershipScope !== 'function'
    || typeof expectedBodyId !== 'string' || expectedBodyId.length < 1 || encoder.encode(expectedBodyId).length > 256
    || !Array.isArray(expectedRoot) || expectedRoot.length !== 32
    || expectedRoot.some(byte => !Number.isInteger(byte) || byte < 0 || byte > 255)
    || expectedRoot.every(byte => byte === 0)) {
    throw new TypeError('Verified Part retention requires independently pinned Body/Root and selected private storage');
  }
  const bodyId = expectedBodyId, root = [...expectedRoot];
  const hash = new Uint8Array(await crypto.subtle.digest('SHA-256', encoder.encode(bodyId)));
  const key = `part-membership/${Array.from(hash, byte => byte.toString(16).padStart(2, '0')).join('')}`;
  const bounded = bytes => bytes instanceof Uint8Array && bytes.length > 0 && bytes.length <= MAX_RECORD_BYTES;
  return Object.freeze({
    async retain() {
      const bytes = membership.exportPartMembership({ expectedBodyId: bodyId, expectedRoot: [...root] });
      try {
        if (!bounded(bytes)) throw new Error('Private Part record exceeds its admitted bound');
        await storage.writeBytes(key, bytes);
        return Object.freeze({ bodyId, state: 'retained-private-part' });
      } finally { bytes?.fill?.(0); }
    },
    async load() {
      const bytes = await storage.readBytes(key);
      if (bytes === null) return null;
      try {
        if (!bounded(bytes)) throw new Error('Retained private Part record exceeds its admitted bound');
        // Returns the original Root-verified admission credential as scope
        // evidence. Newer return credentials still use the existing owner
        // credential persistence path; no latest credential is invented here.
        const admissionCredential = membership.importPartMembership(bytes, { expectedBodyId: bodyId, expectedRoot: [...root] });
        return Object.freeze({ admissionCredential });
      } finally { bytes?.fill?.(0); }
    },
    async forget(partId) {
      membership.retireMembershipScope({ expectedBodyId: bodyId, partId });
      await storage.deleteJson(key); // existing primitive deletes either encoding
    },
  });
}
