const DATABASE_NAME = "conduit-browser-host-applications";
const DATABASE_VERSION = 2;
const APPLICATION_STORE = "application-state";
const HOST_STORE = "browser-host-identity";
const DURABLE_IDENTITY = "durable-browser-host";
const IDENTITY_SCHEMA = "conduit.browser/host-identity@1";
const MAXIMUM_IDENTITY_BYTES = 128;
const KEY_BYTES = 32;

const encoder = new TextEncoder();

function requestResult(request) {
  return new Promise((resolve, reject) => {
    request.addEventListener("success", () => resolve(request.result), { once: true });
    request.addEventListener("error", () => reject(request.error ?? new Error("browser Host identity request failed")), { once: true });
  });
}

function transactionComplete(transaction) {
  return new Promise((resolve, reject) => {
    transaction.addEventListener("complete", resolve, { once: true });
    transaction.addEventListener("abort", () => reject(transaction.error ?? new Error("browser Host identity transaction aborted")), { once: true });
    transaction.addEventListener("error", () => reject(transaction.error ?? new Error("browser Host identity transaction failed")), { once: true });
  });
}

async function openDatabase() {
  const request = indexedDB.open(DATABASE_NAME, DATABASE_VERSION);
  request.addEventListener("upgradeneeded", () => {
    const database = request.result;
    if (!database.objectStoreNames.contains(APPLICATION_STORE)) {
      const store = database.createObjectStore(APPLICATION_STORE, { keyPath: "identity" });
      store.createIndex("application", "applicationIdentity", { unique: false });
    }
    if (!database.objectStoreNames.contains(HOST_STORE)) {
      database.createObjectStore(HOST_STORE, { keyPath: "identity" });
    }
  });
  return requestResult(request);
}

function validate(record) {
  const seed = record?.seed;
  if (record?.schema !== IDENTITY_SCHEMA
    || record.identity !== DURABLE_IDENTITY
    || typeof record.hostId !== "string"
    || !record.hostId.startsWith("browser/")
    || encoder.encode(record.hostId).length > MAXIMUM_IDENTITY_BYTES
    || !Array.isArray(seed)
    || seed.length !== KEY_BYTES
    || seed.some((byte) => !Number.isInteger(byte) || byte < 0 || byte > 255)
    || seed.every((byte) => byte === 0)) {
    throw new Error("durable browser Host identity is malformed; explicit Host reset is required");
  }
  return Object.freeze({ hostId: record.hostId, seed: new Uint8Array(seed) });
}

export async function openBrowserHostIdentity({ durable = true } = {}) {
  if (!durable) {
    return Object.freeze({
      schema: IDENTITY_SCHEMA,
      profile: "ephemeral",
      hostId: `browser-ephemeral/${crypto.randomUUID()}`,
      seed: crypto.getRandomValues(new Uint8Array(KEY_BYTES)),
    });
  }
  const database = await openDatabase();
  let transaction = database.transaction(HOST_STORE, "readonly");
  const existing = await requestResult(transaction.objectStore(HOST_STORE).get(DURABLE_IDENTITY));
  await transactionComplete(transaction);
  if (existing) {
    const identity = validate(existing);
    return Object.freeze({ schema: IDENTITY_SCHEMA, profile: "durable", ...identity });
  }

  const seed = crypto.getRandomValues(new Uint8Array(KEY_BYTES));
  const record = {
    schema: IDENTITY_SCHEMA,
    identity: DURABLE_IDENTITY,
    hostId: `browser/${crypto.randomUUID()}`,
    seed: Array.from(seed),
  };
  transaction = database.transaction(HOST_STORE, "readwrite");
  const store = transaction.objectStore(HOST_STORE);
  const raced = await requestResult(store.get(DURABLE_IDENTITY));
  if (raced) {
    await transactionComplete(transaction);
    seed.fill(0);
    const identity = validate(raced);
    return Object.freeze({ schema: IDENTITY_SCHEMA, profile: "durable", ...identity });
  }
  store.add(record);
  await transactionComplete(transaction);
  return Object.freeze({ schema: IDENTITY_SCHEMA, profile: "durable", hostId: record.hostId, seed });
}

export async function resetBrowserHostIdentity() {
  const database = await openDatabase();
  const transaction = database.transaction(HOST_STORE, "readwrite");
  transaction.objectStore(HOST_STORE).delete(DURABLE_IDENTITY);
  await transactionComplete(transaction);
}
