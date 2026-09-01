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
          { parent: null, component: "navigation", key: "navigation", text: "", action: null },
          { parent: 0, component: "status", key: "progress", text: `Page ${currentPage + 1} of ${pageCount}`, action: null },
          { parent: 0, component: "button", key: "previous", text: "Previous", action: !running && currentPage > 0 ? 0 : null },
          { parent: 0, component: "button", key: "next", text: "Next", action: !running && currentPage < pageCount - 1 ? 1 : null },
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
