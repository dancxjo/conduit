const CONTRACT = "conduit.host/browser-effects@1";
const MAXIMUM_SLOTS = 4;
const MAXIMUM_ARTIFACT_BYTES = 81 * 1024 * 1024;
const MAXIMUM_FILENAME_BYTES = 192;
const MAXIMUM_MEDIA_TYPE_BYTES = 96;
const MAXIMUM_LOCATION_BYTES = 256;
const MAXIMUM_DEVICE_FILTERS = 16;
const MAXIMUM_RESOURCE_IDENTITY_BYTES = 256;
const MAXIMUM_ID_BYTES = 96;
const encoder = new TextEncoder();

export class BrowserHostOperationRefusal extends Error {
  constructor(code) {
    super(`browser Host operation refused: ${code}`);
    this.name = "BrowserHostOperationRefusal";
    this.code = code;
  }
}

const refuse = (code) => { throw new BrowserHostOperationRefusal(code); };
const byteLength = (value) => encoder.encode(value).byteLength;
const validId = (value) => typeof value === "string" && value.length > 0
  && byteLength(value) <= MAXIMUM_ID_BYTES && /^[A-Za-z0-9][A-Za-z0-9._:/@-]*$/.test(value);

function bytesOf(value) {
  if (value instanceof Uint8Array) return new Uint8Array(value);
  if (ArrayBuffer.isView(value)) {
    return new Uint8Array(value.buffer.slice(value.byteOffset, value.byteOffset + value.byteLength));
  }
  if (value instanceof ArrayBuffer) return new Uint8Array(value.slice(0));
  refuse("malformed-request");
}

function classify(error) {
  if (error instanceof BrowserHostOperationRefusal) return error.code;
  if (error?.name === "AbortError" || error?.name === "NotFoundError") return "cancelled";
  if (error?.name === "NotAllowedError") return "denied";
  if (error?.name === "SecurityError") return "policy-prerequisite-absent";
  if (error?.name === "NotSupportedError") return "unavailable-api";
  if (error?.name === "QuotaExceededError") return "resource-failure";
  return "adapter-failure";
}

function defaultArtifactAdapter() {
  return Object.freeze({
    async handoff({ bytes, filename, mediaType }) {
      if (typeof globalThis.showSaveFilePicker === "function") {
        const handle = await globalThis.showSaveFilePicker({ suggestedName: filename });
        const writable = await handle.createWritable();
        await writable.write(new Blob([bytes], { type: mediaType }));
        await writable.close();
        return Object.freeze({ disposition: "completed", visibleName: handle.name ?? filename });
      }
      if (!globalThis.document || !globalThis.URL?.createObjectURL) {
        throw new DOMException("artifact handoff is unavailable", "NotSupportedError");
      }
      const link = document.createElement("a");
      const url = URL.createObjectURL(new Blob([bytes], { type: mediaType }));
      link.href = url;
      link.download = filename;
      link.hidden = true;
      document.body.append(link);
      link.click();
      link.remove();
      URL.revokeObjectURL(url);
      return Object.freeze({ disposition: "handoff-offered", visibleName: filename });
    },
  });
}

function defaultLocationAdapter() {
  return Object.freeze({
    move({ mode, path }) {
      if (!globalThis.history || !globalThis.location) {
        throw new DOMException("history is unavailable", "NotSupportedError");
      }
      history[mode === "replace" ? "replaceState" : "pushState"](null, "", path);
      return Object.freeze({ disposition: "completed", path: location.pathname });
    },
  });
}

function defaultDeviceAdapters() {
  return Object.freeze({
    "browser/web-serial@1": async ({ filters }) => {
      if (!globalThis.navigator?.serial?.requestPort) {
        throw new DOMException("WebSerial is unavailable", "NotSupportedError");
      }
      return navigator.serial.requestPort({ filters });
    },
    "browser/web-usb@1": async ({ filters }) => {
      if (!globalThis.navigator?.usb?.requestDevice) {
        throw new DOMException("WebUSB is unavailable", "NotSupportedError");
      }
      return navigator.usb.requestDevice({ filters });
    },
  });
}

export function createBrowserHostOperations({
  hostId,
  bootId,
  applicationId,
  applicationGeneration,
  authorityGeneration,
  currentContext = () => ({ hostId, bootId, applicationId, applicationGeneration, authorityGeneration }),
  selectedImplementations = [],
  initializedImplementations = [],
  artifactAdapter = defaultArtifactAdapter(),
  locationAdapter = defaultLocationAdapter(),
  deviceAdapters = defaultDeviceAdapters(),
  secureContext = globalThis.isSecureContext === true,
} = {}) {
  if (!validId(hostId) || !validId(bootId) || !validId(applicationId)
    || !Number.isSafeInteger(applicationGeneration) || applicationGeneration < 1
    || !Number.isSafeInteger(authorityGeneration) || authorityGeneration < 1
    || !Array.isArray(selectedImplementations)
    || !Array.isArray(initializedImplementations) || selectedImplementations.length > 32
    || initializedImplementations.length > 32) refuse("malformed-context");
  const selected = new Set(selectedImplementations);
  const initialized = new Set(initializedImplementations);
  if (selected.size !== selectedImplementations.length || initialized.size !== initializedImplementations.length
    || [...selected, ...initialized].some((identity) => !validId(identity))) refuse("malformed-context");
  const slots = new Map();

  const contextIsCurrent = () => {
    const current = currentContext();
    return current?.hostId === hostId && current?.bootId === bootId
      && current?.applicationId === applicationId
      && current?.applicationGeneration === applicationGeneration
      && current?.authorityGeneration === authorityGeneration;
  };
  const hasUserActivation = (declared) => declared === true
    && (globalThis.navigator?.userActivation === undefined || navigator.userActivation.isActive === true);
  const admit = (request, kind) => {
    if (!request || request.contract !== CONTRACT || request.kind !== kind
      || !validId(request.operationId) || request.hostId !== hostId || request.bootId !== bootId
      || request.applicationId !== applicationId || request.applicationGeneration !== applicationGeneration
      || request.authorityGeneration !== authorityGeneration) refuse("malformed-request");
    if (!contextIsCurrent()) refuse("stale-request");
    if (slots.has(request.operationId)) refuse("duplicate-operation");
    if (slots.size === MAXIMUM_SLOTS) refuse("operation-pressure");
    const admitted = Object.freeze({ operationId: request.operationId, kind });
    slots.set(request.operationId, admitted);
    return admitted;
  };
  const outcome = (admitted, disposition, details = {}) => Object.freeze({
    ...details,
    contract: CONTRACT,
    operationId: admitted.operationId,
    kind: admitted.kind,
    hostId,
    bootId,
    applicationId,
    applicationGeneration,
    authorityGeneration,
    disposition,
  });
  const run = async (request, kind, effect) => {
    const admitted = admit(request, kind);
    try {
      const result = await effect();
      if (!contextIsCurrent()) return outcome(admitted, "stale-completion");
      if (!result || typeof result !== "object" || ![
        "completed", "handoff-offered", "no-matching-device",
      ].includes(result.disposition)) return outcome(admitted, "malformed-completion");
      return outcome(admitted, result.disposition, result);
    } catch (error) {
      return outcome(admitted, contextIsCurrent() ? classify(error) : "stale-completion");
    } finally {
      slots.delete(admitted.operationId);
    }
  };

  return Object.freeze({
    async handoffArtifact(request) {
      return run(request, "artifact-handoff", async () => {
        if (!hasUserActivation(request.userActivation)) refuse("user-activation-required");
        const bytes = bytesOf(request.bytes);
        if (!Number.isSafeInteger(request.maximumBytes) || request.maximumBytes < 1
          || request.maximumBytes > MAXIMUM_ARTIFACT_BYTES || bytes.byteLength > request.maximumBytes
          || !validId(request.artifactId) || typeof request.filename !== "string"
          || byteLength(request.filename) < 1 || byteLength(request.filename) > MAXIMUM_FILENAME_BYTES
          || request.filename.includes("/") || request.filename.includes("\\")
          || typeof request.mediaType !== "string" || byteLength(request.mediaType) < 1
          || byteLength(request.mediaType) > MAXIMUM_MEDIA_TYPE_BYTES) refuse("artifact-not-admitted");
        return artifactAdapter.handoff({
          artifactId: request.artifactId, bytes, maximumBytes: request.maximumBytes,
          filename: request.filename, mediaType: request.mediaType,
        });
      });
    },
    async moveLocation(request) {
      return run(request, "location", async () => {
        if (!Number.isInteger(request.presentationRevision) || request.presentationRevision < 1
          || !["push", "replace"].includes(request.mode) || typeof request.path !== "string"
          || byteLength(request.path) < 1 || byteLength(request.path) > MAXIMUM_LOCATION_BYTES) {
          refuse("location-not-admitted");
        }
        const parsed = new URL(request.path, "https://conduit.invalid/");
        if (parsed.origin !== "https://conduit.invalid" || !parsed.pathname.startsWith("/")
          || parsed.username || parsed.password || parsed.hash || parsed.search) refuse("location-not-admitted");
        return locationAdapter.move({ mode: request.mode, path: parsed.pathname });
      });
    },
    async chooseDevice(request) {
      return run(request, "device-choice", async () => {
        if (!validId(request.implementationId) || !selected.has(request.implementationId)) {
          refuse("implementation-not-selected");
        }
        if (!initialized.has(request.implementationId)) refuse("implementation-not-initialized");
        if (request.authorized !== true) refuse("authority-missing");
        if (!hasUserActivation(request.userActivation)) refuse("user-activation-required");
        if (!secureContext) refuse("secure-context-required");
        if (request.maximumResults !== 1 || !Number.isSafeInteger(request.maximumResourceIdentityBytes)
          || request.maximumResourceIdentityBytes < 1
          || request.maximumResourceIdentityBytes > MAXIMUM_RESOURCE_IDENTITY_BYTES
          || !Array.isArray(request.filters) || request.filters.length > MAXIMUM_DEVICE_FILTERS) {
          refuse("device-constraints-not-admitted");
        }
        const filters = request.filters.map((filter) => {
          if (!filter || typeof filter !== "object" || Array.isArray(filter)
            || Object.keys(filter).length > 8 || Object.values(filter).some((value) =>
              !Number.isInteger(value) || value < 0 || value > 65_535)) {
            refuse("device-constraints-not-admitted");
          }
          return Object.freeze({ ...filter });
        });
        const adapter = deviceAdapters[request.implementationId];
        if (typeof adapter !== "function") refuse("unavailable-api");
        const device = await adapter({ filters });
        if (!device) return Object.freeze({ disposition: "no-matching-device" });
        const resource = Object.freeze({
          handle: `browser-resource/${request.operationId}`,
          vendorId: device.vendorId ?? device.getInfo?.().usbVendorId ?? null,
          productId: device.productId ?? device.getInfo?.().usbProductId ?? null,
          serialNumber: device.serialNumber ?? null,
        });
        if (byteLength(JSON.stringify(resource)) > request.maximumResourceIdentityBytes) {
          refuse("resource-identity-pressure");
        }
        return Object.freeze({
          disposition: "completed",
          resource,
        });
      });
    },
    activeOperations() { return slots.size; },
  });
}

export const browserHostOperationLimits = Object.freeze({
  contract: CONTRACT,
  slots: MAXIMUM_SLOTS,
  artifactBytes: MAXIMUM_ARTIFACT_BYTES,
  filenameBytes: MAXIMUM_FILENAME_BYTES,
  mediaTypeBytes: MAXIMUM_MEDIA_TYPE_BYTES,
  locationBytes: MAXIMUM_LOCATION_BYTES,
  deviceFilters: MAXIMUM_DEVICE_FILTERS,
  resourceIdentityBytes: MAXIMUM_RESOURCE_IDENTITY_BYTES,
});
