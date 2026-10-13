// Read-only rendering of a separately checked instruction component. The caller
// must validate original Source frames and provenance before supplying a record.
// A historical reconstruction grants no Show acknowledgement or input authority.
export class InstructionComponentElement extends (globalThis.HTMLElement ?? class {}) {
  #reading;
  constructor() {
    super();
    this.#reading = document.createElement('p');
    this.#reading.setAttribute('part', 'instruction');
    this.#reading.setAttribute('role', 'note');
    this.#reading.setAttribute('aria-label', 'Input instruction from captured execution');
    this.attachShadow({ mode: 'open' }).append(this.#reading);
  }
  connectedCallback() { this.withdraw(); }
  showReconstruction(record) {
    this.withdraw();
    const bytes = new TextEncoder().encode(record?.wording ?? '');
    if (typeof record?.wording !== 'string' || !bytes.length || bytes.length > 256
      || /[\u0000-\u001f\u007f]/u.test(record.wording)
      || typeof record?.provenance?.source_show !== 'string'
      || !record.provenance.source_show
      || typeof record?.provenance?.action !== 'string' || !record.provenance.action) {
      throw new TypeError('Incomplete or unbounded instruction reconstruction');
    }
    this.#reading.textContent = record.wording;
    this.hidden = false;
  }
  withdraw() {
    this.#reading.textContent = '';
    this.hidden = true;
  }
  disconnectedCallback() { this.withdraw(); }
}
if (globalThis.customElements && !customElements.get('conduit-instruction-component')) {
  customElements.define('conduit-instruction-component', InstructionComponentElement);
}
