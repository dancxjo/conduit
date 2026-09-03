const MAGIC = new TextEncoder().encode("CNDSPOR1");
const encoder = new TextEncoder();
const MAXIMUM_MANIFEST_BYTES = 64 * 1024;
// Admit the reviewed 64 MiB SD image plus one bounded native provisioning trailer.
const MAXIMUM_PAYLOAD_BYTES = 81 * 1024 * 1024;

export async function createNativeSporeDownload({
  prepared,
  bytes,
  contentDigest,
  filename,
  format,
  mediaType = "application/octet-stream",
}) {
  if (prepared?.spore_manifest?.schema !== "conduit.body/spore-manifest@2"
    || prepared.spore_manifest.spore_id !== prepared.spore_id) {
    throw new TypeError("native spore download requires one exact prepared spore manifest");
  }
  const payload = bytesOf(bytes);
  if (payload.byteLength < 1 || payload.byteLength > MAXIMUM_PAYLOAD_BYTES) {
    throw new RangeError("native spore exceeds its admitted byte bound");
  }
  if (!/^sha256:[0-9a-f]{64}$/.test(contentDigest ?? "")) {
    throw new TypeError("native spore download requires one exact content digest");
  }
  const observed = await sha256(payload);
  if (observed !== contentDigest) {
    throw new TypeError("native spore bytes do not match their exact content digest");
  }
  if (typeof filename !== "string" || filename.length < 1 || filename.length > 192
    || filename.includes("/") || typeof format !== "string" || format.length < 1) {
    throw new TypeError("native spore download requires one bounded target filename and format");
  }
  return Object.freeze({
    schema: "conduit.spore/browser-artifact@1",
    artifact_id: contentDigest,
    filename,
    payload,
    bytes: payload.byteLength,
    maximum_bytes: MAXIMUM_PAYLOAD_BYTES,
    format,
    media_type: mediaType,
    spore_id: prepared.spore_id,
    image_id: prepared.image_id,
    image_content_digest: prepared.image_content_digest,
    spore_content_digest: contentDigest,
  });
}

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
  const bytes = concatenate([header, manifestBytes, ...payloads]);
  if (bytes.byteLength > MAXIMUM_PAYLOAD_BYTES) {
    throw new RangeError("spore bundle exceeds its admitted browser Host handoff bound");
  }
  return Object.freeze({
    schema: "conduit.spore/browser-artifact@1",
    artifact_id: prepared.spore_id,
    filename,
    payload: bytes,
    bytes: bytes.byteLength,
    maximum_bytes: MAXIMUM_PAYLOAD_BYTES,
    format: "spore",
    media_type: "application/vnd.conduit.spore",
    spore_id: prepared.spore_id,
    image_content_digest: prepared.image_content_digest,
  });
}

function concatenate(chunks) {
  const bytes = new Uint8Array(chunks.reduce((sum, chunk) => sum + chunk.byteLength, 0));
  let offset = 0;
  for (const chunk of chunks) { bytes.set(chunk, offset); offset += chunk.byteLength; }
  return bytes;
}

function bytesOf(value) {
  if (value instanceof Uint8Array) return new Uint8Array(value);
  if (ArrayBuffer.isView(value)) return new Uint8Array(value.buffer.slice(value.byteOffset, value.byteOffset + value.byteLength));
  if (value instanceof ArrayBuffer) return new Uint8Array(value.slice(0));
  throw new TypeError("spore bundle artifact payload is not bytes");
}

async function sha256(bytes) {
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", bytes));
  return `sha256:${Array.from(digest, (byte) => byte.toString(16).padStart(2, "0")).join("")}`;
}
