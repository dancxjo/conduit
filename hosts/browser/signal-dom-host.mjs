export const BrowserDomFailure = Object.freeze({
  InvalidBinding: "CND-BRW-S4-001",
  InvalidPresentation: "CND-BRW-S4-002",
  DuplicatePresentation: "CND-BRW-S4-003",
  ReceiptCapacity: "CND-BRW-S4-004",
});

const SIGNAL_VALUE_KIND = "value/signal";
const SIGNAL_PRESENTATION_KIND = "presentation/signal";
const LOCAL_PRESENTATION_EFFECT = 2;
const DISTRIBUTED_PRESENTATION_EFFECT = 3;
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

function exactBytes(value) {
  if (!Array.isArray(value) || value.length !== SIGNAL_ENCODED_LENGTH) return null;
  if (value.some((byte) => !Number.isInteger(byte) || byte < 0 || byte > 255)) return null;
  return Uint8Array.from(value);
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
  #presentationIds = new Set();
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

  completePresentation(effect) {
    const identityFields = [
      effect?.sourceDocumentId,
      effect?.checkedFormId,
      effect?.expandedFormId,
      effect?.planId,
      effect?.fragmentId,
      effect?.hostId,
      effect?.bootId,
      effect?.activePlayId,
      effect?.presentationId,
      effect?.clueId,
      effect?.hostOperationContractId,
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
    const requiresRemoteIdentity = effect?.kind === DISTRIBUTED_PRESENTATION_EFFECT;
    const signal = decodeSignal(effect?.value);
    if (identityFields.some((value) => !boundedIdentity(value)) ||
        (effect?.kind !== LOCAL_PRESENTATION_EFFECT && !requiresRemoteIdentity) ||
        requiresRemoteIdentity !== hasRemoteIdentity ||
        (requiresRemoteIdentity &&
          remoteIdentityFields.some((value) => !boundedIdentity(value))) ||
        effect?.presentationKind !== SIGNAL_PRESENTATION_KIND ||
        effect?.hostId !== this.hostId ||
        effect?.bootId !== this.bootId ||
        !Number.isSafeInteger(effect?.requestNode) ||
        !Number.isSafeInteger(effect?.requestId) ||
        !Number.isSafeInteger(effect?.operationId) ||
        !signal) {
      return failure(BrowserDomFailure.InvalidPresentation, "malformed-effect");
    }
    if (this.#presentationIds.has(effect.presentationId)) {
      return failure(BrowserDomFailure.DuplicatePresentation, effect.presentationId);
    }
    if (this.#receipts.length === this.maximumReceiptItems ||
        this.#receiptBytes + signal.encoded.length > this.maximumReceiptBytes) {
      return failure(BrowserDomFailure.ReceiptCapacity, "receipt-store-full");
    }

    const receipt = Object.freeze({
      hostId: this.hostId,
      bootId: this.bootId,
      sourceDocumentId: effect.sourceDocumentId,
      checkedFormId: effect.checkedFormId,
      expandedFormId: effect.expandedFormId,
      planId: effect.planId,
      fragmentId: effect.fragmentId,
      activePlayId: effect.activePlayId,
      presentationId: effect.presentationId,
      clueId: effect.clueId,
      requestNode: effect.requestNode,
      requestId: effect.requestId,
      operationId: effect.operationId,
      hostOperationContractId: effect.hostOperationContractId,
      placementId: effect.placementId,
      sequence: signal.sequence.toString(),
      level: signal.level,
      encoded: signal.encoded,
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
    output.dataset.presentationId = receipt.presentationId;
    output.dataset.clueId = receipt.clueId;
    output.dataset.requestId = String(receipt.requestId);
    output.dataset.placementId = receipt.placementId;
    output.dataset.sequence = receipt.sequence;
    output.dataset.level = String(receipt.level);
    output.textContent =
      `receipt signal host=${receipt.hostId} boot=${receipt.bootId} ` +
      `placement=${receipt.placementId} sequence=${receipt.sequence} level=${receipt.level}`;
    this.root.append(output);

    this.#presentationIds.add(effect.presentationId);
    this.#receipts.push(receipt);
    this.#receiptBytes += signal.encoded.length;
    return Object.freeze({
      ok: true,
      completion: Object.freeze({
        sourceDocumentId: effect.sourceDocumentId,
        checkedFormId: effect.checkedFormId,
        expandedFormId: effect.expandedFormId,
        planId: effect.planId,
        fragmentId: effect.fragmentId,
        hostId: effect.hostId,
        bootId: effect.bootId,
        activePlayId: effect.activePlayId,
        requestNode: effect.requestNode,
        requestId: effect.requestId,
        operationId: effect.operationId,
        hostOperationContractId: effect.hostOperationContractId,
        presentationId: effect.presentationId,
        clueId: effect.clueId,
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
          valueKind: SIGNAL_VALUE_KIND,
          encoded: signal.encoded,
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
