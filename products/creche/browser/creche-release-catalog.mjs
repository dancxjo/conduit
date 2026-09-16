import { acquireHostRelease } from "./creche-release-bundle.mjs";

const SCHEMA = "conduit.release/catalog@1";
const MAXIMUM_ENTRIES = 64;
const MAXIMUM_CATALOG_BYTES = 128 * 1024;
const MAXIMUM_MANIFEST_BYTES = 256 * 1024;
const encoder = new TextEncoder();

export function createMemoryReleaseCache() {
  const values = new Map();
  return Object.freeze({
    async get(digest) {
      const bytes = values.get(digest);
      return bytes ? bytes.slice() : null;
    },
    async put(digest, bytes) {
      values.set(digest, bytes.slice());
    },
  });
}

export async function openReleaseCatalog({ source, minimumGeneration = 0, signal, fetcher = fetch, cache } = {}) {
  if (!(source instanceof URL) && (typeof source !== "string" || source.length < 1 || source.length > 2048)) {
    throw new TypeError("release catalog source is missing or outside its finite bound");
  }
  if (!Number.isSafeInteger(minimumGeneration) || minimumGeneration < 0) {
    throw new TypeError("release catalog minimum generation is invalid");
  }
  const response = await fetcher(source, { signal, cache: "no-store" }).catch((error) => {
    refuse("CatalogUnavailable", "reviewed release catalog is unavailable", error);
  });
  if (!response?.ok) refuse("CatalogUnavailable", `reviewed release catalog returned HTTP ${response?.status}`);
  const bytes = new Uint8Array(await response.arrayBuffer());
  if (bytes.byteLength < 2 || bytes.byteLength > MAXIMUM_CATALOG_BYTES) {
    refuse("CatalogBound", "reviewed release catalog violates its admitted byte bound");
  }
  let catalog;
  try { catalog = JSON.parse(new TextDecoder().decode(bytes)); }
  catch (error) { refuse("CatalogMalformed", "reviewed release catalog is not finite JSON", error); }
  await requireCatalog(catalog, minimumGeneration);
  const catalogUrl = new URL(response.url || String(source), globalThis.location?.href ?? "http://localhost/");

  return Object.freeze({
    schema: SCHEMA,
    generation: catalog.generation,
    catalog_id: catalog.catalog_id,
    targets: Object.freeze(catalog.entries.map((entry) => entry.target_id)),
    async acquire(profile, acquireSignal = signal) {
      const entry = catalog.entries.find((candidate) => candidate.target_id === profile?.target_id);
      if (!entry) refuse("UnknownTarget", "selected target is absent from the reviewed release catalog");
      requireEntryMatchesProfile(entry, profile);
      const manifestUrl = new URL(entry.manifest.path, catalogUrl);
      return acquireHostRelease(profile, acquireSignal, {
        manifestUrl,
        expectedManifest: entry.manifest,
        fetcher,
        cache,
      });
    },
  });
}

export async function acquireConfiguredHostRelease(host, profile, signal) {
  if (!host?.releaseCatalogSource) return acquireHostRelease(profile, signal);
  const release = await openReleaseCatalog({
    source: host.releaseCatalogSource,
    minimumGeneration: host.minimumReleaseCatalogGeneration ?? 0,
    signal,
    cache: host.releaseArtifactCache,
  });
  return release.acquire(profile, signal);
}

async function requireCatalog(catalog, minimumGeneration) {
  if (catalog?.schema !== SCHEMA
    || !Number.isSafeInteger(catalog.generation) || catalog.generation <= minimumGeneration
    || typeof catalog.catalog_id !== "string" || !/^sha256:[0-9a-f]{64}$/.test(catalog.catalog_id)
    || !Array.isArray(catalog.entries) || catalog.entries.length < 1 || catalog.entries.length > MAXIMUM_ENTRIES) {
    refuse("StaleCatalog", "reviewed release catalog identity, generation, or bounds are invalid");
  }
  const targets = new Set();
  for (const entry of catalog.entries) {
    if (!bounded(entry?.target_id) || targets.has(entry.target_id)
      || !bounded(entry.package_id) || !bounded(entry.output)
      || !bounded(entry.builder_adapter) || !bounded(entry.deployment_adapter)
      || !validArtifact(entry.manifest, MAXIMUM_MANIFEST_BYTES)) {
      refuse("CatalogMalformed", "reviewed release catalog contains an invalid or duplicate target descriptor");
    }
    targets.add(entry.target_id);
  }
  const identityBytes = encoder.encode(JSON.stringify({
    schema: catalog.schema,
    generation: catalog.generation,
    entries: catalog.entries,
  }));
  if (await sha256(identityBytes) !== catalog.catalog_id) {
    refuse("CatalogIdentityMismatch", "release catalog content does not match its exact catalog identity");
  }
}

async function sha256(bytes) {
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", bytes));
  return `sha256:${Array.from(digest, (byte) => byte.toString(16).padStart(2, "0")).join("")}`;
}

function requireEntryMatchesProfile(entry, profile) {
  for (const name of ["target_id", "package_id", "output", "builder_adapter", "deployment_adapter"]) {
    if (entry[name] !== profile?.[name]) {
      refuse("CatalogMismatch", "release catalog cannot relabel an artifact for a different target profile");
    }
  }
}

function validArtifact(value, maximumBytes) {
  return value && bounded(value.path) && !value.path.includes("..")
    && Number.isSafeInteger(value.bytes) && value.bytes > 0 && value.bytes <= maximumBytes
    && /^sha256:[0-9a-f]{64}$/.test(value.sha256);
}

function bounded(value) {
  return typeof value === "string" && value.length > 0 && value.length <= 256;
}

function refuse(code, message, cause) {
  const error = new Error(message, cause ? { cause } : undefined);
  error.code = code;
  error.evidence = Object.freeze({
    schema: "conduit.release/catalog-failure@1",
    terminal: code,
    message,
    body_secret_observed: false,
    body_binding_started: false,
  });
  throw error;
}
