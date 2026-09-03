export function createPatchbaySharedPresentation(presentation, scope = document) {
  const revisions = new Map();
  const present = (slot, description, options = {}) => {
    const revision = (revisions.get(slot) ?? 0) + 1;
    revisions.set(slot, revision);
    return presentation.present(slot, { revision, ...description }, {
      ...options,
      onEvent(event) {
        presentation.nextEvent(slot);
        options.onEvent?.(event);
      },
    });
  };
  return Object.freeze({
    present,
    definitions(slot, entries) {
      return present(slot, {
        actions: [],
        nodes: [
          { parent: null, component: "definition-table", key: "facts", text: "Exact facts", action: null },
          ...entries.map(([name, value], index) => ({ parent: 0, component: "definition", key: `fact-${index}`, text: name, value: String(value ?? "not present"), valueCapacity: 65_536, action: null })),
        ],
      });
    },
    actions(slot, entries) {
      return present(slot, {
        actions: entries.map((_, index) => ({ id: `action-${index}`, event: "activate" })),
        nodes: [
          { parent: null, component: "action-group", key: "actions", text: "Available actions", action: null },
          ...entries.map((entry, index) => ({ parent: 0, component: "button", key: `action-${index}`, text: entry.label, state: entry.disabled ? "unavailable" : "ready", action: entry.disabled ? null : index })),
        ],
      }, { onEvent(event) { entries[Number(event.action.slice("action-".length))]?.run(); } });
    },
    status(slot, text, component = "status") {
      return present(slot, { actions: [], nodes: [{ parent: null, component, key: "status", text, action: null }] });
    },
    navigation(slot, label, entries, currentKey) {
      present(slot, {
        actions: entries.map((_, index) => ({ id: `navigate-${index}`, event: "activate" })),
        nodes: [
          { parent: null, component: "navigation", key: "navigation", text: label, value: currentKey ?? "", valueCapacity: currentKey ? 32 : 0, action: null },
          ...entries.map((entry, index) => ({ parent: 0, component: "button", key: entry.key, text: entry.label, action: index })),
        ],
      }, { onEvent(event) { entries[Number(event.action.slice("navigate-".length))]?.run(); } });
      const controls = scope.querySelector(`[data-application-slot="${slot}"]`).querySelectorAll("button");
      controls.forEach((control, index) => entries[index].annotate?.(control));
    },
  });
}
