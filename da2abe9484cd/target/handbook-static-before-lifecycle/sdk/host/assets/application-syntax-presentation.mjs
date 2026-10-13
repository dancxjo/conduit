const encoder = new TextEncoder();
const decoder = new TextDecoder("utf-8", { fatal: true });
const PROTOCOL = "conduit.syntax-highlight-projection@1";

export function attachConduitSyntaxEditor(textarea, runtime) {
  const document = textarea?.ownerDocument;
  if (!document || !(textarea instanceof document.defaultView.HTMLTextAreaElement)) throw new TypeError("Conduit syntax editor requires a textarea");
  const container = textarea.parentNode;
  if (!container) throw new TypeError("Conduit syntax editor requires an attached textarea");
  const editor = document.createElement("div");
  editor.className = "syntax-editor";
  const wrapper = document.createElement("div");
  wrapper.dataset.applicationSyntax = "conduit";
  wrapper.append(editor);
  const backdrop = document.createElement("pre");
  backdrop.className = "syntax-highlight";
  backdrop.setAttribute("aria-hidden", "true");
  const code = document.createElement("code");
  backdrop.append(code);
  container.insertBefore(wrapper, textarea);
  editor.append(backdrop, textarea);

  let destroyed = false;
  const render = () => {
    if (destroyed) throw new Error("Conduit syntax editor has been destroyed");
    renderSyntax(textarea.value, code, textarea, runtime);
  };
  const synchronizeScroll = () => {
    backdrop.scrollTop = textarea.scrollTop;
    backdrop.scrollLeft = textarea.scrollLeft;
  };
  textarea.addEventListener("input", render);
  textarea.addEventListener("scroll", synchronizeScroll, { passive: true });
  render();
  synchronizeScroll();
  return Object.freeze({ render, destroy() {
    if (destroyed) return;
    destroyed = true;
    textarea.removeEventListener("input", render);
    textarea.removeEventListener("scroll", synchronizeScroll);
    wrapper.replaceWith(textarea);
    delete textarea.dataset.syntaxDisposition;
  } });
}

export function createConduitSyntaxExample(source, runtime) {
  if (typeof source !== "string" || source.length === 0) throw new TypeError("Conduit syntax example requires source");
  const example = document.createElement("pre");
  example.className = "syntax-example";
  example.setAttribute("aria-label", "Read-only Conduit example");
  example.tabIndex = 0;
  const code = document.createElement("code");
  example.append(code);
  renderSyntax(source, code, example, runtime);
  return example;
}

function renderSyntax(source, target, owner, runtime) {
  if (source.length === 0) {
    target.replaceChildren();
    owner.dataset.syntaxDisposition = "empty";
    return;
  }
  const document = owner.ownerDocument;
  const bytes = encoder.encode(source);
  let projection;
  try { projection = projectConduitSyntax(source, runtime); }
  catch {
    renderPlain(source, target, owner, "refused");
    return;
  }
  const fragment = document.createDocumentFragment();
  for (const [start, end, kindIndex] of projection.spans) {
    const span = document.createElement("span");
    span.className = `syntax-${projection.kinds[kindIndex]}`;
    span.textContent = decoder.decode(bytes.subarray(start, end));
    fragment.append(span);
  }
  target.replaceChildren(fragment);
  owner.dataset.syntaxDisposition = "accepted";
}

// The canonical Rust highlighter accepts incomplete source. This adapter only
// checks its finite, lossless UTF-8 projection; it never tokenizes source.
export function projectConduitSyntax(source, runtime) {
  if (typeof source !== "string") throw new TypeError("Conduit syntax requires text");
  const bytes = encoder.encode(source);
  if (decoder.decode(bytes) !== source) throw new TypeError("Conduit syntax source is not valid Unicode");
  if (bytes.length > 8192 || bytes.length > runtime.conduit_syntax_input_capacity()) {
    throw new RangeError("Conduit syntax source exceeds its byte bound");
  }
  new Uint8Array(runtime.memory.buffer, runtime.conduit_syntax_input_ptr(), bytes.length).set(bytes);
  const status = runtime.conduit_syntax_project(bytes.length);
  const length = runtime.conduit_syntax_output_len();
  if (status !== 0 || !Number.isSafeInteger(length) || length < 1 || length > 128 * 1024) {
    throw new Error("Conduit native syntax projection refused");
  }
  const output = new Uint8Array(runtime.memory.buffer, runtime.conduit_syntax_output_ptr(), length);
  const projection = JSON.parse(decoder.decode(output));
  validateProjection(projection, bytes);
  return Object.freeze({ ...projection, kinds: Object.freeze(projection.kinds), spans: Object.freeze(projection.spans.map(Object.freeze)) });
}

function validateProjection(projection, sourceBytes) {
  if (projection?.protocol !== PROTOCOL || projection.source_bytes !== sourceBytes.length) {
    throw new TypeError("Tour syntax projection identity changed");
  }
  if (!Array.isArray(projection.kinds) || projection.kinds.length !== 10 || !Array.isArray(projection.spans)) {
    throw new TypeError("Tour syntax projection shape changed");
  }
  const kinds = ["whitespace", "comment", "keyword", "name", "identity", "string", "number", "literal", "operator", "delimiter"];
  if (projection.kinds.some((kind, index) => kind !== kinds[index]) || projection.spans.length > sourceBytes.length) {
    throw new TypeError("Conduit syntax projection vocabulary or bound changed");
  }
  let cursor = 0;
  for (const span of projection.spans) {
    if (!Array.isArray(span) || span.length !== 3) throw new TypeError("Tour syntax span shape changed");
    const [start, end, kindIndex] = span;
    if (!Number.isSafeInteger(start) || !Number.isSafeInteger(end) || start !== cursor || end <= start || end > sourceBytes.length) {
      throw new TypeError("Tour syntax span bounds changed");
    }
    if (!Number.isSafeInteger(kindIndex) || kindIndex < 0 || kindIndex >= projection.kinds.length) {
      throw new TypeError("Tour syntax kind changed");
    }
    decoder.decode(sourceBytes.subarray(start, end));
    cursor = end;
  }
  if (cursor !== sourceBytes.length) throw new TypeError("Tour syntax projection is not lossless");
}

function renderPlain(source, target, owner, disposition) {
  target.replaceChildren(owner.ownerDocument.createTextNode(source));
  owner.dataset.syntaxDisposition = disposition;
}
