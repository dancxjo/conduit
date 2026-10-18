// A conservative admission check for this generic native-control renderer.
// This does not certify arbitrary CSS or another renderer's accessibility tree.
const fail = action => {
  const error = new Error(`No reachable named browser control for Face action ${action.identity}.`);
  error.code = 'RendererActionInaccessible';
  throw error;
};
const composedParent = node => node.assignedSlot ?? node.parentElement ?? node.getRootNode().host;
function visiblePath(node) {
  let closedDisclosure = false;
  for (let ancestor = node; ancestor; ancestor = composedParent(ancestor)) {
    const style = getComputedStyle(ancestor);
    if (ancestor.hidden || ancestor.inert || ancestor.getAttribute('aria-hidden') === 'true'
      || style.display === 'none' || style.visibility !== 'visible' || Number(style.opacity) === 0
      || style.clipPath !== 'none' || style.clip !== 'auto') return false;
    if (ancestor instanceof HTMLDetailsElement && !ancestor.open && ancestor !== node
      && !ancestor.querySelector(':scope > summary')?.contains(node)) {
      const summary = ancestor.querySelector(':scope > summary');
      if (!summary?.textContent.trim() || !visiblePath(summary)) return false;
      closedDisclosure = true;
    }
    // A closed native disclosure is a truthful reachable keyboard path. It
    // hides descendants without making their own boxes currently meaningful.
    if (!closedDisclosure && !(ancestor instanceof HTMLDetailsElement)) {
      const rect = ancestor.getBoundingClientRect();
      const target = node.getBoundingClientRect();
      if (['hidden', 'clip'].includes(style.overflowX) || ['hidden', 'clip'].includes(style.overflowY)) {
        if (target.width && target.height && (target.left < rect.left || target.right > rect.right
          || target.top < rect.top || target.bottom > rect.bottom)) return false;
      }
      if (!rect.width || !rect.height) {
        const enclosing = ancestor.closest('details:not([open])');
        if (!enclosing) return false;
      }
    }
  }
  return true;
}
export function assertOwnerFaceReachability(element, view) {
  const certifiedControl = control => !control.hasAttribute('aria-label')
    && !control.hasAttribute('aria-labelledby') && control.getAttribute('aria-disabled') !== 'true'
    && (view.show_state === 'prepared' || !control.matches(':disabled'));
  for (const action of view.actions.filter(action => action.availability === 'available')) {
    const regions = [...element.shadowRoot.querySelectorAll('[data-owner-action]')]
      .filter(region => region.dataset.ownerAction === action.identity);
    const reachable = regions.some(region => {
      const controls = [...region.querySelectorAll('button,input,select')];
      const activator = controls.find(control => control.matches('button,input[type=radio]'));
      if (!activator || !certifiedControl(activator) || activator.dataset.unavailable === 'true' || activator.tabIndex < 0) return false;
      const named = activator.matches('button') ? activator.textContent.trim()
        : [...activator.labels].some(label => label.textContent.trim());
      const role = activator.getAttribute('role');
      if (role && role !== (activator.matches('button') ? 'button' : 'radio')) return false;
      if (!named || !visiblePath(activator)) return false;
      return controls.every(control => certifiedControl(control) && control.dataset.unavailable !== 'true'
        && control.tabIndex >= 0 && visiblePath(control)
        && (control.matches('button') || [...control.labels].some(label => label.textContent.trim())));
    });
    if (!reachable) fail(action);
  }
}
