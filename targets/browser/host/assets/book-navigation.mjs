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
