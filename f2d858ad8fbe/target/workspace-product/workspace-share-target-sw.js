const DATABASE = "conduit-workspace-share-target";
const STORE = "pending-invitations";
const MAXIMUM_PENDING = 4;
const MAXIMUM_BYTES = 8192;
const LIFETIME_MILLIS = 60_000;

self.addEventListener("install", () => self.skipWaiting());
self.addEventListener("activate", event => event.waitUntil(self.clients.claim()));

self.addEventListener("fetch", event => {
  const url = new URL(event.request.url);
  if (event.request.method !== "POST" || !url.pathname.endsWith("/share-body-invitation")) return;
  event.respondWith(receive(event.request));
});

self.addEventListener("message", event => {
  if (event.data?.type !== "consume-body-invitation" || typeof event.data.token !== "string") return;
  event.waitUntil(consume(event.data.token).then(
    value => event.ports[0]?.postMessage({ type: "body-invitation", value }),
    error => event.ports[0]?.postMessage({ type: "share-refusal", message: error instanceof Error ? error.message : String(error) }),
  ));
});

async function receive(request) {
  const fields = await request.formData();
  const values = fields.getAll("body-invitation");
  if (values.length !== 1 || typeof values[0] !== "string" || !values[0].trim()) return refusal(400, "Body invitation share is absent or duplicated");
  const value = values[0].trim();
  if (new TextEncoder().encode(value).length > MAXIMUM_BYTES) return refusal(413, "Body invitation share exceeds its bound");
  const token = crypto.randomUUID();
  try {
    await retain(token, value, Date.now());
  } catch (error) {
    return refusal(503, error instanceof Error ? error.message : String(error));
  }
  return Response.redirect(new URL(`./#body-share=${encodeURIComponent(token)}`, request.url), 303);
}

async function retain(token, value, now) {
  const database = await openDatabase();
  const transaction = database.transaction(STORE, "readwrite");
  const store = transaction.objectStore(STORE);
  const records = await requestResult(store.getAll());
  for (const record of records) if (record.expires_at_millis <= now) store.delete(record.token);
  if (records.filter(record => record.expires_at_millis > now).length >= MAXIMUM_PENDING) throw new Error("Body invitation share pressure exceeded");
  store.add({ token, value, expires_at_millis: now + LIFETIME_MILLIS });
  await transactionDone(transaction);
  database.close();
}

async function consume(token) {
  if (!/^[0-9a-f-]{36}$/u.test(token)) throw new Error("Body invitation share token is malformed");
  const database = await openDatabase();
  const transaction = database.transaction(STORE, "readwrite");
  const store = transaction.objectStore(STORE);
  const record = await requestResult(store.get(token));
  if (record) store.delete(token);
  await transactionDone(transaction);
  database.close();
  if (!record) throw new Error("Body invitation share was already consumed or unavailable");
  if (record.expires_at_millis <= Date.now()) throw new Error("Body invitation share expired");
  return record.value;
}

function openDatabase() {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(DATABASE, 1);
    request.onupgradeneeded = () => request.result.createObjectStore(STORE, { keyPath: "token" });
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error ?? new Error("Body invitation share storage failed"));
  });
}

function requestResult(request) {
  return new Promise((resolve, reject) => {
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error ?? new Error("Body invitation share storage failed"));
  });
}

function transactionDone(transaction) {
  return new Promise((resolve, reject) => {
    transaction.oncomplete = resolve;
    transaction.onabort = transaction.onerror = () => reject(transaction.error ?? new Error("Body invitation share storage failed"));
  });
}

function refusal(status, message) {
  return new Response(message, { status, headers: { "content-type": "text/plain; charset=utf-8", "cache-control": "no-store" } });
}
