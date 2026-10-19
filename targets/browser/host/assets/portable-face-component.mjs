// One optional registered Web Component Back. The admitted Host supplies its
// checked renderer and return route; this module owns only the DOM surface.
export const PORTABLE_FACE_COMPONENT = Object.freeze({
  identity: 'conduit.browser/portable-face-component@1', version: 1,
  tag: 'conduit-portable-face', maximum_nodes: 2048, maximum_depth: 12,
  maximum_in_flight_events: 1,
});
let registered = null;
export function registerPortableFaceComponent({ render, styleText }) {
  if (typeof render !== 'function' || typeof styleText !== 'string' || styleText.length > 16384) throw new Error('Component preparation refused');
  const existing = customElements.get(PORTABLE_FACE_COMPONENT.tag);
  if (existing && existing !== registered) throw new Error('Untrusted custom element registration');
  if (registered) return registered;
  class PortableFaceElement extends HTMLElement {
    #root;
    #controller;
    #encounter;
    #disconnected;
    constructor() {
      super();
      const shadow = this.attachShadow({ mode: 'open' });
      const style = document.createElement('style'); style.textContent = styleText;
      this.#root = document.createElement('div'); shadow.append(style, this.#root);
    }
    present(encounter, callbacks) {
      const component = encounter.view.component, expression = component?.expression?.expression;
      // This built-in Back realizes the checked single-column profile. Refuse
      // another recipe instead of silently ignoring planned configuration.
      if (!component || component.fallback || component.maximum_columns !== 1
        || component.groups.some(group => group !== 0)
        || !Number.isInteger(component.gutter) || component.gutter < 0 || component.gutter > 64
        || !expression || !['compact', 'spacious'].includes(expression.density)
        || !['phosphor', 'editorial'].includes(expression.palette) || expression.motion !== 'none'
        || !Number.isInteger(expression.text_scale_percent) || expression.text_scale_percent < 100 || expression.text_scale_percent > 200) {
        throw new Error('Component visual profile unsupported');
      }
      this.dataset.componentExpression = expression.identity;
      this.dataset.componentDensity = expression.density; this.dataset.componentPalette = expression.palette;
      this.style.setProperty('--portable-text-scale', String(expression.text_scale_percent / 100));
      this.style.setProperty('--portable-gutter', `${component.gutter}px`);
      this.#controller?.retire();
      this.#encounter = encounter;
      this.#disconnected = callbacks.disconnected;
      this.#controller = render(this.#root, encounter, { ...callbacks, enhanced: true, inspectSurface: () => this.#inspectBounds() });
      return this.#controller;
    }
    activate(encounter) { this.#controller?.activate(encounter); }
    inspect() { return this.#controller?.inspect(); }
    #inspectBounds() {
      const nodes = this.shadowRoot.querySelectorAll('*');
      if (nodes.length > PORTABLE_FACE_COMPONENT.maximum_nodes) throw new Error('Component node capacity');
      let depth = 0;
      for (const node of nodes) {
        let current = node, value = 0;
        while (current && current !== this.shadowRoot) { value += 1; current = current.parentNode; }
        depth = Math.max(depth, value);
      }
      if (depth > PORTABLE_FACE_COMPONENT.maximum_depth) throw new Error('Component depth capacity');
      return { nodes: nodes.length, depth };
    }
    retire() { this.#controller?.retire(); }
    disconnectedCallback() {
      this.#controller?.retire();
      if (this.#encounter) this.#disconnected?.(this.#encounter.view.show_id);
      this.#encounter = null;
    }
  }
  Object.defineProperty(PortableFaceElement, 'componentIdentity', { value: PORTABLE_FACE_COMPONENT.identity });
  Object.freeze(PortableFaceElement.prototype);
  customElements.define(PORTABLE_FACE_COMPONENT.tag, PortableFaceElement);
  registered = PortableFaceElement;
  return registered;
}
