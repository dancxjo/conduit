import { BrowserStorageRefusal } from "./browser-application-storage.mjs";

// Matches the runtime's bounded snapshot record envelope, including metadata.
export const MAXIMUM_RESOURCE_RECORD_BYTES = 4780;
const scopeFields = ["host_id", "boot_id", "active_play_id", "placement_id", "request_sequence"];
function refuse(code, message) { throw new BrowserStorageRefusal(code, message); }

/** Execute one kernel-issued effect under the caller's exact pending scope.
 * The Rust runtime owns Resource/authority admission and record validation.
 * This adapter only transfers opaque bytes through the selected storage API.
 */
export async function executeResourceStorageEffect(storage, effect, pendingScope) {
  if (effect?.schema !== "conduit.browser/resource-effect@1") {
    refuse("InvalidResourceEffect", "unsupported Resource effect schema");
  }
  for (const field of scopeFields) {
    const value = pendingScope?.[field];
    const valid = field === "request_sequence"
      ? Number.isSafeInteger(value) && value >= 0 && value <= 0xffffffff
      : typeof value === "string" && value.length > 0 && value.length <= 256;
    if (!valid || effect[field] !== value) refuse("StaleResourceEffect", "Resource effect differs from the exact pending request");
  }
  if (typeof effect.key !== "string" || !/^resource\/[0-9a-f]{64}$/.test(effect.key)) {
    refuse("InvalidResourceEffect", "Resource effect has an invalid bounded storage key");
  }
  if (effect.effect_kind === "resource-publish") {
    const record = effect.record;
    if (!Array.isArray(record) || record.length === 0 || record.length > MAXIMUM_RESOURCE_RECORD_BYTES
      || !Array.from(record).every((byte) => Number.isInteger(byte) && byte >= 0 && byte <= 255)) {
      refuse("ResourceRecordBound", "Resource publication record is outside its byte contract");
    }
    if (typeof storage?.publishBytes !== "function") {
      refuse("ImplementationNotSelected", "immutable byte publication is not installed");
    }
    await storage.publishBytes(effect.key, new Uint8Array(record));
    return Object.freeze({ status: "completed", record: null });
  }
  if (effect.effect_kind === "resource-read") {
    if (effect.record !== null) {
      refuse("InvalidResourceEffect", "Resource read has the wrong input");
    }
    if (typeof storage?.readBytes !== "function") {
      refuse("ImplementationNotSelected", "byte reading is not installed");
    }
    const record = await storage.readBytes(effect.key);
    if (record === null) refuse("ResourceMissing", "the exact Resource generation is absent");
    if (!(record instanceof Uint8Array) || record.length === 0 || record.length > MAXIMUM_RESOURCE_RECORD_BYTES) {
      refuse("ResourceRecordBound", "stored Resource record exceeds the admitted envelope");
    }
    return Object.freeze({ status: "completed", record });
  }
  refuse("InvalidResourceEffect", "unknown Resource storage operation");
}
