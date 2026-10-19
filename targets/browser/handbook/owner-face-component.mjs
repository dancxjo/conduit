import { OwnerFacePresentation } from "./owner-face-presentation.mjs";

let admittedStyle;
export function admitOwnerFaceStyle(styleText) {
  if (typeof styleText !== 'string' || !styleText || new TextEncoder().encode(styleText).length > 16384
    || /@import\b|url\s*\(/i.test(styleText) || (admittedStyle !== undefined && admittedStyle !== styleText)) {
    throw new Error('Owner Face component style admission refused');
  }
  admittedStyle = styleText;
  return OwnerFaceElement;
}

// Custom-element composition of the same trusted DOM presentation. The SDK
// retains the current Show and the checked return route to the source owner.
export class OwnerFaceElement extends (globalThis.HTMLElement ?? class {}) {
  #presentation;
  constructor() {
    super();
    const shadow = this.attachShadow({ mode: 'open' });
    const look = document.createElement(admittedStyle === undefined ? 'link' : 'style');
    if (admittedStyle === undefined) {
      look.rel = 'stylesheet';
      look.href = new URL('./owner-face-look.css', import.meta.url).href;
    } else look.textContent = admittedStyle;
    const surface = document.createElement('div');
    shadow.append(look, surface);
    this.#presentation = new OwnerFacePresentation(this, surface);
  }
  disconnectedCallback() { this.#presentation.retire(); }
  set disabled(value) { this.#presentation.disabled = value; }
  show(view, options) { return this.#presentation.show(view, options); }
}
if (globalThis.customElements && !customElements.get('conduit-owner-face')) {
  customElements.define('conduit-owner-face', OwnerFaceElement);
}
