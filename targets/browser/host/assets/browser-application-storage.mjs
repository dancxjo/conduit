const DATABASE_NAME = "conduit-browser-host-applications";
const DATABASE_VERSION = 1;
const STORE_NAME = "application-state";
const MAXIMUM_RECORDS = 64;
const MAXIMUM_KEY_BYTES = 128;
const MAXIMUM_VALUE_BYTES = 64 * 1024;
const MAXIMUM_APPLICATION_BYTES = 1024 * 1024;
const MAXIMUM_APPLICATIONS = 16;
const MAXIMUM_HOST_RECORDS = MAXIMUM_APPLICATIONS * MAXIMUM_RECORDS;
const MAXIMUM_HOST_BYTES = MAXIMUM_APPLICATIONS * MAXIMUM_APPLICATION_BYTES;

const encoder = new TextEncoder();

function requestResult(request) {
  return new Promise((resolve, reject) => {
    request.addEventListener("success", () => resolve(request.result), { once: true });
    request.addEventListener("error", () => reject(request.error ?? new Error("browser storage request failed")), { once: true });
  });
}

function transactionComplete(transaction) {
  return new Promise((resolve, reject) => {
    transaction.addEventListener("complete", resolve, { once: true });
    transaction.addEventListener("abort", () => reject(transaction.error ?? new Error("browser storage transaction aborted")), { once: true });
    transaction.addEventListener("error", () => reject(transaction.error ?? new Error("browser storage transaction failed")), { once: true });
  });
}

async function openDatabase() {
  const request = indexedDB.open(DATABASE_NAME, DATABASE_VERSION);
  request.addEventListener("upgradeneeded", () => {
    const database = request.result;
    if (!database.objectStoreNames.contains(STORE_NAME)) {
      const store = database.createObjectStore(STORE_NAME, { keyPath: "identity" });
      store.createIndex("application", "applicationIdentity", { unique: false });
    }
  });
  return requestResult(request);
}

function exactText(value, label, maximumBytes) {
  if (typeof value !== "string" || value.length === 0 || encoder.encode(value).length > maximumBytes) {
    throw new Error(`${label} is outside its admitted bound`);
  }
  return value;
}

export async function openBrowserApplicationStorage(applicationIdentity, applicationVersion) {
  exactText(applicationIdentity, "application identity", MAXIMUM_KEY_BYTES);
  if (!Number.isSafeInteger(applicationVersion) || applicationVersion < 1) {
    throw new Error("application storage version is invalid");
  }
  const database = await openDatabase();
  const prefix = `${applicationIdentity}@${applicationVersion}\u0000`;

  async function readJson(key) {
    exactText(key, "application storage key", MAXIMUM_KEY_BYTES);
    const transaction = database.transaction(STORE_NAME, "readonly");
    const record = await requestResult(transaction.objectStore(STORE_NAME).get(prefix + key));
    await transactionComplete(transaction);
    if (!record) return null;
    if (record.applicationIdentity !== applicationIdentity || record.applicationVersion !== applicationVersion) {
      throw new Error("application storage identity changed");
    }
    try {
      return JSON.parse(record.value);
    } catch {
      throw new Error("application storage record is malformed");
    }
  }

  async function writeJson(key, value) {
    exactText(key, "application storage key", MAXIMUM_KEY_BYTES);
    const encoded = JSON.stringify(value);
    const valueBytes = encoder.encode(encoded).length;
    if (valueBytes > MAXIMUM_VALUE_BYTES) throw new Error("application storage value exceeds its admitted bound");

    const transaction = database.transaction(STORE_NAME, "readwrite");
    const store = transaction.objectStore(STORE_NAME);
    const hostRecords = await requestResult(store.getAll());
    const records = hostRecords.filter((record) => record.applicationIdentity === applicationIdentity);
    const currentIdentity = prefix + key;
    const current = records.find((record) => record.identity === currentIdentity);
    const nextCount = records.length + (current ? 0 : 1);
    const nextBytes = records.reduce((total, record) => total + record.valueBytes, 0)
      - (current?.valueBytes ?? 0) + valueBytes;
    if (nextCount > MAXIMUM_RECORDS || nextBytes > MAXIMUM_APPLICATION_BYTES) {
      transaction.abort();
      throw new Error("application storage capacity is exhausted");
    }
    const applications = new Set(hostRecords.map((record) => record.applicationIdentity));
    applications.add(applicationIdentity);
    const nextHostCount = hostRecords.length + (current ? 0 : 1);
    const nextHostBytes = hostRecords.reduce((total, record) => total + record.valueBytes, 0)
      - (current?.valueBytes ?? 0) + valueBytes;
    if (applications.size > MAXIMUM_APPLICATIONS || nextHostCount > MAXIMUM_HOST_RECORDS || nextHostBytes > MAXIMUM_HOST_BYTES) {
      transaction.abort();
      throw new Error("browser Host storage capacity is exhausted");
    }
    store.put({
      identity: currentIdentity,
      applicationIdentity,
      applicationVersion,
      key,
      value: encoded,
      valueBytes,
    });
    await transactionComplete(transaction);
  }

  return Object.freeze({
    schema: "conduit.browser/application-storage@1",
    applicationIdentity,
    applicationVersion,
    bounds: Object.freeze({
      maximumRecords: MAXIMUM_RECORDS,
      maximumKeyBytes: MAXIMUM_KEY_BYTES,
      maximumValueBytes: MAXIMUM_VALUE_BYTES,
      maximumApplicationBytes: MAXIMUM_APPLICATION_BYTES,
      maximumApplications: MAXIMUM_APPLICATIONS,
      maximumHostRecords: MAXIMUM_HOST_RECORDS,
      maximumHostBytes: MAXIMUM_HOST_BYTES,
    }),
    readJson,
    writeJson,
  });
}
