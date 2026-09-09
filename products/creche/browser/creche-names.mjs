import catalog from "../names/catalog.mjs";
const encoder = new TextEncoder();
const MAX_FRIENDLY_NAME_BYTES = 64;
export const PERSONA_CATALOG_VERSION = catalog.version;
const DOMAIN = "conduit-creche-persona-v1";
const MAX_DERIVATION_ATTEMPTS = 16;
const ROMANIZED_COMPONENT = /^[\p{Script=Latin}\p{Mark} .'-]+$/u;
const systems = Object.freeze(catalog.systems.map(system => namingSystem(system.id, system.label,
  system.stocks.map(value => stock(value.id, value.entries)),
  system.forms.map(form => nameForm(form.id, form.slots, values =>
    form.pattern.replace(/\{([a-z][a-z0-9_]*)\}/g, (_, id) => values[id]))),
)));

export const PERSONA_SYSTEMS = systems;

export const NAMING_SYSTEM_OPTIONS = Object.freeze([
  Object.freeze({ id: "surprise", label: "Surprise me" }),
  ...systems.map(({ id, label }) => Object.freeze({ id, label })),
]);

export const PERSONA_SYSTEM_COUNTS = Object.freeze(Object.fromEntries(systems.map(({ id, size }) => [id, size])));
export const NAME_NAMESPACE_SIZE = systems.reduce((total, entry) => total + entry.size, 0n);

export async function nameFor(uuid, requestedSystem = "surprise", variation = 0, cryptoProvider = globalThis.crypto) {
  const normalizedUuid = normalizeUuid(uuid);
  if (requestedSystem !== "surprise" && !systems.some(({ id }) => id === requestedSystem)) {
    throw new TypeError(`unknown persona naming system ${requestedSystem}`);
  }
  if (!Number.isSafeInteger(variation) || variation < 0 || variation > 0xffff_ffff) {
    throw new TypeError("persona variation must be an unsigned 32-bit integer");
  }
  if (!cryptoProvider?.subtle || typeof cryptoProvider.subtle.digest !== "function") {
    throw new TypeError("persona derivation requires a SHA-256 provider");
  }
  for (let counter = 0; counter < MAX_DERIVATION_ATTEMPTS; counter += 1) {
    const entropy = new Uint8Array(await cryptoProvider.subtle.digest(
      "SHA-256",
      encoder.encode(JSON.stringify([DOMAIN, PERSONA_CATALOG_VERSION, normalizedUuid, requestedSystem, variation, counter])),
    ));
    const generated = selectPersona(entropy, requestedSystem, variation);
    if (encoder.encode(generated.name).length <= MAX_FRIENDLY_NAME_BYTES) return generated;
  }
  throw new RangeError("persona derivation could not satisfy the friendly-name UTF-8 bound");
}

function selectPersona(entropy, requestedSystem, variation) {
  let word = 0;
  const nextIndex = (length) => {
    const offset = word++ * 4;
    return new DataView(entropy.buffer, entropy.byteOffset + offset, 4).getUint32(0, false) % length;
  };
  const selectedSystem = requestedSystem === "surprise" ? systems[nextIndex(systems.length)] : systems.find(({ id }) => id === requestedSystem);
  let formTicket = BigInt(nextIndex(Number(selectedSystem.size)));
  const selectedForm = selectedSystem.forms.find((form) => {
    if (formTicket < form.size) return true;
    formTicket -= form.size;
    return false;
  });
  const values = {};
  const stockIndexes = selectedForm.slots.map((stockId) => {
    const selectedStock = selectedSystem.stocks.find(({ id }) => id === stockId);
    const index = nextIndex(selectedStock.entries.length);
    values[stockId] = selectedStock.entries[index];
    return Object.freeze({ stock_id: stockId, index });
  });
  return Object.freeze({
    name: selectedForm.assemble(values),
    version: PERSONA_CATALOG_VERSION,
    system_id: selectedSystem.id,
    system_label: selectedSystem.label,
    form_id: selectedForm.id,
    variation,
    stock_indexes: Object.freeze(stockIndexes),
  });
}

function stock(id, entries) {
  if (!/^[a-z][a-z0-9_]*$/.test(id)
    || entries.length === 0
    || entries.some((entry) => typeof entry !== "string" || entry.length === 0 || !ROMANIZED_COMPONENT.test(entry))
    || new Set(entries).size !== entries.length) {
    throw new TypeError(`invalid persona stock ${id}`);
  }
  return Object.freeze({ id, entries: Object.freeze(entries) });
}

function nameForm(id, slots, assemble) {
  return Object.freeze({ id, slots: Object.freeze(slots), assemble });
}

function namingSystem(id, label, stocks, forms) {
  const stockIds = new Set(stocks.map((entry) => entry.id));
  if (stockIds.size !== stocks.length || forms.some((form) => form.slots.some((slot) => !stockIds.has(slot)))) {
    throw new TypeError(`invalid persona naming system ${id}`);
  }
  const sizedForms = forms.map((form) => Object.freeze({ ...form, size: form.slots.reduce((count, slot) => {
    const selectedStock = stocks.find(({ id: stockId }) => stockId === slot);
    return count * BigInt(selectedStock.entries.length);
  }, 1n) }));
  const size = sizedForms.reduce((total, form) => total + form.size, 0n);
  if (size > 0xffff_ffffn) throw new TypeError(`persona naming system ${id} exceeds its selection bound`);
  return Object.freeze({ id, label, version: PERSONA_CATALOG_VERSION, stocks: Object.freeze(stocks), forms: Object.freeze(sizedForms), size });
}

function normalizeUuid(value) {
  if (typeof value !== "string" || !/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(value)) {
    throw new TypeError("persona derivation requires a canonical UUID string");
  }
  return value.toLowerCase();
}
