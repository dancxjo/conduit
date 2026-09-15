export function createTourNavigation(runtime, presentation, navigate) {
  let revision = 0;
  return Object.freeze({
    render(currentPage, pageCount) {
      const code = runtime.conduit_tour_navigation_view(++revision, currentPage, pageCount);
      if (code < 0) throw new Error(`Tour navigation presentation was refused (${code})`);
      const pointer = runtime.conduit_tour_navigation_view_ptr();
      const length = runtime.conduit_tour_navigation_view_len();
      const encoded = new Uint8Array(runtime.memory.buffer, pointer, length).slice();
      presentation.present("tour-navigation", encoded, {
        onEvent(event) {
          presentation.nextEvent("tour-navigation");
          const action = event.action === "tour.previous" ? 1 : event.action === "tour.next" ? 2 : 0;
          if (action === 0 || runtime.conduit_tour_application_apply(action) < 0) {
            throw new Error("shared Tour application refused the navigation transition");
          }
          navigate(action === 1 ? -1 : 1);
        },
      });
    },
  });
}

export function presentTourWorkspaceSeparator(presentation) {
  presentation.present("tour-workspace-separator", {
    revision: 1,
    actions: [],
    nodes: [{
      parent: null,
      component: "separator",
      key: "lesson-laboratory-boundary",
      text: "Lesson and laboratory boundary",
      action: null,
    }],
  });
}

export function createTourWorkspace(root, readingState) {
  const content = root.querySelector(".tour-content");
  const laboratory = root.querySelector("#laboratory-slot");
  const width = root.querySelector("#tour-narrative-width");
  const reset = root.querySelector("[data-tour-reset-layout]");
  const viewButtons = [...root.querySelectorAll("[data-tour-view]")];
  if (!content || !laboratory || !width || !reset || viewButtons.length !== 2) {
    throw new Error("Tour workspace controls are incomplete");
  }

  const exposePercent = (control, value) => {
    control.setAttribute("aria-valuetext", `${value} percent`);
  };
  const setWidth = (value, persist) => {
    const admitted = Number(value);
    if (!Number.isInteger(admitted) || admitted < 30 || admitted > 65) {
      throw new Error("Tour narrative width is outside its admitted bound");
    }
    width.value = String(admitted);
    exposePercent(width, admitted);
    content.style.setProperty("--tour-narrative-percent", `${admitted}%`);
    if (persist) readingState.setNarrativePercent(admitted);
  };
  const setLaboratoryGeometry = (selector, property, setter, value, persist) => {
    const admitted = Number(value);
    for (const control of root.querySelectorAll(selector)) {
      control.value = String(admitted);
      exposePercent(control, admitted);
    }
    laboratory.style.setProperty(property, `${admitted}%`);
    if (persist) setter(admitted);
  };
  const dragDivider = (control, event, measure) => {
    if (event.button !== 0) return;
    event.preventDefault();
    control.setPointerCapture(event.pointerId);
    const move = (pointer) => {
      const value = measure(pointer);
      control.value = String(Math.max(Number(control.min), Math.min(Number(control.max), Math.round(value))));
      control.dispatchEvent(new Event("input", { bubbles: true }));
    };
    const finish = () => {
      control.removeEventListener("pointermove", move);
      control.removeEventListener("pointerup", finish);
      control.removeEventListener("pointercancel", finish);
    };
    control.addEventListener("pointermove", move);
    control.addEventListener("pointerup", finish);
    control.addEventListener("pointercancel", finish);
    move(event);
  };
  const show = (view, focus = false) => {
    if (view !== "lesson" && view !== "laboratory") throw new Error("Tour workspace view is not admitted");
    content.dataset.narrowView = view;
    for (const button of viewButtons) button.setAttribute("aria-pressed", String(button.dataset.tourView === view));
    if (focus) (view === "lesson" ? root.querySelector("#chapter") : laboratory).focus({ preventScroll: true });
  };

  setWidth(readingState.workspace.narrativePercent, false);
  setLaboratoryGeometry(".tour-patchbay-height", "--tour-patchbay-percent", readingState.setPatchbayPercent, readingState.workspace.patchbayPercent, false);
  setLaboratoryGeometry(".tour-source-width", "--tour-source-percent", readingState.setSourcePercent, readingState.workspace.sourcePercent, false);
  width.addEventListener("input", () => setWidth(width.value, true));
  width.addEventListener("pointerdown", (event) => dragDivider(width, event, (pointer) => {
    const bounds = content.getBoundingClientRect();
    return (pointer.clientX - bounds.left) * 100 / bounds.width;
  }));
  laboratory.addEventListener("input", (event) => {
    if (event.target.matches(".tour-patchbay-height")) setLaboratoryGeometry(".tour-patchbay-height", "--tour-patchbay-percent", readingState.setPatchbayPercent, event.target.value, true);
    else if (event.target.matches(".tour-source-width")) setLaboratoryGeometry(".tour-source-width", "--tour-source-percent", readingState.setSourcePercent, event.target.value, true);
  });
  laboratory.addEventListener("pointerdown", (event) => {
    const control = event.target.closest(".pane-divider-control");
    if (!control) return;
    const bounds = (control.matches(".tour-patchbay-height") ? laboratory : control.closest(".runner")).getBoundingClientRect();
    dragDivider(control, event, (pointer) => control.matches(".tour-patchbay-height")
      ? (pointer.clientY - bounds.top) * 100 / bounds.height
      : (pointer.clientX - bounds.left) * 100 / bounds.width);
  });
  new MutationObserver(() => {
    setLaboratoryGeometry(".tour-patchbay-height", "--tour-patchbay-percent", readingState.setPatchbayPercent, readingState.workspace.patchbayPercent, false);
    setLaboratoryGeometry(".tour-source-width", "--tour-source-percent", readingState.setSourcePercent, readingState.workspace.sourcePercent, false);
  }).observe(laboratory, { childList: true });
  reset.addEventListener("click", () => {
    setWidth(46, true);
    setLaboratoryGeometry(".tour-patchbay-height", "--tour-patchbay-percent", readingState.setPatchbayPercent, 55, true);
    setLaboratoryGeometry(".tour-source-width", "--tour-source-percent", readingState.setSourcePercent, 60, true);
  });
  for (const button of viewButtons) button.addEventListener("click", () => show(button.dataset.tourView, true));
  return Object.freeze({
    showLesson: (focus = false) => show("lesson", focus),
    showLaboratory: (focus = false) => show("laboratory", focus),
  });
}

export function createTourRunnerActions(runtime, presentation, slot, runLabel, onRun, onStop, onRestore) {
  let revision = 0;
  const application = createTourApplicationActionForwarder({ runtime, onRun, onStop, onRestore });
  return Object.freeze({
    render(running) {
      presentation.present(slot, {
        revision: ++revision,
        actions: TOUR_APPLICATION_ACTIONS.actions.map(({ id }) => ({ id, event: "activate" })),
        nodes: [
          { parent: null, component: "action-group", key: "runner-actions", text: "Play and draft actions", action: null },
          { parent: 0, component: "button", key: "run", text: runLabel, action: running ? null : 0 },
          { parent: 0, component: "button", key: "stop", text: "Stop", action: running ? 1 : null },
          { parent: 0, component: "button", key: "restore", text: "Restore canonical source", action: 2 },
        ],
      }, {
        onEvent(event) {
          presentation.nextEvent(slot);
          application.forward(event);
        },
      });
    },
  });
}
import { createTourApplicationActionForwarder, TOUR_APPLICATION_ACTIONS } from "./tour-application-actions.mjs";
