export const BrowserDomFailure = Object.freeze({
  InvalidBinding: "CND-BRW-S4-001",
  InvalidFace: "CND-BRW-S4-002",
  DuplicateFace: "CND-BRW-S4-003",
  ReceiptCapacity: "CND-BRW-S4-004",
});

const SIGNAL_VALUE_KIND = "value/signal";
const SIGNAL_FACE_KIND = "face/signal";
const BOOL_VALUE_KIND = "value/bool";
const BOOL_FACE_KIND = "face/bool";
const LOCAL_FACE_EFFECT = 2;
const DISTRIBUTED_FACE_EFFECT = 3;
const SIGNAL_ENCODED_LENGTH = 9;
const MAXIMUM_IDENTITY_LENGTH = 256;

function failure(code, detail) {
  return Object.freeze({ ok: false, code, detail });
}

function boundedIdentity(value) {
  return typeof value === "string" &&
    value.length > 0 &&
    value.length <= MAXIMUM_IDENTITY_LENGTH;
}

function exactBytes(value, length = SIGNAL_ENCODED_LENGTH) {
  if (!Array.isArray(value) || value.length !== length) return null;
  if (value.some((byte) => !Number.isInteger(byte) || byte < 0 || byte > 255)) return null;
  return Uint8Array.from(value);
}

function decodeBoolean(value, sequence) {
  if (value?.valueKind !== BOOL_VALUE_KIND) return null;
  const encoded = exactBytes(value.encoded, 1);
  if (!encoded || encoded[0] > 1) return null;
  return Object.freeze({
    sequence: BigInt(sequence),
    level: encoded[0] === 1,
    encoded: Object.freeze([...encoded]),
  });
}

function decodeSignal(value) {
  if (value?.valueKind !== SIGNAL_VALUE_KIND) return null;
  const encoded = exactBytes(value.encoded);
  if (!encoded || encoded[8] > 1) return null;
  const sequence = new DataView(encoded.buffer).getBigUint64(0, true);
  return Object.freeze({
    sequence,
    level: encoded[8] === 1,
    encoded: Object.freeze([...encoded]),
  });
}

export class BrowserDomHost {
  #faceIds = new Set();
  #receipts = [];
  #receiptBytes = 0;

  constructor({ hostId, bootId, root, maximumReceiptItems, maximumReceiptBytes }) {
    if (!boundedIdentity(hostId) ||
        !boundedIdentity(bootId) ||
        !(root instanceof Element) ||
        !Number.isSafeInteger(maximumReceiptItems) ||
        maximumReceiptItems <= 0 ||
        !Number.isSafeInteger(maximumReceiptBytes) ||
        maximumReceiptBytes <= 0) {
      throw new TypeError(BrowserDomFailure.InvalidBinding);
    }
    this.hostId = hostId;
    this.bootId = bootId;
    this.root = root;
    this.maximumReceiptItems = maximumReceiptItems;
    this.maximumReceiptBytes = maximumReceiptBytes;
  }

  completeFace(effect) {
    const identityFields = [
      effect?.sourceDocumentId,
      effect?.checkedPlotId,
      effect?.expandedPlotId,
      effect?.planId,
      effect?.fragmentId,
      effect?.hostId,
      effect?.bootId,
      effect?.activePlayId,
      effect?.faceId,
      effect?.signId,
      effect?.hostCallContractId,
      effect?.placementId,
    ];
    const remoteIdentityFields = [
      effect?.sourceFragmentId,
      effect?.sourceHostId,
      effect?.sourceBootId,
      effect?.sourceActivePlayId,
      effect?.sourceEndpointId,
      effect?.sinkEndpointId,
      effect?.connectionId,
      effect?.linkBindingId,
      effect?.baseInstanceId,
    ];
    const hasRemoteIdentity = remoteIdentityFields.some((value) => value !== undefined);
    const requiresRemoteIdentity = effect?.kind === DISTRIBUTED_FACE_EFFECT;
    const face = effect?.faceKind === SIGNAL_FACE_KIND
      ? decodeSignal(effect?.value)
      : effect?.faceKind === BOOL_FACE_KIND
        ? decodeBoolean(effect?.value, this.#receipts.length)
        : null;
    if (identityFields.some((value) => !boundedIdentity(value)) ||
        (effect?.kind !== LOCAL_FACE_EFFECT && !requiresRemoteIdentity) ||
        requiresRemoteIdentity !== hasRemoteIdentity ||
        (requiresRemoteIdentity &&
          remoteIdentityFields.some((value) => !boundedIdentity(value))) ||
        effect?.hostId !== this.hostId ||
        effect?.bootId !== this.bootId ||
        !Number.isSafeInteger(effect?.requestNode) ||
        !Number.isSafeInteger(effect?.requestId) ||
        !Number.isSafeInteger(effect?.operationId) ||
        !face) {
      return failure(BrowserDomFailure.InvalidFace, "malformed-effect");
    }
    if (this.#faceIds.has(effect.faceId)) {
      return failure(BrowserDomFailure.DuplicateFace, effect.faceId);
    }
    if (this.#receipts.length === this.maximumReceiptItems ||
        this.#receiptBytes + face.encoded.length > this.maximumReceiptBytes) {
      return failure(BrowserDomFailure.ReceiptCapacity, "receipt-store-full");
    }

    const receipt = Object.freeze({
      hostId: this.hostId,
      bootId: this.bootId,
      sourceDocumentId: effect.sourceDocumentId,
      checkedPlotId: effect.checkedPlotId,
      expandedPlotId: effect.expandedPlotId,
      planId: effect.planId,
      fragmentId: effect.fragmentId,
      activePlayId: effect.activePlayId,
      faceId: effect.faceId,
      signId: effect.signId,
      requestNode: effect.requestNode,
      requestId: effect.requestId,
      operationId: effect.operationId,
      hostCallContractId: effect.hostCallContractId,
      placementId: effect.placementId,
      sequence: face.sequence.toString(),
      level: face.level,
      encoded: face.encoded,
      ...(hasRemoteIdentity
        ? {
            sourceFragmentId: effect.sourceFragmentId,
            sourceHostId: effect.sourceHostId,
            sourceBootId: effect.sourceBootId,
            sourceActivePlayId: effect.sourceActivePlayId,
            sourceEndpointId: effect.sourceEndpointId,
            sinkEndpointId: effect.sinkEndpointId,
            connectionId: effect.connectionId,
            linkBindingId: effect.linkBindingId,
            baseInstanceId: effect.baseInstanceId,
          }
        : {}),
    });
    const output = document.createElement("output");
    output.dataset.hostId = receipt.hostId;
    output.dataset.bootId = receipt.bootId;
    output.dataset.planId = receipt.planId;
    output.dataset.fragmentId = receipt.fragmentId;
    output.dataset.activePlayId = receipt.activePlayId;
    output.dataset.faceId = receipt.faceId;
    output.dataset.signId = receipt.signId;
    output.dataset.requestId = String(receipt.requestId);
    output.dataset.placementId = receipt.placementId;
    output.dataset.sequence = receipt.sequence;
    output.dataset.level = String(receipt.level);
    output.textContent =
      `receipt ${effect.faceKind} host=${receipt.hostId} boot=${receipt.bootId} ` +
      `placement=${receipt.placementId} sequence=${receipt.sequence} level=${receipt.level}`;
    this.root.append(output);

    this.#faceIds.add(effect.faceId);
    this.#receipts.push(receipt);
    this.#receiptBytes += face.encoded.length;
    return Object.freeze({
      ok: true,
      completion: Object.freeze({
        sourceDocumentId: effect.sourceDocumentId,
        checkedPlotId: effect.checkedPlotId,
        expandedPlotId: effect.expandedPlotId,
        planId: effect.planId,
        fragmentId: effect.fragmentId,
        hostId: effect.hostId,
        bootId: effect.bootId,
        activePlayId: effect.activePlayId,
        requestNode: effect.requestNode,
        requestId: effect.requestId,
        operationId: effect.operationId,
        hostCallContractId: effect.hostCallContractId,
        faceId: effect.faceId,
        signId: effect.signId,
        placementId: effect.placementId,
        ...(hasRemoteIdentity
          ? {
              sourceFragmentId: effect.sourceFragmentId,
              sourceHostId: effect.sourceHostId,
              sourceBootId: effect.sourceBootId,
              sourceActivePlayId: effect.sourceActivePlayId,
              sourceEndpointId: effect.sourceEndpointId,
              sinkEndpointId: effect.sinkEndpointId,
              connectionId: effect.connectionId,
              linkBindingId: effect.linkBindingId,
              baseInstanceId: effect.baseInstanceId,
            }
          : {}),
        value: Object.freeze({
          valueKind: effect.value.valueKind,
          encoded: face.encoded,
        }),
        success: true,
      }),
      receipt,
    });
  }

  receipts() {
    return Object.freeze([...this.#receipts]);
  }
}
