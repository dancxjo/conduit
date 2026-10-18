// DOM is a provisional editing surface. Every committed replacement returns
// through the admitted Host Call; only the next document acknowledges it.
export function openBrowserTextEditor(container) {
  const textarea = container.ownerDocument.createElement("textarea");
  textarea.dataset.conduitTextEditor = "";
  textarea.rows = 6;
  textarea.maxLength = 256;
  container.replaceChildren(textarea);
  let snapshot = null, pending = null, composing = false, compositionBase = null, closed = false, submitted = null, cancelledComposition = false;
  const encoder = new TextEncoder();
  function restore() { textarea.value = snapshot?.text ?? ""; }
  function commit(event) {
    if (composing || event?.isComposing || !snapshot) return;
    if (cancelledComposition) { restore(); return; }
    if (!pending) { restore(); return; }
    if (!textarea.value.isWellFormed()) { restore(); textarea.dataset.editDisposition = "invalid-unicode"; return; }
    if (textarea.value.length > 256 || encoder.encode(textarea.value).length > 256) {
      restore(); textarea.dataset.editDisposition = "capacity-refused"; return;
    }
    const base = compositionBase ?? snapshot;
    compositionBase = null;
    if (textarea.value === base.text) return;
    const before = Array.from(base.text), after = Array.from(textarea.value);
    let start = 0, end = before.length, tail = after.length;
    while (start < end && start < tail && before[start] === after[start]) start++;
    while (end > start && tail > start && before[end - 1] === after[tail - 1]) { end--; tail--; }
    const text = encoder.encode(after.slice(start, tail).join(""));
    if (text.length > 256) { restore(); textarea.dataset.editDisposition = "capacity-refused"; return; }
    const bytes = new Uint8Array(16 + text.length), view = new DataView(bytes.buffer);
    view.setBigUint64(0, BigInt(base.revision), true);
    view.setUint32(8, start, true); view.setUint32(12, end, true); bytes.set(text, 16);
    submitted = { text: textarea.value, selection: [textarea.selectionStart, textarea.selectionEnd, textarea.selectionDirection] };
    restore();
    textarea.readOnly = true;
    textarea.dataset.editDisposition = "awaiting-authority";
    const request = pending; pending = null; request.cleanup(); request.resolve(bytes);
  }
  function compositionStart() { cancelledComposition = false; composing = true; compositionBase = snapshot; }
  function compositionEnd() { composing = false; queueMicrotask(commit); }
  function blur() {
    if (composing) { cancelledComposition = true; composing = false; compositionBase = null; restore(); textarea.dataset.editDisposition = "composition-cancelled"; }
  }
  const beforeInput = event => { if (!event.isComposing && !/composition/i.test(event.inputType ?? "")) cancelledComposition = false; };
  textarea.addEventListener("beforeinput", beforeInput);
  textarea.addEventListener("input", commit);
  textarea.addEventListener("compositionstart", compositionStart);
  textarea.addEventListener("compositionend", compositionEnd);
  textarea.addEventListener("blur", blur);
  return Object.freeze({
    next(document, signal) {
      if (closed || pending || signal.aborted) return Promise.reject(new Error("editor request cancelled or overlapping"));
      if (typeof document.revision !== "string" || !/^(0|[1-9][0-9]{0,19})$/.test(document.revision) || BigInt(document.revision) > 18446744073709551615n || typeof document.text !== "string" || !document.text.isWellFormed() || document.text.length > 256 || encoder.encode(document.text).length > 256) return Promise.reject(new Error("invalid authoritative document"));
      const selection = [textarea.selectionStart, textarea.selectionEnd, textarea.selectionDirection];
      const same = textarea.value === document.text;
      textarea.value = document.text;
      if (textarea.value !== document.text) {
        restore(); textarea.readOnly = true;
        return Promise.reject(new Error("UnsupportedDocumentRepresentation"));
      }
      snapshot = Object.freeze({ revision: document.revision, text: document.text });
      if (submitted?.text === document.text) textarea.setSelectionRange(...submitted.selection);
      else if (same) textarea.setSelectionRange(...selection);
      submitted = null;
      textarea.setAttribute("aria-label", document.label ?? "Text editor");
      textarea.dataset.revision = String(document.revision);
      textarea.dataset.editDisposition = "authoritative";
      textarea.readOnly = false;
      return new Promise((resolve, reject) => {
        const abort = () => { pending = null; composing = false; compositionBase = null; restore(); textarea.readOnly = true; reject(new Error("editor request cancelled")); };
        signal.addEventListener("abort", abort, { once: true });
        pending = { resolve, reject, cleanup: () => signal.removeEventListener("abort", abort) };
      });
    },
    close() {
      if (closed) return; closed = true;
      if (pending) { pending.cleanup(); pending.reject(new Error("editor closed")); pending = null; }
      textarea.removeEventListener("beforeinput", beforeInput);
      textarea.removeEventListener("input", commit); textarea.removeEventListener("compositionstart", compositionStart);
      textarea.removeEventListener("compositionend", compositionEnd); textarea.removeEventListener("blur", blur);
      submitted = null; snapshot = null; compositionBase = null;
      textarea.remove();
    },
  });
}
