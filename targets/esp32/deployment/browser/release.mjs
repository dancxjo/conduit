const MAXIMUM_ARTIFACT_BYTES = 4 * 1024 * 1024;
const decoder = new TextDecoder("utf-8", { fatal: true });

export async function acquireEsp32Release(targetProfile, signal, { resolved = null, fetcher = fetch } = {}) {
  let response;
  let manifest;
  if (resolved) {
    try { manifest = JSON.parse(decoder.decode(resolved.manifest)); }
    catch (error) { refuse("StaleArtifact", "catalog-resolved ESP32 release manifest is malformed", error); }
  } else {
    try { response = await fetcher(targetProfile.artifactManifest, { signal, cache: "no-store" }); }
    catch (error) { refuse("ArtifactUnavailable", "generic ESP32 release manifest is unavailable", error); }
    if (!response.ok) refuse("ArtifactUnavailable", `generic ESP32 release manifest returned HTTP ${response.status}`);
    manifest = await response.json();
  }
  if (manifest?.schema !== "conduit.release/target-artifact@1"
    || manifest.target_id !== targetProfile.target.id
    || typeof manifest.source_identity !== "string"
    || typeof manifest.image_id !== "string"
    || !Array.isArray(manifest.segments) || manifest.segments.length < 1 || manifest.segments.length > 8) {
    refuse("StaleArtifact", "generic ESP32 release manifest does not match the exact target profile");
  }
  const segments = [];
  let total = 0;
  for (const segment of manifest.segments) {
    if (!Number.isSafeInteger(segment.offset) || segment.offset < 0 || typeof segment.path !== "string") {
      refuse("StaleArtifact", "generic ESP32 release segment layout is malformed");
    }
    let bytes;
    if (resolved) {
      bytes = await resolved.acquire(segment, MAXIMUM_ARTIFACT_BYTES - total, signal);
    } else {
      const artifactResponse = await fetcher(new URL(segment.path, response.url), { signal, cache: "no-store" });
      if (!artifactResponse.ok) refuse("ArtifactUnavailable", `generic ESP32 release artifact returned HTTP ${artifactResponse.status}`);
      bytes = new Uint8Array(await artifactResponse.arrayBuffer());
    }
    total += bytes.byteLength;
    if (bytes.byteLength !== segment.bytes || total > MAXIMUM_ARTIFACT_BYTES) {
      refuse("ArtifactBound", "generic ESP32 release artifact violated its sealed byte bounds");
    }
    segments.push(Object.freeze({ offset: segment.offset, bytes }));
  }
  const raw = new Uint8Array(total);
  let cursor = 0;
  for (const segment of segments) { raw.set(segment.bytes, cursor); cursor += segment.bytes.byteLength; }
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", raw));
  const rawDigest = `sha256:${Array.from(digest, (byte) => byte.toString(16).padStart(2, "0")).join("")}`;
  if (manifest.artifact_sha256 !== rawDigest || manifest.bytes !== total) {
    refuse("StaleArtifact", "generic ESP32 release artifact identity does not match its reviewed manifest");
  }
  return Object.freeze({ manifest: Object.freeze(manifest), segments: Object.freeze(segments) });
}

function refuse(code, message, cause) {
  const error = new Error(message, cause ? { cause } : undefined);
  error.code = code;
  error.evidence = Object.freeze({
    schema: "conduit.esp32/release-acquisition-refusal@1",
    terminal: code,
    authority_requested: false,
    body_binding_started: false,
  });
  throw error;
}
