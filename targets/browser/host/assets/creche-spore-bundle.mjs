const MAGIC = new TextEncoder().encode("CNDSPOR1");
const encoder = new TextEncoder();
const MAXIMUM_MANIFEST_BYTES = 64 * 1024;
// Admit the reviewed 64 MiB SD image while retaining one finite envelope bound.
const MAXIMUM_PAYLOAD_BYTES = 80 * 1024 * 1024;

export function packageSporeBundle({ prepared, artifact, filename }) {
  if (prepared?.spore_manifest?.schema !== "conduit.body/spore-manifest@2"
    || prepared.spore_manifest.spore_id !== prepared.spore_id
    || prepared.spore_manifest.image_content_digest !== prepared.image_content_digest) {
    throw new TypeError("spore bundle requires one exact prepared spore manifest");
  }
  if (!Array.isArray(artifact?.payloads) || artifact.payloads.length < 1 || artifact.payloads.length > 8) {
    throw new TypeError("spore bundle requires one finite artifact payload set");
  }
  const payloads = artifact.payloads.map((payload) => bytesOf(payload));
  const payloadBytes = payloads.reduce((sum, payload) => sum + payload.byteLength, 0);
  if (payloadBytes < 1 || payloadBytes > MAXIMUM_PAYLOAD_BYTES) {
    throw new RangeError("spore bundle artifact exceeds its admitted byte bound");
  }
  const manifest = Object.freeze({
    schema: "conduit.spore/bundle@1",
    spore: prepared.spore_manifest,
    invitation_provision: Object.freeze({
      invitation_id: prepared.invitation_id,
      nonce: prepared.invitation_nonce,
      expires_at_millis: prepared.invitation_expires_at_millis,
      secret: prepared.invitation_secret,
    }),
    artifact: Object.freeze({
      content_digest: prepared.image_content_digest,
      layout: artifact.layout,
      payload_lengths: payloads.map((payload) => payload.byteLength),
    }),
  });
  const manifestBytes = encoder.encode(JSON.stringify(manifest));
  if (manifestBytes.byteLength > MAXIMUM_MANIFEST_BYTES) {
    throw new RangeError("spore bundle manifest exceeds its admitted byte bound");
  }
  const header = new Uint8Array(MAGIC.byteLength + 4);
  header.set(MAGIC);
  new DataView(header.buffer).setUint32(MAGIC.byteLength, manifestBytes.byteLength, true);
  const blob = new Blob([header, manifestBytes, ...payloads], { type: "application/vnd.conduit.spore" });
  return Object.freeze({
    schema: "conduit.spore/browser-download@1",
    filename,
    blob,
    bytes: blob.size,
    spore_id: prepared.spore_id,
    image_content_digest: prepared.image_content_digest,
  });
}

function bytesOf(value) {
  if (value instanceof Uint8Array) return new Uint8Array(value);
  if (ArrayBuffer.isView(value)) return new Uint8Array(value.buffer.slice(value.byteOffset, value.byteOffset + value.byteLength));
  if (value instanceof ArrayBuffer) return new Uint8Array(value.slice(0));
  throw new TypeError("spore bundle artifact payload is not bytes");
}
