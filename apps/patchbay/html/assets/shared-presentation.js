export function createPatchbaySharedPresentation(presentation, scope = document) {
  const revisions = new Map();
  const activeListChunks = new Map();
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
    boundedList(slotPrefix, entries, selectedIdentity) {
      const slots = Array.from(scope.querySelectorAll(`[data-application-slot^="${slotPrefix}-"]`));
      if (entries.length > slots.length * 16) throw new Error(`shared ${slotPrefix} capacity exceeded`);
      const usedChunks = Math.ceil(entries.length / 16), previousChunks = activeListChunks.get(slotPrefix) ?? 0;
      for (let slotIndex = 0; slotIndex < Math.max(usedChunks, previousChunks); slotIndex += 1) {
        const root = slots[slotIndex];
        const chunk = entries.slice(slotIndex * 16, slotIndex * 16 + 16);
        root.hidden = chunk.length === 0;
        const slot = `${slotPrefix}-${slotIndex}`;
        if (chunk.length === 0) {
          present(slot, { actions: [], nodes: [{ parent: null, component: "stack", key: "empty", text: "", action: null }] });
          continue;
        }
        const selected = chunk.findIndex(entry => entry.identity === selectedIdentity);
        present(slot, {
          actions: chunk.map((_, index) => ({ id: `choose-${index}`, event: "activate" })),
          nodes: [
            { parent: null, component: "navigation", key: "items", text: "Canonical Presentation subjects", value: selected < 0 ? "" : `item-${selected}`, valueCapacity: selected < 0 ? 0 : 32, action: null },
            ...chunk.map((entry, index) => ({ parent: 0, component: "button", key: `item-${index}`, text: entry.text, action: index })),
          ],
        }, { onEvent(event) { chunk[Number(event.action.slice("choose-".length))]?.run(); } });
        root.querySelectorAll("button").forEach((control, index) => {
          control.setAttribute("aria-pressed", String(chunk[index].identity === selectedIdentity));
          chunk[index].annotate?.(control);
        });
      }
      activeListChunks.set(slotPrefix, usedChunks);
    },
  });
}
