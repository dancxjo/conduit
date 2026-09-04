export function createTourNavigation(presentation, navigate) {
  let revision = 0;
  return Object.freeze({
    render(currentPage, pageCount, running) {
      presentation.present("book-navigation", {
        revision: ++revision,
        actions: [
          { id: "book.previous", event: "activate" },
          { id: "book.next", event: "activate" },
        ],
        nodes: [
          { parent: null, component: "navigation", key: "navigation", text: "Tour pages", action: null },
          { parent: 0, component: "status", key: "progress", text: `Page ${currentPage + 1} of ${pageCount}`, action: null },
          { parent: 0, component: "button", key: "previous", text: "Previous", action: currentPage > 0 ? 0 : null },
          { parent: 0, component: "button", key: "next", text: "Next", action: currentPage < pageCount - 1 ? 1 : null },
        ],
      }, {
        onEvent(event) {
          presentation.nextEvent("book-navigation");
          if (event.action === "book.previous") navigate(-1);
          else if (event.action === "book.next") navigate(1);
        },
      });
    },
  });
}

export function createTourWorkspace(root, readingState) {
  const content = root.querySelector(".tour-content");
  const laboratory = root.querySelector("#laboratory-slot");
  const width = root.querySelector("#tour-narrative-width");
  const patchbayHeight = root.querySelector("#tour-patchbay-height");
  const sourceWidth = root.querySelector("#tour-source-width");
  const reset = root.querySelector("[data-tour-reset-layout]");
  const viewButtons = [...root.querySelectorAll("[data-tour-view]")];
  if (!content || !laboratory || !width || !patchbayHeight || !sourceWidth || !reset || viewButtons.length !== 2) {
    throw new Error("Tour workspace controls are incomplete");
  }

  const setWidth = (value, persist) => {
    const admitted = Number(value);
    if (!Number.isInteger(admitted) || admitted < 30 || admitted > 65) {
      throw new Error("Tour narrative width is outside its admitted bound");
    }
    width.value = String(admitted);
    content.style.setProperty("--tour-narrative-percent", `${admitted}%`);
    if (persist) readingState.setNarrativePercent(admitted);
  };
  const setLaboratoryGeometry = (control, property, setter, value, persist) => {
    const admitted = Number(value);
    control.value = String(admitted);
    laboratory.style.setProperty(property, `${admitted}%`);
    if (persist) setter(admitted);
  };
  const show = (view, focus = false) => {
    if (view !== "lesson" && view !== "laboratory") throw new Error("Tour workspace view is not admitted");
    content.dataset.narrowView = view;
    for (const button of viewButtons) button.setAttribute("aria-pressed", String(button.dataset.tourView === view));
    if (focus) (view === "lesson" ? root.querySelector("#chapter") : laboratory).focus({ preventScroll: true });
  };

  setWidth(readingState.workspace.narrativePercent, false);
  setLaboratoryGeometry(patchbayHeight, "--tour-patchbay-percent", readingState.setPatchbayPercent, readingState.workspace.patchbayPercent, false);
  setLaboratoryGeometry(sourceWidth, "--tour-source-percent", readingState.setSourcePercent, readingState.workspace.sourcePercent, false);
  width.addEventListener("input", () => setWidth(width.value, true));
  patchbayHeight.addEventListener("input", () => setLaboratoryGeometry(patchbayHeight, "--tour-patchbay-percent", readingState.setPatchbayPercent, patchbayHeight.value, true));
  sourceWidth.addEventListener("input", () => setLaboratoryGeometry(sourceWidth, "--tour-source-percent", readingState.setSourcePercent, sourceWidth.value, true));
  reset.addEventListener("click", () => {
    setWidth(46, true);
    setLaboratoryGeometry(patchbayHeight, "--tour-patchbay-percent", readingState.setPatchbayPercent, 55, true);
    setLaboratoryGeometry(sourceWidth, "--tour-source-percent", readingState.setSourcePercent, 60, true);
  });
  for (const button of viewButtons) button.addEventListener("click", () => show(button.dataset.tourView, true));
  return Object.freeze({
    showLesson: (focus = false) => show("lesson", focus),
    showLaboratory: (focus = false) => show("laboratory", focus),
  });
}

export function createTourRunnerActions(presentation, slot, runLabel, onRun, onStop, onRestore) {
  let revision = 0;
  return Object.freeze({
    render(running) {
      presentation.present(slot, {
        revision: ++revision,
        actions: [
          { id: "book.run", event: "activate" },
          { id: "book.stop", event: "activate" },
          { id: "book.restore", event: "activate" },
        ],
        nodes: [
          { parent: null, component: "action-group", key: "runner-actions", text: "Play and draft actions", action: null },
          { parent: 0, component: "button", key: "run", text: runLabel, action: running ? null : 0 },
          { parent: 0, component: "button", key: "stop", text: "Stop", action: running ? 1 : null },
          { parent: 0, component: "button", key: "restore", text: "Restore canonical source", action: 2 },
        ],
      }, {
        onEvent(event) {
          presentation.nextEvent(slot);
          if (event.action === "book.run") onRun();
          else if (event.action === "book.stop") onStop();
          else if (event.action === "book.restore") onRestore();
        },
      });
    },
  });
}
