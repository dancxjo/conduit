const PROTOCOL = 2;
const MAX_FRAME_BYTES = 4094;
const MAX_STREAM_READ_BYTES = MAX_FRAME_BYTES + 2;
const MAX_TOTAL_RESPONSE_BYTES = MAX_STREAM_READ_BYTES * 2;
const MAX_STREAM_CHUNKS_PER_READ = MAX_STREAM_READ_BYTES;
const MAX_STREAM_READ_MILLIS = 10_000;
const encoder = new TextEncoder();
const decoder = new TextDecoder("utf-8", { fatal: true });
const NATIVE_SPORE_JOIN_REQUEST = encoder.encode("CONDUIT_SPORE_JOIN@1");

export class Rp2040SpawnRefusal extends Error {
  constructor(code, message, cause = undefined) {
    super(message, cause ? { cause } : undefined);
    this.name = "Rp2040SpawnRefusal";
    this.code = code;
  }
}

function refuse(code, message, cause) {
  throw new Rp2040SpawnRefusal(code, message, cause);
}

function requireIdentity(value, expected, name) {
  if (typeof value !== "string" || value.length === 0 || value.length > 128) {
    refuse("Identity", `${name} is missing or outside its finite bound`);
  }
  if (expected !== undefined && value !== expected) {
    const code = {
      "spore identity": "WrongSpore",
      "IMAGE identity": "WrongImage",
      "invitation identity": "WrongInvitation",
      "Body identity": "WrongBody",
      "Host identity": "WrongHost",
      "Boot identity": "WrongBoot",
    }[name] ?? "WrongIdentity";
    refuse(code, `${name} does not match the prepared spore`);
  }
}

function requireBytes(value, length, name) {
  if (!Array.isArray(value) || value.length !== length
    || value.some((byte) => !Number.isInteger(byte) || byte < 0 || byte > 255)) {
    refuse("Malformed", `${name} is not an exact ${length}-byte value`);
  }
}

function frame(payload) {
  if (payload.byteLength === 0 || payload.byteLength > MAX_FRAME_BYTES) {
    refuse("Oversized", "spawn provision exceeds its admitted frame bound");
  }
  const bytes = new Uint8Array(payload.byteLength + 2);
  new DataView(bytes.buffer).setUint16(0, payload.byteLength, false);
  bytes.set(payload, 2);
  return bytes;
}

async function readFrame(base, state) {
  let chunks = 0;
  if (!completeFrameAvailable(state)) {
    const remainingBytes = MAX_STREAM_READ_BYTES - state.length;
    if (remainingBytes < 1) refuse("Oversized", "spawn response exceeds its admitted frame bound");
    const result = await base.readStream({
      maximumBytes: remainingBytes,
      maximumChunks: remainingBytes,
      timeoutMillis: MAX_STREAM_READ_MILLIS,
      complete: (bytes) => completeFrameAvailable(state, bytes),
    });
    const { bytes } = result;
    const joined = new Uint8Array(state.length + bytes.byteLength);
    joined.set(state);
    joined.set(bytes, state.length);
    state = joined;
    chunks = result.chunks;
  }
  const length = new DataView(state.buffer, state.byteOffset, state.byteLength).getUint16(0, false);
  if (length === 0 || length > MAX_FRAME_BYTES) {
    refuse("Malformed", "spawn response is not one exact bounded frame");
  }
  return {
    payload: state.slice(2, length + 2),
    remainder: state.slice(length + 2),
    chunks,
  };
}

function completeFrameAvailable(prior, incoming = new Uint8Array()) {
  const total = prior.length + incoming.length;
  if (total < 2) return false;
  const byte = (index) => index < prior.length ? prior[index] : incoming[index - prior.length];
  const length = (byte(0) << 8) | byte(1);
  return length === 0 || length > MAX_FRAME_BYTES || total >= length + 2;
}

function parseObject(bytes, name) {
  try {
    const value = JSON.parse(decoder.decode(bytes));
    if (!value || typeof value !== "object" || Array.isArray(value)) throw new TypeError();
    return value;
  } catch (error) {
    refuse("Malformed", `${name} is not canonical JSON`, error);
  }
}

export async function requestPhysicalSpawnJoin({
  base,
  prepared,
  evidenceSchema,
  usePlanPrefix,
  subject = "physical Host",
}) {
  if (!base || ["startUse", "write", "readStream", "setSignals", "evidence"].some((name) => typeof base[name] !== "function")) {
    refuse("BaseContract", `${subject} spawn observation requires one admitted browser serial Base`);
  }
  requireIdentity(prepared?.spore_id, undefined, "spore identity");
  requireIdentity(prepared?.image_id, undefined, "IMAGE identity");
  requireIdentity(prepared?.invitation_id, undefined, "invitation identity");
  requireIdentity(prepared?.body_id, undefined, "Body identity");
  requireBytes(prepared?.invitation_nonce, 32, "invitation nonce");
  if (!Number.isSafeInteger(prepared.invitation_expires_at_millis)
    || prepared.invitation_expires_at_millis <= 0) {
    refuse("Malformed", "invitation expiry is missing or outside its finite bound");
  }
  if (prepared.invitation_expires_at_millis <= Date.now()) {
    refuse("ExpiredInvitation", "prepared invitation expired before the Boot/join request");
  }

  requireIdentity(evidenceSchema, undefined, "evidence schema");
  requireIdentity(usePlanPrefix, undefined, "use Plan prefix");
  const usePlanId = `${usePlanPrefix}/${prepared.spore_id}`;
  base.startUse(usePlanId);
  await base.setSignals({ dataTerminalReady: true });
  await base.write(frame(NATIVE_SPORE_JOIN_REQUEST));

  const first = await readFrame(base, new Uint8Array());
  const second = await readFrame(base, first.remainder);
  if (second.remainder.length !== 0) {
    refuse("Malformed", "spawn response contains unadmitted trailing data");
  }
  const advertisementEnvelope = parseObject(first.payload, "Boot advertisement");
  const join = parseObject(second.payload, "join request");
  if (advertisementEnvelope.protocol !== 1 || !advertisementEnvelope.advertisement) {
    refuse("WrongAdvertisement", "first response is not one current Pico advertisement");
  }
  const advertisement = advertisementEnvelope.advertisement;
  if (join.protocol !== PROTOCOL) refuse("WrongProtocol", "join request uses the wrong protocol");
  requireIdentity(join.spore_id, prepared.spore_id, "spore identity");
  requireIdentity(join.image_id, prepared.image_id, "IMAGE identity");
  requireIdentity(join.invitation_id, prepared.invitation_id, "invitation identity");
  requireIdentity(join.body_id, prepared.body_id, "Body identity");
  requireIdentity(join.host_id, advertisement.host_id, "Host identity");
  requireIdentity(join.boot_id, advertisement.boot_id, "Boot identity");
  if (join.offer_generation !== advertisement.offer_generation) {
    refuse("WrongAdvertisement", "join request offer generation does not match the observed advertisement");
  }
  requireBytes(join.nonce, 32, "join nonce");
  requireBytes(join.signature, 64, "join signature");
  if (join.nonce.some((byte, index) => byte !== prepared.invitation_nonce[index])) {
    refuse("WrongInvitation", "join request nonce does not match the prepared invitation");
  }
  return Object.freeze({
    schema: evidenceSchema,
    spore_id: join.spore_id,
    image_id: join.image_id,
    advertisement,
    invitation_id: join.invitation_id,
    body_id: join.body_id,
    host_id: join.host_id,
    boot_id: join.boot_id,
    nonce: join.nonce,
    signature: join.signature,
    observed_at_millis: Date.now(),
    serial_use_plan_id: usePlanId,
    serial_stream: Object.freeze({
      response_frames: 2,
      browser_chunks: first.chunks + second.chunks,
      admitted_reads: Number(first.chunks > 0) + Number(second.chunks > 0),
      maximum_read_bytes: MAX_STREAM_READ_BYTES,
      maximum_total_response_bytes: MAX_TOTAL_RESPONSE_BYTES,
      maximum_chunks_per_read: MAX_STREAM_CHUNKS_PER_READ,
      maximum_read_millis: MAX_STREAM_READ_MILLIS,
    }),
  });
}

export function requestRp2040SpawnJoin({ base, prepared }) {
  return requestPhysicalSpawnJoin({
    base,
    prepared,
    evidenceSchema: "conduit.rp2040/browser-spawn-observation@1",
    usePlanPrefix: "pico-spawn",
    subject: "Pico",
  });
}

export const PHYSICAL_SPAWN_STREAM_BOUNDS = Object.freeze({
  maximumTransferBytes: MAX_STREAM_READ_BYTES,
  maximumReads: 2,
  maximumWrites: 1,
  maximumSignalOperations: 1,
  maximumChunksPerRead: MAX_STREAM_CHUNKS_PER_READ,
  maximumReadMillis: MAX_STREAM_READ_MILLIS,
});
