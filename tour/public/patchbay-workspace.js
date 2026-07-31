const DEFAULT_WINDOW = Object.freeze({
  mode: "floating",
  shaded: false,
  hidden: false,
  x: 24,
  y: 88,
  width: 520,
  height: 470,
});

const MINIMUM_WIDTH = 320;
const MINIMUM_HEIGHT = 220;
const TITLE_BAR_HEIGHT = 52;
const VIEWPORT_MARGIN = 12;

function boundedNumber(value, fallback, minimum, maximum) {
  return Number.isFinite(value)
    ? Math.min(maximum, Math.max(minimum, value))
    : fallback;
}

function button(id, label, title) {
  const control = document.createElement("button");
  control.id = id;
  control.type = "button";
  control.className = "btn small workspace-control";
  control.textContent = label;
  control.title = title;
  control.setAttribute("aria-label", title);
  return control;
}

export class PatchbayWorkspaceController {
  constructor({
    canvasCard,
    sourceCard,
    actionBar,
    consoleCard,
    source,
    renderer,
    getContext,
  }) {
    this.canvasCard = canvasCard;
    this.sourceCard = sourceCard;
    this.actionBar = actionBar;
    this.consoleCard = consoleCard;
    this.source = source;
    this.renderer = renderer;
    this.getContext = getContext;
    this.active = false;
    this.nativeFullscreen = false;
    this.documentId = "default";
    this.state = { ...DEFAULT_WINDOW };
    this.savedFocus = null;
    this.transitionViewport = null;
    this.drag = null;
    this.resize = null;
    this.originalSourceParent = sourceCard.parentNode;
    this.originalSourceNext = sourceCard.nextSibling;
    this.originalActionParent = actionBar.parentNode;
    this.originalActionNext = actionBar.nextSibling;
    this.originalConsoleParent = consoleCard?.parentNode || null;
    this.originalConsoleNext = consoleCard?.nextSibling || null;
    this.diagnosticsOpen = false;
    this.storagePrefix = "conduit-patchbay-workspace/0/";
  }

  init() {
    this.buildToolbar();
    this.enhanceEditorWindow();
    this.fullscreenButton.onclick = () => void this.enter();
    this.exitButton.onclick = () => void this.exit();
    this.showEditorButton.onclick = () => this.showEditor();
    this.errorCount.onclick = () => this.toggleDiagnostics();
    this.shadeButton.onclick = () => this.toggleShade();
    this.dockButton.onclick = () => this.toggleDock();
    this.hideButton.onclick = () => this.hideEditor();
    this.resizeHandle.addEventListener("pointerdown", (event) =>
      this.startResize(event)
    );
    this.resizeHandle.addEventListener("keydown", (event) =>
      this.keyboardResize(event)
    );
    this.editorHeader.addEventListener("pointerdown", (event) =>
      this.startDrag(event)
    );
    document.addEventListener("fullscreenchange", () =>
      this.handleFullscreenChange()
    );
    document.addEventListener("keydown", (event) =>
      this.handleShortcut(event)
    );
    const recoverAfterViewportChange = () => {
      if (this.notifyingResize) return;
      requestAnimationFrame(() => this.recoverBounds());
    };
    window.addEventListener("resize", recoverAfterViewportChange);
    window.addEventListener("orientationchange", recoverAfterViewportChange);
    window.visualViewport?.addEventListener(
      "resize",
      recoverAfterViewportChange,
    );
    this.workspaceResizeObserver = new ResizeObserver(() => {
      if (this.active) requestAnimationFrame(() => this.recoverBounds());
    });
    this.workspaceResizeObserver.observe(this.canvasCard);
    this.source.addEventListener("input", () => this.updateStatus());
    this.setDocument("default");
    this.updateStatus();
  }

  buildToolbar() {
    this.canvasCard.id ||= "patchbay-workspace";
    this.toolbar = document.createElement("div");
    this.toolbar.className = "patchbay-workspace-toolbar";
    this.toolbar.setAttribute("role", "toolbar");
    this.toolbar.setAttribute("aria-label", "Topology workspace controls");
    this.fullscreenButton = button(
      "workspace-fullscreen",
      "⛶ Fullscreen canvas",
      "Enter fullscreen Patchbay workspace",
    );
    this.fullscreenButton.setAttribute("aria-keyshortcuts", "Control+Shift+F");
    this.fullscreenButton.setAttribute("aria-pressed", "false");
    this.exitButton = button(
      "workspace-exit",
      "⤢ Exit fullscreen",
      "Exit fullscreen Patchbay workspace",
    );
    this.exitButton.hidden = true;
    this.showEditorButton = button(
      "workspace-show-editor",
      "▣ Show source",
      "Show the live source editor",
    );
    this.showEditorButton.setAttribute("aria-keyshortcuts", "Alt+Shift+E");
    this.showEditorButton.hidden = true;
    this.errorCount = button(
      "workspace-error-count",
      "0 errors",
      "Show source diagnostics",
    );
    this.errorCount.classList.add("workspace-error-count");
    this.errorCount.setAttribute("aria-expanded", "false");
    this.errorCount.setAttribute("aria-controls", "diagnostic-console");
    this.errorCount.setAttribute("aria-live", "polite");
    this.workspaceStatus = document.createElement("span");
    this.workspaceStatus.id = "workspace-mode-status";
    this.workspaceStatus.className = "workspace-mode-status";
    this.workspaceStatus.setAttribute("aria-live", "polite");
    this.toolbar.append(
      this.fullscreenButton,
      this.exitButton,
      this.showEditorButton,
      this.errorCount,
      this.workspaceStatus,
    );
    this.canvasCard.querySelector(".card-header")?.after(this.toolbar);
  }

  enhanceEditorWindow() {
    this.sourceCard.id = "patchbay-source-window";
    this.sourceCard.classList.add("patchbay-source-window");
    this.sourceCard.setAttribute("role", "region");
    this.sourceCard.setAttribute("aria-label", "Live Panel source editor");
    this.editorHeader = this.sourceCard.querySelector(".card-header");
    this.editorHeader.classList.add("patchbay-source-titlebar");
    this.editorHeader.tabIndex = 0;
    this.editorHeader.setAttribute("aria-label", "Source editor window title bar");
    this.editorStatus = document.createElement("span");
    this.editorStatus.id = "patchbay-editor-status";
    this.editorStatus.className = "patchbay-editor-status";
    this.editorStatus.setAttribute("aria-live", "polite");
    this.editorControls = document.createElement("div");
    this.editorControls.className = "patchbay-editor-window-controls";
    this.shadeButton = button(
      "workspace-shade-editor",
      "▴ Shade",
      "Shade the source editor to its title bar",
    );
    this.shadeButton.setAttribute("aria-expanded", "true");
    this.dockButton = button(
      "workspace-dock-editor",
      "⇥ Dock",
      "Dock the source editor to the workspace edge",
    );
    this.dockButton.setAttribute("aria-pressed", "false");
    this.hideButton = button(
      "workspace-hide-editor",
      "× Hide",
      "Temporarily hide the source editor",
    );
    this.editorControls.append(
      this.shadeButton,
      this.dockButton,
      this.hideButton,
    );
    const existingBadge = this.editorHeader.querySelector(".lang-badge");
    existingBadge?.remove();
    this.editorHeader.append(this.editorStatus, this.editorControls);
    this.resizeHandle = document.createElement("button");
    this.resizeHandle.type = "button";
    this.resizeHandle.className = "patchbay-editor-resize-handle";
    this.resizeHandle.textContent = "↘";
    this.resizeHandle.title = "Resize source editor";
    this.resizeHandle.setAttribute("aria-label", "Resize source editor");
    this.resizeHandle.setAttribute(
      "aria-keyshortcuts",
      "ArrowUp ArrowDown ArrowLeft ArrowRight",
    );
    this.sourceCard.append(this.resizeHandle);
  }

  setDocument(documentId) {
    if (!documentId) return;
    this.documentId = documentId;
    this.state = this.loadState();
    if (this.active) {
      this.applyWindowState();
      this.recoverBounds();
    }
    this.updateStatus();
  }

  storageKey() {
    return `${this.storagePrefix}${this.documentId}`;
  }

  loadState() {
    let stored;
    try {
      stored = JSON.parse(sessionStorage.getItem(this.storageKey()) || "null");
    } catch {
      sessionStorage.removeItem(this.storageKey());
    }
    if (!stored || typeof stored !== "object" || Array.isArray(stored)) {
      return { ...DEFAULT_WINDOW };
    }
    return {
      mode: stored.mode === "docked" ? "docked" : "floating",
      shaded: stored.shaded === true,
      hidden: stored.hidden === true,
      x: boundedNumber(stored.x, DEFAULT_WINDOW.x, 0, 100_000),
      y: boundedNumber(stored.y, DEFAULT_WINDOW.y, 0, 100_000),
      width: boundedNumber(stored.width, DEFAULT_WINDOW.width, MINIMUM_WIDTH, 100_000),
      height: boundedNumber(stored.height, DEFAULT_WINDOW.height, MINIMUM_HEIGHT, 100_000),
    };
  }

  persistState() {
    sessionStorage.setItem(this.storageKey(), JSON.stringify(this.state));
  }

  context() {
    return this.getContext?.() || {};
  }

  updateStatus() {
    const context = this.context();
    const diagnostics = Math.max(0, Number(context.diagnostics) || 0);
    const dirty = context.dirty === true;
    const identity = context.identity
      ? String(context.identity).replace(/^sha256:/, "").slice(0, 8)
      : "unresolved";
    const revision = Number.isInteger(context.revision) ? context.revision : 0;
    const name = context.documentName || this.documentId;
    this.editorStatus.textContent =
      `${name} · r${revision} · ${identity} · ` +
      `${dirty ? "edited" : "clean"} · ${diagnostics} diagnostic` +
      `${diagnostics === 1 ? "" : "s"}`;
    this.editorHeader.dataset.dirty = String(dirty);
    this.editorHeader.dataset.diagnostics = String(diagnostics);
    this.errorCount.textContent =
      `${diagnostics} error${diagnostics === 1 ? "" : "s"}`;
    this.errorCount.dataset.count = String(diagnostics);
  }

  captureViewport() {
    this.transitionViewport = this.renderer?.getViewport?.() || null;
  }

  restoreViewport() {
    const viewport = this.transitionViewport;
    requestAnimationFrame(() => {
      this.notifyingResize = true;
      this.renderer?.notifyResize?.();
      this.notifyingResize = false;
      if (viewport) this.renderer?.setViewport?.(viewport);
    });
  }

  async enter() {
    if (this.active) return;
    this.savedFocus = document.activeElement;
    this.captureViewport();
    if (typeof this.canvasCard.requestFullscreen === "function") {
      try {
        await this.canvasCard.requestFullscreen();
        this.activate(true);
        return;
      } catch {
        this.activate(false);
        return;
      }
    }
    this.activate(false);
  }

  activate(nativeFullscreen) {
    if (this.active) return;
    this.active = true;
    this.nativeFullscreen = nativeFullscreen;
    this.canvasCard.classList.add("patchbay-workspace-active");
    this.canvasCard.classList.toggle(
      "patchbay-workspace-fallback",
      !nativeFullscreen,
    );
    document.body.classList.toggle(
      "patchbay-workspace-fallback-active",
      !nativeFullscreen,
    );
    this.fullscreenButton.hidden = true;
    this.fullscreenButton.setAttribute("aria-pressed", "true");
    this.exitButton.hidden = false;
    this.workspaceStatus.textContent = nativeFullscreen
      ? "Browser fullscreen"
      : "In-page fullscreen fallback";
    this.toolbar.after(this.actionBar);
    if (this.consoleCard) {
      this.canvasCard.append(this.consoleCard);
      this.consoleCard.classList.add("patchbay-workspace-console");
      this.consoleCard.hidden = !this.diagnosticsOpen;
    }
    this.canvasCard.append(this.sourceCard);
    this.sourceCard.setAttribute("role", "dialog");
    this.applyWindowState();
    this.recoverBounds();
    this.restoreViewport();
    this.exitButton.focus({ preventScroll: true });
    this.canvasCard.dispatchEvent(new CustomEvent("patchbayworkspacechange", {
      detail: { active: true, nativeFullscreen },
    }));
  }

  async exit() {
    if (!this.active) return;
    this.captureViewport();
    if (this.nativeFullscreen && document.fullscreenElement) {
      try {
        await document.exitFullscreen();
      } catch {
        this.deactivate();
      }
      return;
    }
    this.deactivate();
  }

  handleFullscreenChange() {
    if (document.fullscreenElement === this.canvasCard) {
      this.activate(true);
    } else if (this.active && this.nativeFullscreen) {
      this.deactivate();
    }
  }

  deactivate() {
    if (!this.active) return;
    this.active = false;
    this.nativeFullscreen = false;
    this.canvasCard.classList.remove(
      "patchbay-workspace-active",
      "patchbay-workspace-fallback",
    );
    document.body.classList.remove("patchbay-workspace-fallback-active");
    this.fullscreenButton.hidden = false;
    this.fullscreenButton.setAttribute("aria-pressed", "false");
    this.exitButton.hidden = true;
    this.showEditorButton.hidden = true;
    this.workspaceStatus.textContent = "";
    this.diagnosticsOpen = false;
    this.errorCount.setAttribute("aria-expanded", "false");
    this.sourceCard.classList.remove(
      "workspace-floating",
      "workspace-docked",
      "workspace-shaded",
      "workspace-hidden",
      "workspace-dragging",
      "workspace-resizing",
    );
    this.sourceCard.style.removeProperty("--workspace-window-x");
    this.sourceCard.style.removeProperty("--workspace-window-y");
    this.sourceCard.style.removeProperty("--workspace-window-width");
    this.sourceCard.style.removeProperty("--workspace-window-height");
    this.restoreNode(
      this.sourceCard,
      this.originalSourceParent,
      this.originalSourceNext,
    );
    this.restoreNode(
      this.actionBar,
      this.originalActionParent,
      this.originalActionNext,
    );
    if (this.consoleCard && this.originalConsoleParent) {
      this.consoleCard.hidden = false;
      this.consoleCard.classList.remove("patchbay-workspace-console");
      this.restoreNode(
        this.consoleCard,
        this.originalConsoleParent,
        this.originalConsoleNext,
      );
    }
    this.sourceCard.setAttribute("role", "region");
    this.restoreViewport();
    const focusTarget = this.savedFocus?.isConnected
      ? this.savedFocus
      : this.fullscreenButton;
    focusTarget?.focus?.({ preventScroll: true });
    this.canvasCard.dispatchEvent(new CustomEvent("patchbayworkspacechange", {
      detail: { active: false, nativeFullscreen: false },
    }));
  }

  restoreNode(node, parent, nextSibling) {
    if (nextSibling?.parentNode === parent) {
      parent.insertBefore(node, nextSibling);
    } else {
      parent.append(node);
    }
  }

  applyWindowState() {
    if (!this.active) return;
    this.sourceCard.classList.toggle(
      "workspace-floating",
      this.state.mode === "floating",
    );
    this.sourceCard.classList.toggle(
      "workspace-docked",
      this.state.mode === "docked",
    );
    this.sourceCard.classList.toggle("workspace-shaded", this.state.shaded);
    this.sourceCard.classList.toggle("workspace-hidden", this.state.hidden);
    this.sourceCard.setAttribute("aria-hidden", String(this.state.hidden));
    this.sourceCard.style.setProperty("--workspace-window-x", `${this.state.x}px`);
    this.sourceCard.style.setProperty("--workspace-window-y", `${this.state.y}px`);
    this.sourceCard.style.setProperty(
      "--workspace-window-width",
      `${this.state.width}px`,
    );
    this.sourceCard.style.setProperty(
      "--workspace-window-height",
      `${this.state.height}px`,
    );
    this.shadeButton.textContent = this.state.shaded ? "▾ Restore" : "▴ Shade";
    this.shadeButton.title = this.state.shaded
      ? "Restore the source editor"
      : "Shade the source editor to its title bar";
    this.shadeButton.setAttribute("aria-expanded", String(!this.state.shaded));
    this.dockButton.textContent =
      this.state.mode === "docked" ? "↗ Float" : "⇥ Dock";
    this.dockButton.title = this.state.mode === "docked"
      ? "Float the source editor"
      : "Dock the source editor to the workspace edge";
    this.dockButton.setAttribute(
      "aria-pressed",
      String(this.state.mode === "docked"),
    );
    this.showEditorButton.hidden = !this.state.hidden;
    this.resizeHandle.hidden =
      this.state.shaded || this.state.hidden || this.state.mode === "docked";
    this.updateStatus();
  }

  toggleShade() {
    if (!this.active) return;
    const wasShaded = this.state.shaded;
    this.state.shaded = !wasShaded;
    this.persistState();
    this.applyWindowState();
    if (this.state.shaded && this.sourceCard.contains(document.activeElement)) {
      this.shadeButton.focus({ preventScroll: true });
    } else if (wasShaded) {
      this.source.focus({ preventScroll: true });
    }
  }

  toggleDock() {
    if (!this.active) return;
    this.state.mode = this.state.mode === "docked" ? "floating" : "docked";
    this.persistState();
    this.applyWindowState();
    this.recoverBounds();
  }

  hideEditor() {
    if (!this.active) return;
    this.state.hidden = true;
    this.persistState();
    this.applyWindowState();
    this.showEditorButton.focus({ preventScroll: true });
  }

  showEditor() {
    if (!this.active) return;
    this.state.hidden = false;
    this.persistState();
    this.applyWindowState();
    this.shadeButton.focus({ preventScroll: true });
  }

  toggleDiagnostics() {
    if (!this.active || !this.consoleCard) return;
    this.diagnosticsOpen = !this.diagnosticsOpen;
    this.consoleCard.hidden = !this.diagnosticsOpen;
    this.errorCount.setAttribute("aria-expanded", String(this.diagnosticsOpen));
    if (this.diagnosticsOpen) {
      this.consoleCard.querySelector(
        ".diagnostic-console-button, #result",
      )?.focus?.({ preventScroll: true });
    } else {
      this.errorCount.focus({ preventScroll: true });
    }
  }

  workspaceBounds() {
    return {
      width: this.canvasCard.clientWidth,
      height: this.canvasCard.clientHeight,
    };
  }

  recoverBounds() {
    if (!this.active) return;
    const bounds = this.workspaceBounds();
    const safeTop = Math.max(
      VIEWPORT_MARGIN,
      this.actionBar.offsetTop + this.actionBar.offsetHeight + 8,
    );
    const maximumWidth = Math.max(
      MINIMUM_WIDTH,
      bounds.width - VIEWPORT_MARGIN * 2,
    );
    const maximumHeight = Math.max(
      MINIMUM_HEIGHT,
      bounds.height - safeTop - VIEWPORT_MARGIN,
    );
    this.state.width = Math.min(this.state.width, maximumWidth);
    this.state.height = Math.min(this.state.height, maximumHeight);
    this.state.x = Math.max(
      VIEWPORT_MARGIN,
      Math.min(
        this.state.x,
        Math.max(VIEWPORT_MARGIN, bounds.width - this.state.width - VIEWPORT_MARGIN),
      ),
    );
    this.state.y = Math.max(
      safeTop,
      Math.min(
        this.state.y,
        Math.max(
          safeTop,
          bounds.height -
            (this.state.shaded ? TITLE_BAR_HEIGHT : this.state.height) -
            VIEWPORT_MARGIN,
        ),
      ),
    );
    this.sourceCard.style.setProperty(
      "--workspace-window-safe-top",
      `${safeTop}px`,
    );
    this.persistState();
    this.applyWindowState();
    this.restoreViewport();
  }

  startDrag(event) {
    if (!this.active || this.state.mode !== "floating" ||
        event.button !== 0 || event.target.closest("button")) {
      return;
    }
    event.preventDefault();
    this.editorHeader.setPointerCapture(event.pointerId);
    this.drag = {
      pointerId: event.pointerId,
      startX: event.clientX,
      startY: event.clientY,
      x: this.state.x,
      y: this.state.y,
    };
    this.sourceCard.classList.add("workspace-dragging");
    const move = (moveEvent) => this.moveDrag(moveEvent);
    const finish = (upEvent) => {
      if (upEvent.pointerId !== this.drag?.pointerId) return;
      this.editorHeader.removeEventListener("pointermove", move);
      this.editorHeader.removeEventListener("pointerup", finish);
      this.editorHeader.removeEventListener("pointercancel", finish);
      this.drag = null;
      this.sourceCard.classList.remove("workspace-dragging");
      this.recoverBounds();
    };
    this.editorHeader.addEventListener("pointermove", move);
    this.editorHeader.addEventListener("pointerup", finish);
    this.editorHeader.addEventListener("pointercancel", finish);
  }

  moveDrag(event) {
    if (!this.drag || event.pointerId !== this.drag.pointerId) return;
    this.state.x = this.drag.x + event.clientX - this.drag.startX;
    this.state.y = this.drag.y + event.clientY - this.drag.startY;
    this.sourceCard.style.setProperty("--workspace-window-x", `${this.state.x}px`);
    this.sourceCard.style.setProperty("--workspace-window-y", `${this.state.y}px`);
  }

  startResize(event) {
    if (!this.active || this.state.mode !== "floating" ||
        this.state.shaded || event.button !== 0) {
      return;
    }
    event.preventDefault();
    this.resizeHandle.setPointerCapture(event.pointerId);
    this.resize = {
      pointerId: event.pointerId,
      startX: event.clientX,
      startY: event.clientY,
      width: this.state.width,
      height: this.state.height,
    };
    this.sourceCard.classList.add("workspace-resizing");
    const move = (moveEvent) => this.moveResize(moveEvent);
    const finish = (upEvent) => {
      if (upEvent.pointerId !== this.resize?.pointerId) return;
      this.resizeHandle.removeEventListener("pointermove", move);
      this.resizeHandle.removeEventListener("pointerup", finish);
      this.resizeHandle.removeEventListener("pointercancel", finish);
      this.resize = null;
      this.sourceCard.classList.remove("workspace-resizing");
      this.recoverBounds();
    };
    this.resizeHandle.addEventListener("pointermove", move);
    this.resizeHandle.addEventListener("pointerup", finish);
    this.resizeHandle.addEventListener("pointercancel", finish);
  }

  moveResize(event) {
    if (!this.resize || event.pointerId !== this.resize.pointerId) return;
    this.state.width = Math.max(
      MINIMUM_WIDTH,
      this.resize.width + event.clientX - this.resize.startX,
    );
    this.state.height = Math.max(
      MINIMUM_HEIGHT,
      this.resize.height + event.clientY - this.resize.startY,
    );
    this.sourceCard.style.setProperty(
      "--workspace-window-width",
      `${this.state.width}px`,
    );
    this.sourceCard.style.setProperty(
      "--workspace-window-height",
      `${this.state.height}px`,
    );
  }

  keyboardResize(event) {
    if (!this.active || this.state.mode !== "floating") return;
    const directions = {
      ArrowLeft: [-1, 0],
      ArrowRight: [1, 0],
      ArrowUp: [0, -1],
      ArrowDown: [0, 1],
    };
    const direction = directions[event.key];
    if (!direction) return;
    event.preventDefault();
    const step = event.shiftKey ? 40 : 10;
    this.state.width = Math.max(
      MINIMUM_WIDTH,
      this.state.width + direction[0] * step,
    );
    this.state.height = Math.max(
      MINIMUM_HEIGHT,
      this.state.height + direction[1] * step,
    );
    this.recoverBounds();
  }

  handleShortcut(event) {
    if (event.repeat || event.isComposing) return;
    const fullscreen = event.ctrlKey && event.shiftKey &&
      !event.altKey && !event.metaKey && event.key.toLowerCase() === "f";
    const editor = event.altKey && event.shiftKey &&
      !event.ctrlKey && !event.metaKey && event.key.toLowerCase() === "e";
    if (fullscreen) {
      event.preventDefault();
      void (this.active ? this.exit() : this.enter());
    } else if (editor && this.active) {
      event.preventDefault();
      if (this.state.hidden) {
        this.showEditor();
      } else {
        this.toggleShade();
      }
    } else if (event.key === "Escape" && this.active && !this.nativeFullscreen) {
      event.preventDefault();
      void this.exit();
    }
  }
}
