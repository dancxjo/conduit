const encoder = new TextEncoder();
const MAXIMUM_FILES = 16;
const MAXIMUM_FILE_BYTES = 32 * 1024 * 1024;
const MAXIMUM_BUNDLE_BYTES = 48 * 1024 * 1024;

export async function acquireHostRelease(profile, signal, {
  manifestUrl = profile?.manifest_path,
  expectedManifest = null,
  fetcher = fetch,
  cache = null,
} = {}) {
  requireProfile(profile);
  const manifestResource = await acquireBytes({
    url: manifestUrl,
    expected: expectedManifest,
    maximumBytes: 256 * 1024,
    signal,
    fetcher,
    cache,
    label: "manifest",
  });
  let manifest;
  try { manifest = JSON.parse(new TextDecoder().decode(manifestResource.bytes)); }
  catch (error) { refuse("StaleArtifact", "reviewed generic Host release manifest is malformed", error); }
  requireManifest(manifest, profile);
  const payloads = [];
  let totalBytes = 0;
  for (const file of manifest.files) {
    const resource = await acquireBytes({
      url: new URL(file.path, manifestResource.url), expected: file,
      maximumBytes: MAXIMUM_FILE_BYTES, signal, fetcher, cache, label: file.path,
    });
    const bytes = resource.bytes;
    totalBytes += bytes.byteLength;
    if (bytes.byteLength !== file.bytes || bytes.byteLength < 1 || bytes.byteLength > MAXIMUM_FILE_BYTES
      || totalBytes > MAXIMUM_BUNDLE_BYTES) {
      refuse("ArtifactBound", "reviewed Host release violated its sealed byte bounds");
    }
    payloads.push(Object.freeze({ ...file, bytes }));
  }
  const bundleDigest = await digestFileIdentities(payloads);
  if (bundleDigest !== manifest.bundle_sha256) refuse("StaleArtifact", "reviewed Host release bundle identity is stale");
  return Object.freeze({ manifest: Object.freeze(manifest), payloads: Object.freeze(payloads), totalBytes });
}

async function acquireBytes({ url, expected, maximumBytes, signal, fetcher, cache, label }) {
  if (expected && cache) {
    const cached = await cache.get(expected.sha256);
    if (cached) {
      const bytes = new Uint8Array(cached);
      if (bytes.byteLength === expected.bytes && await sha256(bytes) === expected.sha256) {
        return Object.freeze({ bytes, url: String(url), cache_hit: true });
      }
    }
  }
  let response;
  try { response = await fetcher(url, { signal, cache: "no-store" }); }
  catch (error) { refuse("ArtifactUnavailable", `reviewed Host release ${label} is unavailable`, error); }
  if (!response?.ok) refuse("ArtifactUnavailable", `reviewed Host release ${label} returned HTTP ${response?.status}`);
  const bytes = new Uint8Array(await response.arrayBuffer());
  if (bytes.byteLength < 1 || bytes.byteLength > maximumBytes
    || expected && (bytes.byteLength !== expected.bytes || await sha256(bytes) !== expected.sha256)) {
    refuse("StaleArtifact", `reviewed Host release ${label} failed its exact identity`);
  }
  if (expected && cache) await cache.put(expected.sha256, bytes);
  return Object.freeze({ bytes, url: response.url || String(url), cache_hit: false });
}

function requireProfile(profile) {
  for (const name of ["target_id", "package_id", "output", "builder_adapter", "deployment_adapter"]) {
    if (typeof profile?.[name] !== "string" || profile[name].length < 1 || profile[name].length > 256) {
      throw new TypeError(`existing-computer target profile omitted ${name}`);
    }
  }
  if (typeof profile.manifest_path !== "string" && typeof profile.release_catalog_key !== "string") {
    throw new TypeError("existing-computer target profile omitted release location");
  }
}

function requireManifest(manifest, profile) {
  if (manifest?.schema !== "conduit.release/host-bundle@1"
    || manifest.target_id !== profile.target_id
    || manifest.fabrication_package_id !== profile.package_id
    || manifest.output !== profile.output
    || manifest.builder_adapter !== profile.builder_adapter
    || manifest.deployment_adapter !== profile.deployment_adapter
    || typeof manifest.source_identity !== "string"
    || !/^sha256:[0-9a-f]{64}$/.test(manifest.bundle_sha256)
    || !Array.isArray(manifest.files) || manifest.files.length < 1 || manifest.files.length > MAXIMUM_FILES) {
    refuse("StaleArtifact", "reviewed generic Host release does not match the exact selected target");
  }
  const paths = new Set();
  for (const file of manifest.files) {
    if (typeof file?.path !== "string" || file.path.length < 1 || file.path.length > 256
      || file.path.includes("/") || paths.has(file.path)
      || !Number.isSafeInteger(file.bytes) || file.bytes < 1 || file.bytes > MAXIMUM_FILE_BYTES
      || !/^sha256:[0-9a-f]{64}$/.test(file.sha256)
      || typeof file.media_type !== "string" || file.media_type.length > 128) {
      refuse("StaleArtifact", "reviewed generic Host release file table is malformed or unbounded");
    }
    paths.add(file.path);
  }
}

async function digestFileIdentities(files) {
  const chunks = [encoder.encode("conduit.release/host-bundle-content@1\0")];
  for (const file of files) chunks.push(encoder.encode(`${file.path}\0${file.sha256}\n`));
  const length = chunks.reduce((sum, chunk) => sum + chunk.length, 0);
  const bytes = new Uint8Array(length);
  let cursor = 0;
  for (const chunk of chunks) { bytes.set(chunk, cursor); cursor += chunk.length; }
  return sha256(bytes);
}

async function sha256(bytes) {
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", bytes));
  return `sha256:${Array.from(digest, (byte) => byte.toString(16).padStart(2, "0")).join("")}`;
}

function refuse(code, message, cause) {
  const error = new Error(message, cause ? { cause } : undefined);
  error.code = code;
  error.evidence = Object.freeze({
    schema: "conduit.release/host-bundle-failure@1",
    terminal: code,
    message,
    authority_requested: false,
    external_work_started: false,
  });
  throw error;
}
