const encoder = new TextEncoder();
const decoder = new TextDecoder("utf-8", { fatal: true });
const QUERY = "CONDUIT_BOOTSEL_QUERY@1";
const CHALLENGE_PREFIX = "CONDUIT_BOOTSEL_CHALLENGE@1:";
const REQUEST_PREFIX = "CONDUIT_REBOOT_BOOTSEL@1:";
const ACK = "CONDUIT_REBOOT_BOOTSEL_ACK@1";
const MAXIMUM_FRAME_BYTES = 1024;
const MAXIMUM_READS_PER_FRAME = 4;

export class Rp2040BootselRefusal extends Error {
  constructor(code, message, cause = undefined) {
    super(message, cause ? { cause } : undefined);
    this.name = "Rp2040BootselRefusal";
    this.code = code;
  }
}

function refuse(code, message, cause) {
  throw new Rp2040BootselRefusal(code, message, cause);
}

function frame(text) {
  const payload = encoder.encode(text);
  if (payload.byteLength === 0 || payload.byteLength > MAXIMUM_FRAME_BYTES) {
    refuse("FrameBound", "BOOTSEL control frame exceeds its finite payload bound");
  }
  const bytes = new Uint8Array(payload.byteLength + 2);
  new DataView(bytes.buffer).setUint16(0, payload.byteLength, false);
  bytes.set(payload, 2);
  return bytes;
}

async function readFrame(base) {
  const retained = new Uint8Array(MAXIMUM_FRAME_BYTES + 2);
  let length = 0;
  for (let read = 0; read < MAXIMUM_READS_PER_FRAME; read += 1) {
    const result = await base.read();
    if (!(result.bytes instanceof Uint8Array) || length + result.bytes.byteLength > retained.byteLength) {
      refuse("FrameOverflow", "BOOTSEL response exceeded its finite frame buffer");
    }
    retained.set(result.bytes, length);
    length += result.bytes.byteLength;
    if (length < 2) continue;
    const payloadLength = new DataView(retained.buffer).getUint16(0, false);
    if (payloadLength === 0 || payloadLength > MAXIMUM_FRAME_BYTES) {
      refuse("MalformedFrame", "BOOTSEL response has an invalid length prefix");
    }
    if (length < payloadLength + 2) continue;
    if (length !== payloadLength + 2) {
      refuse("FrameTrailingBytes", "BOOTSEL response carried unadmitted trailing bytes");
    }
    try {
      return decoder.decode(retained.subarray(2, length));
    } catch (error) {
      refuse("MalformedFrame", "BOOTSEL response is not exact UTF-8", error);
    }
  }
  refuse("ProtocolStall", "BOOTSEL response did not complete within four admitted reads");
}

function exactSerialTruth(base) {
  if (!base || ["evidence", "startUse", "write", "read"].some((name) => typeof base[name] !== "function")) {
    refuse("BaseContract", "BOOTSEL reboot requires one admitted Web Serial Base");
  }
  const truth = base.evidence();
  if (
    truth?.schema !== "conduit.browser/web-serial-base-evidence@1"
    || truth.phase !== "resource-truth"
    || truth.resource_class !== "conduit.resource/web-serial-port@1"
    || truth.base_implementation_id !== "browser/web-serial@1"
    || truth.use_authority_contract !== "conduit.authority/use-web-serial@1"
    || truth.usb_vendor_id !== 0x2e8a
    || truth.usb_product_id !== 0x000a
    || truth.configuration?.baud_rate !== 115200
    || truth.transfer_bounds?.maximum_reads < 2
    || truth.transfer_bounds?.maximum_writes < 2
    || truth.transfer_bounds.maximum_in_flight !== 1
    || truth.admitted_reads !== 0
    || truth.admitted_writes !== 0
  ) {
    refuse("BaseTruth", "running Pico serial Base truth is missing, stale, or incompatible");
  }
  return truth;
}

export async function requestRunningFirmwareBootsel({
  base,
  usePlanId,
  operationId,
  expectedBuildId,
  explicitAction = false,
}) {
  if (explicitAction !== true) refuse("ExplicitAction", "BOOTSEL reboot requires an explicit operator action");
  for (const [value, name] of [
    [usePlanId, "use Plan identity"],
    [operationId, "reboot operation identity"],
    [expectedBuildId, "running build identity"],
  ]) {
    if (typeof value !== "string" || value.length === 0 || value.length > 512) {
      refuse("Identity", `${name} is missing or outside its finite bound`);
    }
  }
  const truth = exactSerialTruth(base);
  base.startUse(usePlanId);
  try {
    await base.write(frame(QUERY));
    const challenge = await readFrame(base);
    if (!challenge.startsWith(CHALLENGE_PREFIX)) {
      refuse("Challenge", "running firmware returned a different BOOTSEL protocol");
    }
    const runningBuildId = challenge.slice(CHALLENGE_PREFIX.length);
    if (runningBuildId !== expectedBuildId) {
      refuse("RunningBuild", "BOOTSEL challenge came from a different firmware build");
    }
    await base.write(frame(`${REQUEST_PREFIX}${runningBuildId}`));
    if (await readFrame(base) !== ACK) {
      refuse("Acknowledgement", "running firmware did not acknowledge the exact-build reboot request");
    }
    return Object.freeze({
      schema: "conduit.rp2040/browser-bootsel-reboot-receipt@1",
      operation_id: operationId,
      use_plan_id: usePlanId,
      host_id: truth.host_id,
      boot_id: truth.boot_id,
      resource_handle: truth.resource_handle,
      base_instance_id: truth.base_instance_id,
      use_authority_grant: truth.use_authority_grant,
      running_build_id: runningBuildId,
      terminal: "RebootRequested",
      bootsel_resource_observed: false,
      runtime_truth_created: false,
    });
  } catch (error) {
    if (error instanceof Rp2040BootselRefusal) throw error;
    refuse("BaseTransfer", "running-firmware BOOTSEL request failed through the serial Base", error);
  }
}

export const RP2040_BOOTSEL_CONTROL = Object.freeze({
  query: QUERY,
  challengePrefix: CHALLENGE_PREFIX,
  requestPrefix: REQUEST_PREFIX,
  acknowledgement: ACK,
});
