const MAGIC = 0x431fd10b;
const COMMAND_BYTES = 32;
const STATUS_BYTES = 16;
const RESET_REQUEST = 0x41;
const STATUS_REQUEST = 0x42;

const COMMAND = Object.freeze({
  exclusiveAccess: 0x01,
  reboot: 0x02,
  flashErase: 0x03,
  write: 0x05,
  exitXip: 0x06,
  enterCommandXip: 0x07,
});

export class PicobootRefusal extends Error {
  constructor(code, message, cause = undefined) {
    super(message, cause ? { cause } : undefined);
    this.name = "PicobootRefusal";
    this.code = code;
  }
}

function refuse(code, message, cause) {
  throw new PicobootRefusal(code, message, cause);
}

function checkCancelled(signal) {
  if (signal?.aborted) refuse("Cancelled", "RP2040 deployment was explicitly cancelled");
}

function littleEndianWords(...words) {
  const bytes = new Uint8Array(words.length * 4);
  const view = new DataView(bytes.buffer);
  words.forEach((word, index) => view.setUint32(index * 4, word, true));
  return bytes;
}

export function encodePicobootCommand({ token, command, arguments: args = new Uint8Array(), transferBytes = 0 }) {
  if (!Number.isInteger(token) || token <= 0 || token > 0xffffffff) {
    refuse("Token", "PICOBOOT token is outside its exact range");
  }
  if (!Number.isInteger(command) || command < 0 || command > 0xff) {
    refuse("Command", "PICOBOOT command is outside its exact range");
  }
  if (!(args instanceof Uint8Array) || args.byteLength > 16) {
    refuse("CommandArguments", "PICOBOOT arguments exceed the 16-byte command bound");
  }
  if (!Number.isInteger(transferBytes) || transferBytes < 0 || transferBytes > 4096) {
    refuse("TransferLength", "PICOBOOT data length exceeds the admitted Base transfer bound");
  }
  const packet = new Uint8Array(COMMAND_BYTES);
  const view = new DataView(packet.buffer);
  view.setUint32(0, MAGIC, true);
  view.setUint32(4, token, true);
  view.setUint8(8, command);
  view.setUint8(9, args.byteLength);
  view.setUint16(10, 0, true);
  view.setUint32(12, transferBytes, true);
  packet.set(args, 16);
  return packet;
}

function parseStatus(bytes, expected) {
  if (!(bytes instanceof Uint8Array) || bytes.byteLength !== STATUS_BYTES) {
    refuse("StatusLength", "PICOBOOT status did not retain its exact 16-byte shape");
  }
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const status = Object.freeze({
    token: view.getUint32(0, true),
    statusCode: view.getUint32(4, true),
    command: view.getUint8(8),
    inProgress: view.getUint8(9),
  });
  if (status.token !== expected.token || status.command !== expected.command) {
    refuse("StaleStatus", "PICOBOOT status belongs to a different command identity");
  }
  if (status.inProgress !== 0) refuse("ProtocolStall", "PICOBOOT command remained in progress");
  if (status.statusCode !== 0) {
    refuse("CommandFailed", `PICOBOOT command ${status.command} failed with status ${status.statusCode}`);
  }
  return status;
}

async function execute(base, interfaceNumber, token, command, args, data, signal) {
  checkCancelled(signal);
  const payload = data ?? new Uint8Array();
  try {
    await base.transferOut(encodePicobootCommand({
      token,
      command,
      arguments: args,
      transferBytes: payload.byteLength,
    }));
    if (payload.byteLength > 0) await base.transferOut(payload);
    const acknowledgement = await base.transferIn(1);
    if (acknowledgement.bytes.byteLength !== 0) {
      refuse("Acknowledgement", "PICOBOOT acknowledgement was not zero length");
    }
    const response = await base.controlTransferIn({
      requestType: "vendor",
      recipient: "interface",
      request: STATUS_REQUEST,
      value: 0,
      index: interfaceNumber,
    }, STATUS_BYTES);
    return parseStatus(response.bytes, { token, command });
  } catch (error) {
    if (error instanceof PicobootRefusal) throw error;
    refuse("BaseTransfer", "PICOBOOT transfer failed through the admitted browser Base", error);
  }
}

export function requiredPicobootTransfers(chunkCount) {
  if (!Number.isInteger(chunkCount) || chunkCount <= 0 || chunkCount > 512) {
    refuse("ChunkBound", "RP2040 IMAGE chunk count exceeds the finite flash bound");
  }
  return Object.freeze({
    maximumOutTransfers: 2 * chunkCount + 6,
    maximumInTransfers: 2 * chunkCount + 10,
  });
}

export async function deployPicoboot({ base, image, interfaceNumber, signal, progress = () => {} }) {
  let token = 1;
  const run = async (command, args = new Uint8Array(), data = new Uint8Array()) => {
    const currentToken = token;
    token += 1;
    const status = await execute(base, interfaceNumber, currentToken, command, args, data, signal);
    progress(Object.freeze({ token: currentToken, command, statusCode: status.statusCode }));
  };

  checkCancelled(signal);
  try {
    await base.controlTransferOut({
      requestType: "vendor",
      recipient: "interface",
      request: RESET_REQUEST,
      value: 0,
      index: interfaceNumber,
    });
  } catch (error) {
    refuse("ResetFailed", "PICOBOOT interface reset failed", error);
  }
  await run(COMMAND.exclusiveAccess, new Uint8Array([2]));
  await run(COMMAND.exitXip);
  await run(COMMAND.flashErase, littleEndianWords(image.eraseStart, image.eraseBytes));
  for (const chunk of image.chunks) {
    await run(
      COMMAND.write,
      littleEndianWords(chunk.address, chunk.bytes.byteLength),
      chunk.bytes,
    );
  }
  await run(COMMAND.enterCommandXip);
  await run(COMMAND.reboot, littleEndianWords(0, 0, 500));
  return Object.freeze({ commands: token - 1, lastToken: token - 1 });
}

export const PICOBOOT = Object.freeze({
  magic: MAGIC,
  commandBytes: COMMAND_BYTES,
  statusBytes: STATUS_BYTES,
  resetRequest: RESET_REQUEST,
  statusRequest: STATUS_REQUEST,
  commands: COMMAND,
});
