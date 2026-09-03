const encoder = new TextEncoder();
const MAXIMUM_FILES = 16;
const MAXIMUM_FILE_BYTES = 32 * 1024 * 1024;
const MAXIMUM_BUNDLE_BYTES = 48 * 1024 * 1024;

export async function acquireHostRelease(profile, signal) {
  requireProfile(profile);
  let response;
  try {
    response = await fetch(profile.manifest_path, { signal, cache: "no-store" });
  } catch (error) {
    refuse("ArtifactUnavailable", "reviewed generic Host release manifest is unavailable", error);
  }
  if (!response.ok) refuse("ArtifactUnavailable", `reviewed generic Host release manifest returned HTTP ${response.status}`);
  const manifest = await response.json();
  requireManifest(manifest, profile);
  const payloads = [];
  let totalBytes = 0;
  for (const file of manifest.files) {
    const artifactResponse = await fetch(new URL(file.path, response.url), { signal, cache: "no-store" });
    if (!artifactResponse.ok) refuse("ArtifactUnavailable", `reviewed Host release file returned HTTP ${artifactResponse.status}`);
    const bytes = new Uint8Array(await artifactResponse.arrayBuffer());
    totalBytes += bytes.byteLength;
    if (bytes.byteLength !== file.bytes || bytes.byteLength < 1 || bytes.byteLength > MAXIMUM_FILE_BYTES
      || totalBytes > MAXIMUM_BUNDLE_BYTES) {
      refuse("ArtifactBound", "reviewed Host release violated its sealed byte bounds");
    }
    const digest = await sha256(bytes);
    if (digest !== file.sha256) refuse("StaleArtifact", `reviewed Host release file ${file.path} failed its exact digest`);
    payloads.push(Object.freeze({ ...file, bytes }));
  }
  const bundleDigest = await digestFileIdentities(payloads);
  if (bundleDigest !== manifest.bundle_sha256) refuse("StaleArtifact", "reviewed Host release bundle identity is stale");
  return Object.freeze({ manifest: Object.freeze(manifest), payloads: Object.freeze(payloads), totalBytes });
}

function requireProfile(profile) {
  for (const name of ["target_id", "manifest_path", "package_id", "output", "builder_adapter", "deployment_adapter"]) {
    if (typeof profile?.[name] !== "string" || profile[name].length < 1 || profile[name].length > 256) {
      throw new TypeError(`existing-computer target profile omitted ${name}`);
    }
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
