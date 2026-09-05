const DATABASE_NAME = "conduit-browser-host-applications";
const DATABASE_VERSION = 2;
const STORE_NAME = "application-state";
const HOST_IDENTITY_STORE_NAME = "browser-host-identity";
const MAXIMUM_RECORDS = 64;
const MAXIMUM_KEY_BYTES = 256;
const MAXIMUM_VALUE_BYTES = 64 * 1024;
const MAXIMUM_APPLICATION_BYTES = 1024 * 1024;
const MAXIMUM_APPLICATIONS = 16;
const MAXIMUM_HOST_RECORDS = MAXIMUM_APPLICATIONS * MAXIMUM_RECORDS;
const MAXIMUM_HOST_BYTES = MAXIMUM_APPLICATIONS * MAXIMUM_APPLICATION_BYTES;
const IMPLEMENTATION_ID = "browser/indexeddb@1";
const IMPLEMENTATION_REVISION = 1;
const ARTIFACT_ID = "browser-application-storage.mjs@1";

const encoder = new TextEncoder();

export class BrowserStorageRefusal extends Error {
  constructor(code, message, cause) {
    super(message, cause ? { cause } : undefined);
    this.name = "BrowserStorageRefusal";
    this.code = code;
  }
}

function refuse(code, message, cause) {
  throw new BrowserStorageRefusal(code, message, cause);
}

function storageFailure(error, fallback = "StorageUnavailable") {
  if (error?.name === "QuotaExceededError") return new BrowserStorageRefusal("QuotaExhausted", "browser storage quota is exhausted", error);
  return new BrowserStorageRefusal(fallback, "browser durable storage is unavailable", error);
}

function requestResult(request) {
  return new Promise((resolve, reject) => {
    request.addEventListener("success", () => resolve(request.result), { once: true });
    request.addEventListener("error", () => reject(storageFailure(request.error)), { once: true });
  });
}

function transactionComplete(transaction) {
  return new Promise((resolve, reject) => {
    transaction.addEventListener("complete", resolve, { once: true });
    transaction.addEventListener("abort", () => reject(storageFailure(transaction.error)), { once: true });
    transaction.addEventListener("error", () => reject(storageFailure(transaction.error)), { once: true });
  });
}

async function openDatabase(factory) {
  let request;
  try { request = factory.open(DATABASE_NAME, DATABASE_VERSION); }
  catch (error) { throw storageFailure(error); }
  request.addEventListener("upgradeneeded", () => {
    const database = request.result;
    if (!database.objectStoreNames.contains(STORE_NAME)) {
      const store = database.createObjectStore(STORE_NAME, { keyPath: "identity" });
      store.createIndex("application", "applicationIdentity", { unique: false });
    }
    if (!database.objectStoreNames.contains(HOST_IDENTITY_STORE_NAME)) {
      database.createObjectStore(HOST_IDENTITY_STORE_NAME, { keyPath: "identity" });
    }
  });
  return requestResult(request);
}

function exactText(value, label, maximumBytes) {
  if (typeof value !== "string" || value.length === 0 || encoder.encode(value).length > maximumBytes) {
    refuse("AdmissionBound", `${label} is outside its admitted bound`);
  }
  return value;
}

export async function openBrowserApplicationStorage(applicationIdentity, applicationVersion, packageDigest, {
  implementationRegistry = [],
  indexedDb = globalThis.indexedDB,
  storageManager = globalThis.navigator?.storage,
} = {}) {
  if (!implementationRegistry.includes(IMPLEMENTATION_ID)) {
    refuse("ImplementationNotSelected", "browser durable storage was not selected into this Host profile");
  }
  if (!indexedDb || typeof indexedDb.open !== "function") {
    refuse("StorageUnavailable", "the selected browser durable-storage API is unavailable");
  }
  exactText(applicationIdentity, "application identity", MAXIMUM_KEY_BYTES);
  if (!Number.isSafeInteger(applicationVersion) || applicationVersion < 1) {
    refuse("VersionInvalid", "application storage version is invalid");
  }
  if (!/^sha256:[0-9a-f]{64}$/.test(packageDigest ?? "")) refuse("PackageIdentityInvalid", "application package identity is invalid");
  const database = await openDatabase(indexedDb);
  const prefix = `${applicationIdentity}@${applicationVersion}\u0000`;
  let current = true;

  function requireCurrent() {
    if (!current) refuse("StaleApplicationGeneration", "application storage belongs to a stale package generation");
  }

  async function durability() {
    if (!storageManager || typeof storageManager.persisted !== "function") {
      return Object.freeze({ state: "EvictionStatusUnavailable", persistenceGuaranteed: false });
    }
    try {
      const persisted = await storageManager.persisted();
      return Object.freeze({ state: persisted ? "PersistenceGranted" : "EvictionPossible", persistenceGuaranteed: persisted });
    } catch (error) {
      return Object.freeze({ state: "EvictionStatusUnavailable", persistenceGuaranteed: false, detail: String(error) });
    }
  }

  async function readRecord(key) {
    requireCurrent();
    exactText(key, "application storage key", MAXIMUM_KEY_BYTES);
    const transaction = database.transaction(STORE_NAME, "readonly");
    const record = await requestResult(transaction.objectStore(STORE_NAME).get(prefix + key));
    await transactionComplete(transaction);
    if (!record) {
      const transaction = database.transaction(STORE_NAME, "readonly");
      const retained = await requestResult(transaction.objectStore(STORE_NAME).index("application").getAll(applicationIdentity, MAXIMUM_RECORDS + 1));
      await transactionComplete(transaction);
      if (retained.length > MAXIMUM_RECORDS) refuse("CorruptRecord", "application storage record bound was violated");
      if (retained.some((candidate) => candidate.key === key && candidate.applicationVersion !== applicationVersion)) {
        refuse("VersionMismatch", "retained application storage has an incompatible state version");
      }
      return null;
    }
    if (record.applicationIdentity !== applicationIdentity || record.applicationVersion !== applicationVersion) {
      refuse("CorruptRecord", "application storage identity changed");
    }
    return record;
  }

  async function readJson(key) {
    const record = await readRecord(key);
    if (!record) return null;
    if (record.encoding !== undefined && record.encoding !== "json") {
      refuse("ValueKindMismatch", "application storage value is not JSON");
    }
    try {
      return JSON.parse(record.value);
    } catch {
      refuse("CorruptRecord", "application storage record is malformed");
    }
  }

  async function writeValue(key, encoding, value, valueBytes) {
    requireCurrent();
    exactText(key, "application storage key", MAXIMUM_KEY_BYTES);
    const transaction = database.transaction(STORE_NAME, "readwrite");
    const store = transaction.objectStore(STORE_NAME);
    const hostRecords = await requestResult(store.getAll(null, MAXIMUM_HOST_RECORDS + 1));
    if (hostRecords.length > MAXIMUM_HOST_RECORDS) {
      transaction.abort();
      refuse("CorruptRecord", "browser Host storage record bound was violated");
    }
    const records = hostRecords.filter((record) => record.applicationIdentity === applicationIdentity);
    const currentIdentity = prefix + key;
    const current = records.find((record) => record.identity === currentIdentity);
    const nextCount = records.length + (current ? 0 : 1);
    const nextBytes = records.reduce((total, record) => total + record.valueBytes, 0)
      - (current?.valueBytes ?? 0) + valueBytes;
    if (nextCount > MAXIMUM_RECORDS || nextBytes > MAXIMUM_APPLICATION_BYTES) {
      transaction.abort();
      refuse("ApplicationCapacityExhausted", "application storage capacity is exhausted");
    }
    const applications = new Set(hostRecords.map((record) => record.applicationIdentity));
    applications.add(applicationIdentity);
    const nextHostCount = hostRecords.length + (current ? 0 : 1);
    const nextHostBytes = hostRecords.reduce((total, record) => total + record.valueBytes, 0)
      - (current?.valueBytes ?? 0) + valueBytes;
    if (applications.size > MAXIMUM_APPLICATIONS || nextHostCount > MAXIMUM_HOST_RECORDS || nextHostBytes > MAXIMUM_HOST_BYTES) {
      transaction.abort();
      refuse("HostCapacityExhausted", "browser Host storage capacity is exhausted");
    }
    store.put({
      identity: currentIdentity,
      applicationIdentity,
      applicationVersion,
      packageDigest,
      key,
      encoding,
      value,
      valueBytes,
    });
    await transactionComplete(transaction);
  }

  async function writeJson(key, value) {
    requireCurrent();
    exactText(key, "application storage key", MAXIMUM_KEY_BYTES);
    let encoded;
    try { encoded = JSON.stringify(value); }
    catch (error) { refuse("ValueEncoding", "application storage value is not JSON encodable", error); }
    if (typeof encoded !== "string") refuse("ValueEncoding", "application storage value is not JSON encodable");
    const valueBytes = encoder.encode(encoded).length;
    if (valueBytes > MAXIMUM_VALUE_BYTES) refuse("ValueBound", "application storage value exceeds its admitted bound");
    await writeValue(key, "json", encoded, valueBytes);
  }

  async function readBytes(key) {
    const record = await readRecord(key);
    if (!record) return null;
    if (record.encoding !== "bytes") {
      refuse("ValueKindMismatch", "application storage value is not bytes");
    }
    if (!(record.value instanceof ArrayBuffer) || record.value.byteLength !== record.valueBytes
      || record.valueBytes > MAXIMUM_VALUE_BYTES) {
      refuse("CorruptRecord", "application storage byte value is malformed");
    }
    return new Uint8Array(record.value.slice(0));
  }

  async function writeBytes(key, value) {
    requireCurrent();
    exactText(key, "application storage key", MAXIMUM_KEY_BYTES);
    if (!(value instanceof Uint8Array) || !(value.buffer instanceof ArrayBuffer)) {
      refuse("ValueEncoding", "application storage byte value must be a Uint8Array");
    }
    const valueBytes = value.byteLength;
    if (valueBytes > MAXIMUM_VALUE_BYTES) {
      refuse("ValueBound", "application storage value exceeds its admitted bound");
    }
    const copy = value.buffer.slice(value.byteOffset, value.byteOffset + value.byteLength);
    await writeValue(key, "bytes", copy, valueBytes);
  }

  async function deleteJson(key) {
    requireCurrent();
    exactText(key, "application storage key", MAXIMUM_KEY_BYTES);
    const transaction = database.transaction(STORE_NAME, "readwrite");
    transaction.objectStore(STORE_NAME).delete(prefix + key);
    await transactionComplete(transaction);
  }

  async function clearApplication() {
    requireCurrent();
    const transaction = database.transaction(STORE_NAME, "readwrite");
    const store = transaction.objectStore(STORE_NAME);
    const records = await requestResult(store.index("application").getAll(applicationIdentity, MAXIMUM_RECORDS + 1));
    if (records.length > MAXIMUM_RECORDS) {
      transaction.abort();
      refuse("CorruptRecord", "application storage record bound was violated");
    }
    for (const record of records) store.delete(record.identity);
    await transactionComplete(transaction);
  }

  function close() {
    if (!current) return;
    current = false;
    database.close();
  }

  return Object.freeze({
    schema: "conduit.browser/application-storage@1",
    state: "Initialized",
    implementationId: IMPLEMENTATION_ID,
    implementationRevision: IMPLEMENTATION_REVISION,
    artifactId: ARTIFACT_ID,
    applicationIdentity,
    applicationVersion,
    packageDigest,
    bounds: Object.freeze({
      maximumRecords: MAXIMUM_RECORDS,
      maximumKeyBytes: MAXIMUM_KEY_BYTES,
      maximumValueBytes: MAXIMUM_VALUE_BYTES,
      maximumApplicationBytes: MAXIMUM_APPLICATION_BYTES,
      maximumApplications: MAXIMUM_APPLICATIONS,
      maximumHostRecords: MAXIMUM_HOST_RECORDS,
      maximumHostBytes: MAXIMUM_HOST_BYTES,
    }),
    durability,
    readJson,
    writeJson,
    readBytes,
    writeBytes,
    deleteJson,
    clearApplication,
    close,
  });
}
