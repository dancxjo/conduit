// Check the actual pointer path, including shadow roots. This is a bounded
// native-control guard, not a certificate for arbitrary CSS or visual effects.
export function ownerFacePointerReachable(control, { allowDeferred = false } = {}) {
  const parent = node => node.assignedSlot ?? node.parentElement ?? node.getRootNode().host;
  let target = control, depth = 0;
  for (let node = control; node; node = parent(node)) {
    if (++depth > 64) return false;
    if (node instanceof HTMLDetailsElement && !node.open && node !== target) {
      const summary = node.querySelector(':scope > summary');
      if (!summary?.contains(target)) {
        if (!allowDeferred || !summary) return false;
        target = summary;
      }
    }
  }
  const document = control.ownerDocument;
  const viewport = document.defaultView;
  const bounds = target.getBoundingClientRect();
  let left = Math.max(0, bounds.left), right = Math.min(viewport.innerWidth, bounds.right);
  let top = Math.max(0, bounds.top), bottom = Math.min(viewport.innerHeight, bounds.bottom);
  let scrollable = document.scrollingElement.scrollHeight > viewport.innerHeight
    || document.scrollingElement.scrollWidth > viewport.innerWidth;
  let positioned = false;
  depth = 0;
  for (let node = target; node; node = parent(node)) {
    if (++depth > 64) return false;
    const style = getComputedStyle(node);
    positioned ||= ['absolute', 'fixed'].includes(style.position) || style.transform !== 'none';
    const clipsX = ['auto', 'scroll'].includes(style.overflowX);
    const clipsY = ['auto', 'scroll'].includes(style.overflowY);
    if (clipsX || clipsY) {
      const clip = node.getBoundingClientRect();
      if (clipsX) { left = Math.max(left, clip.left); right = Math.min(right, clip.right); }
      if (clipsY) { top = Math.max(top, clip.top); bottom = Math.min(bottom, clip.bottom); }
      scrollable ||= node.scrollHeight > node.clientHeight || node.scrollWidth > node.clientWidth;
    }
  }
  // Preparation/focus may retain ordinary scroll and native disclosure paths.
  // Activation itself must have a currently visible, unobstructed hit target.
  if (right <= left || bottom <= top) return allowDeferred && scrollable && !positioned;
  const x = (left + right) / 2, y = (top + bottom) / 2;
  let hit = document.elementFromPoint(x, y);
  for (let nesting = 0; hit?.shadowRoot; nesting++) {
    if (nesting >= 16) return false;
    const next = hit.shadowRoot.elementFromPoint(x, y);
    if (!next || next === hit) break;
    hit = next;
  }
  return hit === target || target.contains(hit);
}

