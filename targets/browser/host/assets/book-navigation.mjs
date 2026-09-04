export function createBookNavigation(presentation, navigate) {
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

export function createBookWorkspace(root, readingState) {
  const content = root.querySelector(".tour-content");
  const laboratory = root.querySelector("#laboratory-slot");
  const width = root.querySelector("#tour-narrative-width");
  const reset = root.querySelector("[data-tour-reset-layout]");
  const viewButtons = [...root.querySelectorAll("[data-tour-view]")];
  if (!content || !laboratory || !width || !reset || viewButtons.length !== 2) {
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
  const show = (view, focus = false) => {
    if (view !== "lesson" && view !== "laboratory") throw new Error("Tour workspace view is not admitted");
    content.dataset.narrowView = view;
    for (const button of viewButtons) button.setAttribute("aria-pressed", String(button.dataset.tourView === view));
    if (focus) (view === "lesson" ? root.querySelector("#chapter") : laboratory).focus({ preventScroll: true });
  };

  setWidth(readingState.workspace.narrativePercent, false);
  width.addEventListener("input", () => setWidth(width.value, true));
  reset.addEventListener("click", () => setWidth(46, true));
  for (const button of viewButtons) button.addEventListener("click", () => show(button.dataset.tourView, true));
  return Object.freeze({
    showLesson: (focus = false) => show("lesson", focus),
    showLaboratory: (focus = false) => show("laboratory", focus),
  });
}

export function createBookRunnerActions(presentation, slot, runLabel, onRun, onStop) {
  let revision = 0;
  return Object.freeze({
    render(running) {
      presentation.present(slot, {
        revision: ++revision,
        actions: [
          { id: "book.run", event: "activate" },
          { id: "book.stop", event: "activate" },
        ],
        nodes: [
          { parent: null, component: "action-group", key: "runner-actions", text: "Play actions", action: null },
          { parent: 0, component: "button", key: "run", text: runLabel, action: running ? null : 0 },
          { parent: 0, component: "button", key: "stop", text: "Stop", action: running ? 1 : null },
        ],
      }, {
        onEvent(event) {
          presentation.nextEvent(slot);
          if (event.action === "book.run") onRun();
          else if (event.action === "book.stop") onStop();
        },
      });
    },
  });
}
