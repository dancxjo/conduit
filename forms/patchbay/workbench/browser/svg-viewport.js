const MINIMUM_ZOOM = 1;
const MAXIMUM_ZOOM = 8;
const ZOOM_STEP = 1.25;

export class SvgViewportModel {
  constructor(viewBox) {
    if (!Array.isArray(viewBox) || viewBox.length !== 4
      || viewBox.some((value) => !Number.isFinite(value))
      || viewBox[2] <= 0 || viewBox[3] <= 0) {
      throw new TypeError("diagram viewBox is invalid");
    }
    this.original = Object.freeze([...viewBox]);
    this.current = [...viewBox];
    this.zoom = MINIMUM_ZOOM;
  }

  zoomAt(requestedZoom, horizontalRatio = 0.5, verticalRatio = 0.5) {
    const zoom = Math.min(MAXIMUM_ZOOM, Math.max(MINIMUM_ZOOM, requestedZoom));
    const [x, y, width, height] = this.current;
    const focusX = x + width * horizontalRatio;
    const focusY = y + height * verticalRatio;
    const nextWidth = this.original[2] / zoom;
    const nextHeight = this.original[3] / zoom;
    this.current = [
      focusX - nextWidth * horizontalRatio,
      focusY - nextHeight * verticalRatio,
      nextWidth,
      nextHeight,
    ];
    this.zoom = zoom;
  }

  panPixels(horizontalPixels, verticalPixels, viewportWidth, viewportHeight) {
    if (viewportWidth <= 0 || viewportHeight <= 0) return;
    this.current[0] -= horizontalPixels * this.current[2] / viewportWidth;
    this.current[1] -= verticalPixels * this.current[3] / viewportHeight;
  }

  reset() {
    this.current = [...this.original];
    this.zoom = MINIMUM_ZOOM;
  }

  serializedViewBox() {
    return this.current.join(" ");
  }
}

function control(label, text, action) {
  const button = document.createElement("button");
  button.type = "button";
  button.setAttribute("aria-label", label);
  button.textContent = text;
  button.addEventListener("click", action);
  return button;
}

function svgViewBox(svg) {
  const values = svg.getAttribute("viewBox")?.trim().split(/\s+/).map(Number);
  if (!values || values.length !== 4) throw new TypeError("diagram lacks a finite viewBox");
  return values;
}

export async function enhanceSvgMask(image) {
  const source = new URL(image.currentSrc || image.src, window.location.href);
  if (source.origin !== window.location.origin || !source.pathname.endsWith(".svg")) return;
  const response = await fetch(source, { credentials: "same-origin" });
  if (!response.ok) throw new Error(`diagram request failed with ${response.status}`);
  const documentValue = new DOMParser().parseFromString(await response.text(), "image/svg+xml");
  const svg = documentValue.documentElement;
  if (svg.localName !== "svg" || svg.querySelector("parsererror, script, foreignObject")) {
    throw new TypeError("diagram SVG is not admitted for inline presentation");
  }

  const model = new SvgViewportModel(svgViewBox(svg));
  svg.removeAttribute("width");
  svg.removeAttribute("height");
  svg.setAttribute("preserveAspectRatio", "xMidYMid meet");
  svg.setAttribute("aria-hidden", "true");
  svg.classList.add("handbook-diagram");

  const viewport = document.createElement("span");
  viewport.className = "diagram-viewport";
  const toolbar = document.createElement("span");
  toolbar.className = "diagram-toolbar";
  const stage = document.createElement("span");
  stage.className = "diagram-stage";
  stage.tabIndex = 0;
  stage.setAttribute("role", "img");
  stage.setAttribute("aria-label", image.alt || "Interactive Conduit Form diagram");
  const status = document.createElement("output");
  status.setAttribute("aria-live", "polite");

  const update = () => {
    svg.setAttribute("viewBox", model.serializedViewBox());
    status.value = `${Math.round(model.zoom * 100)}%`;
    status.textContent = status.value;
  };
  const zoom = (factor, horizontalRatio = 0.5, verticalRatio = 0.5) => {
    model.zoomAt(model.zoom * factor, horizontalRatio, verticalRatio);
    update();
  };
  toolbar.append(
    control("Zoom out", "−", () => zoom(1 / ZOOM_STEP)),
    control("Reset diagram view", "Fit", () => { model.reset(); update(); }),
    control("Zoom in", "+", () => zoom(ZOOM_STEP)),
    status,
  );
  const original = document.createElement("a");
  original.href = source.href;
  original.textContent = "Open SVG";
  original.setAttribute("aria-label", "Open the original SVG diagram");
  toolbar.append(original);

  let pointer = null;
  stage.addEventListener("pointerdown", (event) => {
    if (event.button !== 0 || pointer !== null) return;
    pointer = { id: event.pointerId, x: event.clientX, y: event.clientY };
    stage.setPointerCapture(event.pointerId);
    stage.classList.add("is-dragging");
  });
  stage.addEventListener("pointermove", (event) => {
    if (!pointer || pointer.id !== event.pointerId) return;
    model.panPixels(
      event.clientX - pointer.x,
      event.clientY - pointer.y,
      stage.clientWidth,
      stage.clientHeight,
    );
    pointer.x = event.clientX;
    pointer.y = event.clientY;
    update();
  });
  const releasePointer = (event) => {
    if (!pointer || pointer.id !== event.pointerId) return;
    pointer = null;
    stage.classList.remove("is-dragging");
  };
  stage.addEventListener("pointerup", releasePointer);
  stage.addEventListener("pointercancel", releasePointer);
  stage.addEventListener("wheel", (event) => {
    event.preventDefault();
    const bounds = stage.getBoundingClientRect();
    zoom(
      event.deltaY < 0 ? ZOOM_STEP : 1 / ZOOM_STEP,
      (event.clientX - bounds.left) / bounds.width,
      (event.clientY - bounds.top) / bounds.height,
    );
  }, { passive: false });
  stage.addEventListener("keydown", (event) => {
    const pan = 48;
    if (event.key === "+" || event.key === "=") zoom(ZOOM_STEP);
    else if (event.key === "-") zoom(1 / ZOOM_STEP);
    else if (event.key === "0" || event.key === "Home") { model.reset(); update(); }
    else if (event.key === "ArrowLeft") model.panPixels(pan, 0, stage.clientWidth, stage.clientHeight);
    else if (event.key === "ArrowRight") model.panPixels(-pan, 0, stage.clientWidth, stage.clientHeight);
    else if (event.key === "ArrowUp") model.panPixels(0, pan, stage.clientWidth, stage.clientHeight);
    else if (event.key === "ArrowDown") model.panPixels(0, -pan, stage.clientWidth, stage.clientHeight);
    else return;
    event.preventDefault();
    update();
  });

  const help = document.createElement("span");
  help.className = "diagram-help";
  help.textContent = "Drag to pan · wheel or +/− to zoom · arrows to move · 0 to fit";
  stage.append(svg);
  viewport.append(toolbar, stage, help);
  image.replaceWith(viewport);
  update();
}

export async function enhanceSvgMasks(root, selector = "img[data-patchbay-svg-mask]") {
  if (!root?.querySelectorAll) throw new TypeError("SVG Mask root must support selectors");
  return Promise.allSettled([...root.querySelectorAll(selector)].map(enhanceSvgMask));
}
