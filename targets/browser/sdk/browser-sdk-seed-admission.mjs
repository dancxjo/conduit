// Human carrier for existing portable admission documents. The runtime owns
// invitation/receipt checks; explicit human import authenticates the carrier.
const encoder = new TextEncoder();
const decoder = new TextDecoder("utf-8", { fatal: true });
const RECEIPT_KEY = "portable-seed-admission-receipt";
const REQUIRED = ["conduit_browser_seed_input_ptr", "conduit_browser_seed_input_capacity",
  "conduit_browser_seed_prepare", "conduit_browser_seed_complete", "conduit_browser_seed_cancel",
  "conduit_browser_seed_credential", "conduit_browser_membership_output_ptr",
  "conduit_browser_membership_output_len"];

export function createPortableSeedAdmission({ api, storage, assertCurrent, now = Date.now }) {
  let attempted = false, closed = false, owning = false, retained = null, receiptToRetain = null;
  let persistence = null;
  const current = () => {
    assertCurrent();
    if (closed) throw new Error("Seed admission is closed");
    if (REQUIRED.some(name => typeof api[name] !== "function")) {
      throw new Error("Browser runtime lacks portable Seed admission verification");
    }
  };
  const clock = () => {
    const millis = now();
    if (!Number.isSafeInteger(millis) || millis < 0) throw new Error("Seed admission clock is invalid");
    return [millis >>> 0, Math.floor(millis / 0x100000000)];
  };
  const invoke = (operation, document, authorized) => {
    current();
    const [low, high] = clock();
    const bytes = encoder.encode(JSON.stringify(document));
    const maximum = api.conduit_browser_seed_input_capacity();
    if (bytes.length === 0 || bytes.length > maximum) throw new Error("Seed admission document exceeds runtime bound");
    const input = new Uint8Array(api.memory.buffer, api.conduit_browser_seed_input_ptr(), bytes.length);
    input.set(bytes);
    bytes.fill(0);
    let status;
    try { status = api[operation](input.length, low, high, authorized === true ? 1 : 0); }
    catch (error) { input.fill(0); throw error; }
    if (status !== 0) throw new Error(`Seed admission refused (${status})`);
    const output = new Uint8Array(api.memory.buffer, api.conduit_browser_membership_output_ptr(),
      api.conduit_browser_membership_output_len());
    const value = JSON.parse(decoder.decode(output));
    return value;
  };
  const retain = async () => {
    if (!receiptToRetain) throw new Error("No verified Seed receipt awaits retention");
    if (!storage) throw new Error("Durable browser storage is required to retain Seed membership");
    // Persist the exact existing receipt, not a fabricated presence or Body image.
    await storage.writeJson(RECEIPT_KEY, receiptToRetain);
    retained = structuredClone(receiptToRetain.credential);
    receiptToRetain = null;
    return structuredClone(retained);
  };
  return Object.freeze({
    prepare({ invitation, authorizeJoin = false } = {}) {
      current();
      if (owning || attempted) throw new Error("This Host Boot already selected a Body admission or ownership path");
      if (authorizeJoin !== true) throw new Error("Seed admission requires explicit join authorization");
      if (!storage) throw new Error("Durable browser storage is required for Seed admission");
      const request = invoke("conduit_browser_seed_prepare", invitation, true);
      attempted = true;
      return request;
    },
    async complete({ receipt, authorizeMembership = false } = {}) {
      current();
      if (authorizeMembership !== true) throw new Error("Explicit human authorization of the owner receipt carrier is required");
      if (!attempted || retained || persistence || receiptToRetain) throw new Error("No unconsumed Seed admission is pending");
      const credential = invoke("conduit_browser_seed_complete", receipt, true);
      receiptToRetain = structuredClone(receipt);
      // The runtime's correlated credential must be exactly the one retained.
      receiptToRetain.credential = credential;
      persistence = retain();
      try { return await persistence; } finally { persistence = null; }
    },
    async retryRetention() {
      current();
      if (persistence) throw new Error("Seed receipt retention is already active");
      persistence = retain();
      try { return await persistence; } finally { persistence = null; }
    },
    credential() { return retained === null ? null : structuredClone(retained); },
    cancel() {
      current();
      if (persistence || receiptToRetain || retained) throw new Error("Verified membership cannot be canceled as pending admission");
      api.conduit_browser_seed_cancel();
      attempted = true;
    },
    assertMayOwnBody() {
      if (attempted) throw new Error("This Host Boot already selected portable Seed admission");
      owning = true;
    },
    async close() { closed = true; await persistence?.catch(() => {}); },
  });
}
