const DEFAULT_MAXIMUM_RESPONSE_BYTES = 128 * 1024;
const encoder = new TextEncoder();
const decoder = new TextDecoder("utf-8", { fatal: true });

export class BrowserFaceError extends Error {
  constructor({ code, message, operation, status = null, cause }) {
    super(message, cause ? { cause } : undefined);
    this.name = "BrowserFaceError";
    this.code = code;
    this.operation = operation;
    this.status = status;
    Object.freeze(this);
  }
}

/** Bounded same-origin client for one HTTP-carried Body Face. */
export class BrowserFaceClient {
  #base;
  #fetch;
  #maximumResponseBytes;

  constructor({ base = document.baseURI, fetch: fetchImplementation = globalThis.fetch,
    maximumResponseBytes = DEFAULT_MAXIMUM_RESPONSE_BYTES } = {}) {
    const resolved = new URL("api/", base);
    const origin = new URL(base).origin;
    if (resolved.origin !== origin || typeof fetchImplementation !== "function") {
      throw new TypeError("BrowserFaceClient requires one same-origin HTTP Face and fetch implementation");
    }
    if (!Number.isSafeInteger(maximumResponseBytes) || maximumResponseBytes < 1) {
      throw new TypeError("BrowserFaceClient requires one finite positive response bound");
    }
    this.#base = resolved;
    this.#fetch = fetchImplementation;
    this.#maximumResponseBytes = maximumResponseBytes;
    Object.freeze(this);
  }

  snapshot() { return this.#request("snapshot"); }

  interact(interaction) {
    return this.#request("interaction", { method: "POST", body: interaction });
  }

  async #request(path, { method = "GET", body } = {}) {
    if (typeof path !== "string" || !/^[a-z][a-z0-9-]*$/.test(path)) {
      throw new TypeError("BrowserFaceClient endpoint is not a bounded SDK path");
    }
    const options = {
      method,
      cache: "no-store",
      credentials: "same-origin",
      redirect: "error",
    };
    if (body !== undefined) {
      const bytes = encoder.encode(JSON.stringify(body));
      if (bytes.byteLength > this.#maximumResponseBytes) {
        throw new RangeError("Browser Face request exceeds its finite SDK bound");
      }
      options.headers = { "content-type": "application/json" };
      options.body = decoder.decode(bytes);
    }
    const operation = `BrowserFace.${path}`;
    let response;
    try { response = await this.#fetch(new URL(path, this.#base), options); }
    catch (cause) {
      throw new BrowserFaceError({ code: "FaceUnavailable", message: `Browser Face ${path} is unavailable`, operation, cause });
    }
    if (!response.ok) {
      throw new BrowserFaceError({
        code: "FaceHttpRefusal",
        message: `Browser Face ${path} refused with HTTP ${response.status}`,
        operation,
        status: response.status,
      });
    }
    const bytes = new Uint8Array(await response.arrayBuffer());
    if (bytes.byteLength < 1 || bytes.byteLength > this.#maximumResponseBytes) {
      throw new BrowserFaceError({
        code: "FaceResponseBound",
        message: `Browser Face ${path} response exceeds its finite SDK bound`,
        operation,
      });
    }
    try { return JSON.parse(decoder.decode(bytes)); }
    catch (cause) {
      throw new BrowserFaceError({
        code: "FaceResponseMalformed",
        message: `Browser Face ${path} response is not bounded UTF-8 JSON`,
        operation,
        cause,
      });
    }
  }
}
