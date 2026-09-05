const encoder = new TextEncoder();
const decoder = new TextDecoder("utf-8", { fatal: true });
const PROTOCOL = "conduit.syntax-highlight-projection@1";

export function attachConduitSyntaxEditor(textarea, runtime) {
  if (!(textarea instanceof HTMLTextAreaElement)) throw new TypeError("Conduit syntax editor requires a textarea");
  const container = textarea.closest('[data-application-component="form-field"]');
  if (!container) throw new TypeError("Conduit syntax editor requires a presentation form field");
  container.dataset.applicationSyntax = "conduit";
  const backdrop = document.createElement("pre");
  backdrop.className = "syntax-highlight";
  backdrop.setAttribute("aria-hidden", "true");
  const code = document.createElement("code");
  backdrop.append(code);
  container.insertBefore(backdrop, textarea);

  const render = () => renderSyntax(textarea.value, code, textarea, runtime);
  const synchronizeScroll = () => {
    backdrop.scrollTop = textarea.scrollTop;
    backdrop.scrollLeft = textarea.scrollLeft;
  };
  textarea.addEventListener("input", render);
  textarea.addEventListener("scroll", synchronizeScroll, { passive: true });
  render();
  synchronizeScroll();
  return Object.freeze({ render });
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
  const bytes = encoder.encode(source);
  if (bytes.length > runtime.conduit_syntax_input_capacity()) {
    renderPlain(source, target, owner, "refused");
    return;
  }
  new Uint8Array(runtime.memory.buffer, runtime.conduit_syntax_input_ptr(), bytes.length).set(bytes);
  const status = runtime.conduit_syntax_project(bytes.length);
  if (status < 0 || runtime.conduit_syntax_output_len() === 0) {
    renderPlain(source, target, owner, "refused");
    return;
  }
  let projection;
  try {
    const output = new Uint8Array(
      runtime.memory.buffer,
      runtime.conduit_syntax_output_ptr(),
      runtime.conduit_syntax_output_len(),
    );
    projection = JSON.parse(decoder.decode(output));
    validateProjection(projection, bytes);
  } catch {
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

function validateProjection(projection, sourceBytes) {
  if (projection?.protocol !== PROTOCOL || projection.source_bytes !== sourceBytes.length) {
    throw new TypeError("Tour syntax projection identity changed");
  }
  if (!Array.isArray(projection.kinds) || projection.kinds.length !== 10 || !Array.isArray(projection.spans)) {
    throw new TypeError("Tour syntax projection shape changed");
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
  target.replaceChildren(document.createTextNode(source));
  owner.dataset.syntaxDisposition = disposition;
}
