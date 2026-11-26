const IMPLEMENTATION = "browser/websocket-owner-face@1";
const PROFILE = "browser/owner-face-full@1";
const PREPARATION_REFUSED = -266;
const encoder = new TextEncoder();

/** Prepare only the explicitly IMAGE-selected optional generic Face interface.
 * A normal preparation refusal does not reset membership or publish an offer.
 */
export function prepareOwnerFaceFullProfile({ api, hostId, boot }, { optional = false } = {}) {
  const absent = message => {
    if (!optional) throw new Error(message);
    return Object.freeze({ selected: false });
  };
  if (!boot.implementation_registry.some(item => item.id === IMPLEMENTATION && item.revision === 1)) {
    return absent("Full owner Face Line was not selected in this checked browser IMAGE");
  }
  if (typeof api.conduit_browser_membership_owner_full_profile_admit !== "function"
    || typeof api.conduit_browser_owner_face_full_profile_ready !== "function") {
    return absent("Full owner Face profile is unavailable in this reviewed runtime");
  }
  const request = encoder.encode(JSON.stringify({ host_id: hostId, boot_id: boot.boot_id, profile: PROFILE }));
  if (request.length > api.conduit_browser_membership_input_capacity()) {
    throw new Error("Full profile request extent");
  }
  new Uint8Array(api.memory.buffer, api.conduit_browser_membership_input_ptr(), request.length).set(request);
  const status = api.conduit_browser_membership_owner_full_profile_admit(request.length);
  const ready = api.conduit_browser_owner_face_full_profile_ready();
  if (status === PREPARATION_REFUSED && ready === 0) {
    return absent("Full owner Face profile preparation refused or Host offers are already frozen");
  }
  if (status !== 0 || ready !== 1) {
    throw new Error(`Full owner Face profile preparation failed (${status}, ready ${ready})`);
  }
  return Object.freeze({ selected: true, profile: PROFILE });
}
