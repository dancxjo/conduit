import { acquireBrowserBodyContinuity } from "../host/assets/browser-body-continuity.mjs";

const encoder = new TextEncoder();
const legacyIdentity = "conduit.browser/sdk-body-continuity@1";

export async function applicationContinuity(application, bundleDigest) {
  if (application === undefined) return { identity: legacyIdentity, version: 1, packageDigest: bundleDigest };
  const bounded = value => typeof value === "string" && value.length > 0 && encoder.encode(value).length <= 256;
  if (!bounded(application?.identity) || !bounded(application?.stateCompatibility?.identity)
    || !Number.isSafeInteger(application.stateCompatibility.version) || application.stateCompatibility.version < 1
    || !/^sha256:[0-9a-f]{64}$/.test(application.packageDigest ?? "")) {
    throw new TypeError("Application continuity requires bounded application and compatibility identities, a positive version, and an exact package digest");
  }
  // Compatibility survives package upgrades; different applications cannot share
  // state merely by using the same compatibility label. Storage owns version checks.
  const bytes = encoder.encode(JSON.stringify([application.identity, application.stateCompatibility.identity]));
  const digest = await crypto.subtle.digest("SHA-256", bytes);
  const hex = Array.from(new Uint8Array(digest), byte => byte.toString(16).padStart(2, "0")).join("");
  return { identity: `conduit.browser/application/${hex}`, version: application.stateCompatibility.version,
    packageDigest: application.packageDigest };
}

export function createSdkContinuity({ storage, identity, refusal, acquire = acquireBrowserBodyContinuity }) {
  let body = null;
  let lease = null;
  let pending = Promise.resolve();
  let terminal = false;
  let closing;
  let forgetting = false;
  const assertCurrent = () => {
    if (terminal) throw refusal("HostClosed", "This Browser Host application session is closed");
  };
  const run = (kind, operation) => {
    assertCurrent();
    const result = pending.then(async () => {
      assertCurrent();
      if (storage && !lease) {
        try { lease = await acquire(`${identity}/body-continuity`); }
        catch (error) { throw refusal(error.code ?? "ApplicationOwnershipUnavailable", error.message); }
      }
      if (body) {
        if (kind === "recover") return body;
        throw refusal("BodyAlreadyOpen", "This Browser Host already owns an application Body");
      }
      body = await operation();
      return body;
    });
    pending = result.catch(() => {});
    return result;
  };
  const close = (forget = false) => {
    if (closing) {
      if (forget && !forgetting) return Promise.reject(refusal("HostClosed", "A closed session cannot subsequently forget application state"));
      return closing;
    }
    forgetting = forget;
    terminal = true;
    closing = pending.then(async () => {
      // Forget must own the same lease even if this Host never opened its Body.
      if (forget && storage && !lease) lease = await acquire(`${identity}/body-continuity`);
      await body?.close();
      if (forget) await storage?.clearApplication();
      storage?.close();
      await lease?.close();
    });
    return closing;
  };
  return Object.freeze({ run, close, assertCurrent });
}
