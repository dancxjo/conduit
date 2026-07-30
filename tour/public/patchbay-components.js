import { attachPanelSourceHighlighting } from "./panel-highlighter.js";

/**
 * Conduit Patchbay — Reusable Web Component Profile (#90, #99, #91)
 *
 * Provides modular custom elements for Patchbay application surfaces:
 * - <patchbay-nav>: Navigation and lesson/reference panel selector
 * - <patchbay-canvas>: Interactive Cytoscape visual graph canvas
 * - <patchbay-editor>: Real .panel source editor
 * - <patchbay-connection-list>: Accessible keyboard-addressable connection list
 * - <patchbay-inspector>: Exact plan identity, topology, and evidence stream
 * - <patchbay-app>: Complete root application composite panel
 */

export class PatchbayNavElement extends HTMLElement {
  connectedCallback() {
    this.innerHTML = `
      <div class="nav-section">
        <h3 class="nav-heading">Tour Handbook</h3>
        <ol id="lessons" class="nav-list"></ol>
      </div>
      <div class="nav-section">
        <h3 class="nav-heading">Reference Panels</h3>
        <ol id="reference-panels" class="nav-list"></ol>
      </div>
    `;
  }
}

export class PatchbayCanvasElement extends HTMLElement {
  connectedCallback() {
    this.innerHTML = `
      <div class="card-header">
        <h3>Patchbay Visual Topology</h3>
        <div class="view-toggle" role="group" aria-label="Topology projection">
          <button id="logical-view" class="toggle-btn active" aria-pressed="true">Logical</button>
          <button id="expanded-view" class="toggle-btn" aria-pressed="false">Expanded</button>
        </div>
      </div>
      <p class="card-subtitle">Drag nodes to adjust presentation layout. Click nodes or cords to inspect typed contracts.</p>
      <div id="cy" class="cytoscape-container"></div>
      <div class="node-controls">
        <span id="selected-node-label" class="selected-label">No node selected</span>
        <div class="button-row">
          <button id="move-left" class="btn small" disabled>◀ Move Left</button>
          <button id="move-right" class="btn small" disabled>Move Right ▶</button>
        </div>
      </div>
    `;
  }
}

export class PatchbayEditorElement extends HTMLElement {
  connectedCallback() {
    this.innerHTML = `
      <div class="card-header">
        <h3>Source Editor</h3>
        <span class="badge lang-badge">.panel v1</span>
      </div>
      <label for="source" class="sr-only">Real .panel source</label>
      <div class="panel-source-editor">
        <pre class="panel-source-highlight" aria-hidden="true"></pre>
        <textarea id="source" rows="20" spellcheck="false" autocomplete="off"
          placeholder="Authoring .panel graph..."></textarea>
      </div>
    `;
    attachPanelSourceHighlighting(this.querySelector("#source"));
  }
}

export class PatchbayConnectionListElement extends HTMLElement {
  connectedCallback() {
    this.innerHTML = `
      <div class="card-header">
        <h3>Accessible Connection List</h3>
        <span class="badge">A11y View</span>
      </div>
      <ul id="panel-connection-list" class="connection-list" aria-label="Typed cord connections"></ul>
    `;
  }
}

export class PatchbayInspectorElement extends HTMLElement {
  connectedCallback() {
    this.innerHTML = `
      <div class="inspectors">
        <details class="card inspector-card" open>
          <summary>📜 Exact Plan Identity & Artifacts</summary>
          <pre id="plan"></pre>
        </details>
        <details id="topology-inspector" class="card inspector-card">
          <summary>🧩 Topology Resolution</summary>
          <pre id="topology"></pre>
        </details>
        <details class="card inspector-card" open>
          <summary>📡 Bounded Execution Evidence Stream</summary>
          <pre id="evidence">No run evidence yet.</pre>
        </details>
      </div>
    `;
  }
}

export class PatchbayAppElement extends HTMLElement {
  connectedCallback() {
    this.className = "patchbay-app-root";
  }
}

if (!customElements.get("patchbay-nav")) customElements.define("patchbay-nav", PatchbayNavElement);
if (!customElements.get("patchbay-canvas")) customElements.define("patchbay-canvas", PatchbayCanvasElement);
if (!customElements.get("patchbay-editor")) customElements.define("patchbay-editor", PatchbayEditorElement);
if (!customElements.get("patchbay-connection-list")) customElements.define("patchbay-connection-list", PatchbayConnectionListElement);
if (!customElements.get("patchbay-inspector")) customElements.define("patchbay-inspector", PatchbayInspectorElement);
if (!customElements.get("patchbay-app")) customElements.define("patchbay-app", PatchbayAppElement);
