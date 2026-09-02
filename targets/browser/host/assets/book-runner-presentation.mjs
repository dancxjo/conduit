export function createBookRunnerStatus(presentation, slot, initialText) {
  let revision = 0;
  const render = (component, text) => presentation.present(slot, {
    revision: ++revision,
    actions: [],
    nodes: [
      { parent: null, component, key: "play-status", text, action: null },
    ],
  });
  const status = Object.freeze({
    ordinary(text) { return render("status", text); },
    success(text) { return render("success-status", text); },
    failure(text) { return render("failure-status", text); },
  });
  queueMicrotask(() => status.ordinary(initialText));
  return status;
}
