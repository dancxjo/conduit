// Proof-only DOM observation: retain one manifestation, not execution authority.
export function observeClockManifestation(root) {
  const selector = '[data-face-kind="face/tick"]';
  let latest = null;
  const capture = (node) => {
    if (!(node instanceof Element)) return;
    const outputs = node.matches(selector) ? [node] : node.querySelectorAll(selector);
    for (const output of outputs) latest = Object.freeze({ ...output.dataset });
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
    get current() { record(observer.takeRecords()); return latest; },
    finish() { record(observer.takeRecords()); observer.disconnect(); return latest; },
  });
}
