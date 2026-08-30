const PROTOCOL = 2;
const MAX_FRAME_BYTES = 4096;
const encoder = new TextEncoder();
const decoder = new TextDecoder("utf-8", { fatal: true });

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
  while (state.length < 2 || state.length < new DataView(state.buffer, state.byteOffset, state.byteLength).getUint16(0, false) + 2) {
    const { bytes } = await base.read();
    if (state.length + bytes.byteLength > (MAX_FRAME_BYTES + 2) * 2) {
      refuse("Oversized", "spawn response exceeds its admitted frame bound");
    }
    const joined = new Uint8Array(state.length + bytes.byteLength);
    joined.set(state);
    joined.set(bytes, state.length);
    state = joined;
  }
  const length = new DataView(state.buffer, state.byteOffset, state.byteLength).getUint16(0, false);
  if (length === 0 || length > MAX_FRAME_BYTES) {
    refuse("Malformed", "spawn response is not one exact bounded frame");
  }
  return {
    payload: state.slice(2, length + 2),
    remainder: state.slice(length + 2),
  };
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

export async function requestRp2040SpawnJoin({ base, prepared }) {
  if (!base || ["startUse", "write", "read", "setSignals", "evidence"].some((name) => typeof base[name] !== "function")) {
    refuse("BaseContract", "Pico spawn observation requires one admitted browser serial Base");
  }
  requireIdentity(prepared?.spore_id, undefined, "spore identity");
  requireIdentity(prepared?.image_id, undefined, "IMAGE identity");
  requireIdentity(prepared?.invitation_id, undefined, "invitation identity");
  requireIdentity(prepared?.body_id, undefined, "Body identity");
  requireBytes(prepared?.invitation_nonce, 32, "invitation nonce");
  requireBytes(prepared?.invitation_secret, 32, "invitation secret");
  if (!Number.isSafeInteger(prepared.invitation_expires_at_millis)
    || prepared.invitation_expires_at_millis <= 0) {
    refuse("Malformed", "invitation expiry is missing or outside its finite bound");
  }
  if (prepared.invitation_expires_at_millis <= Date.now()) {
    prepared.invitation_secret.fill(0);
    refuse("ExpiredInvitation", "prepared invitation expired before the Boot/join request");
  }

  const usePlanId = `pico-spawn/${prepared.spore_id}`;
  try {
    base.startUse(usePlanId);
    await base.setSignals({ dataTerminalReady: true });
    const provision = encoder.encode(JSON.stringify({
      protocol: PROTOCOL,
      spore_id: prepared.spore_id,
      image_id: prepared.image_id,
      invitation_id: prepared.invitation_id,
      body_id: prepared.body_id,
      nonce: prepared.invitation_nonce,
      expires_at_millis: prepared.invitation_expires_at_millis,
      secret: prepared.invitation_secret,
    }));
    await base.write(frame(provision));
  } finally {
    prepared.invitation_secret.fill(0);
  }

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
    schema: "conduit.rp2040/browser-spawn-observation@1",
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
  });
}
