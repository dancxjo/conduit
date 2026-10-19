// Proof-only DOM observation: retain first and latest manifestations, not execution authority.
export function observeClockManifestation(root) {
  const selector = '[data-face-kind="face/tick"]';
  let first = null;
  let latest = null;
  const capture = (node) => {
    if (!(node instanceof Element)) return;
    const outputs = node.matches(selector) ? [node] : node.querySelectorAll(selector);
    for (const output of outputs) {
      latest = Object.freeze({ ...output.dataset, text: output.textContent });
      first ??= latest;
    }
  };
  const record = (mutations) => {
    for (const mutation of mutations) {
      if (mutation.type === "childList") {
        // A terminal clear may remove the last output in the same observer
        // batch. Its detached node still carries its actual rendered identity.
        for (const node of mutation.removedNodes) capture(node);
        for (const node of mutation.addedNodes) capture(node);
      } else capture(mutation.target);
    }
    capture(root);
  };
  const observer = new MutationObserver(record);
  observer.observe(root, { childList: true, subtree: true, attributes: true });
  return Object.freeze({
    get first() { record(observer.takeRecords()); return first; },
    get current() { record(observer.takeRecords()); return latest; },
    finish() { record(observer.takeRecords()); observer.disconnect(); return latest; },
  });
}
