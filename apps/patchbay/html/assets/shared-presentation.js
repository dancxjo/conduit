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
      const result = present(slot, {
        actions: entries.map((_, index) => ({ id: `action-${index}`, event: "activate" })),
        nodes: [
          { parent: null, component: "action-group", key: "actions", text: "Available actions", action: null },
          ...entries.map((entry, index) => ({ parent: 0, component: "button", key: `action-${index}`, text: entry.label, state: entry.disabled ? "unavailable" : "ready", action: entry.disabled ? null : index })),
        ],
      }, { onEvent(event) { entries[Number(event.action.slice("action-".length))]?.run(); } });
      scope.querySelector(`[data-application-slot="${slot}"]`).querySelectorAll("button").forEach((control, index) => entries[index].annotate?.(control));
      return result;
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
    boundedList(slotPrefix, label, entries, selectedIdentity) {
      const slots = [];
      for (let index = 0; ; index += 1) {
        const slot = scope.querySelector(`[data-application-slot="${slotPrefix}-${index}"]`);
        if (!slot) break;
        slots.push(slot);
      }
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
            { parent: null, component: "navigation", key: "items", text: label, value: selected < 0 ? "" : `item-${selected}`, valueCapacity: selected < 0 ? 0 : 32, action: null },
            ...chunk.flatMap((entry, index) => [
              { parent: 0, component: "button", key: `item-${index}`, text: entry.text, action: index },
              ...(entry.detail ? [{ parent: 0, component: "paragraph", key: `detail-${index}`, text: entry.detail, action: null }] : []),
            ]),
          ],
        }, { onEvent(event) { chunk[Number(event.action.slice("choose-".length))]?.run(); } });
        root.querySelectorAll("button").forEach((control, index) => {
          control.setAttribute("aria-pressed", String(chunk[index].identity === selectedIdentity));
          chunk[index].annotate?.(control);
        });
      }
      activeListChunks.set(slotPrefix, usedChunks);
    },
    boundedArtifacts(slotPrefix, entries, entriesPerSlot = 4) {
      const slots = [];
      for (let index = 0; ; index += 1) {
        const slot = scope.querySelector(`[data-application-slot="${slotPrefix}-${index}"]`);
        if (!slot) break;
        slots.push(slot);
      }
      if (entriesPerSlot < 1 || entriesPerSlot > 4 || entries.length > slots.length * entriesPerSlot) throw new Error(`shared ${slotPrefix} capacity exceeded`);
      const usedChunks = Math.ceil(entries.length / entriesPerSlot), previousChunks = activeListChunks.get(slotPrefix) ?? 0;
      for (let slotIndex = 0; slotIndex < Math.max(usedChunks, previousChunks); slotIndex += 1) {
        const root = slots[slotIndex], chunk = entries.slice(slotIndex * entriesPerSlot, slotIndex * entriesPerSlot + entriesPerSlot), slot = `${slotPrefix}-${slotIndex}`;
        root.hidden = chunk.length === 0;
        if (chunk.length === 0) { present(slot, { actions: [], nodes: [{ parent: null, component: "stack", key: "empty", text: "", action: null }] }); continue; }
        const actions = chunk.flatMap(entry => entry.actions);
        const nodes = [{ parent: null, component: "grid", key: "artifacts", text: "", action: null }];
        let actionIndex = 0;
        chunk.forEach((entry, entryIndex) => {
          const parent = nodes.length;nodes.push({ parent: 0, component: "artifact", key: `artifact-${entryIndex}`, text: entry.title, action: null });
          entry.details.forEach((detail, detailIndex) => nodes.push({ parent, component: "paragraph", key: `detail-${entryIndex}-${detailIndex}`, text: detail, action: null }));
          if (entry.definitions?.length) {
            const disclosure = nodes.length;nodes.push({ parent, component: "disclosure", key: `disclosure-${entryIndex}`, text: entry.disclosureLabel ?? "Exact evidence", action: null });
            const table = nodes.length;nodes.push({ parent: disclosure, component: "definition-table", key: `definitions-${entryIndex}`, text: "Exact facts", action: null });
            entry.definitions.forEach(([name, value], definitionIndex) => nodes.push({ parent: table, component: "definition", key: `definition-${entryIndex}-${definitionIndex}`, text: name, value: String(value ?? "not present"), valueCapacity: 65_536, action: null }));
          }
          if (entry.exactValue !== undefined) {
            const disclosure = nodes.length;nodes.push({ parent, component: "disclosure", key: `exact-${entryIndex}`, text: entry.disclosureLabel ?? "Exact evidence", action: null });
            nodes.push({ parent: disclosure, component: "code-block", key: `code-${entryIndex}`, text: entry.language ?? "text", value: entry.exactValue, valueCapacity: 65_536, action: null });
          }
          entry.actions.forEach((action, entryActionIndex) => { nodes.push({ parent, component: "button", key: `action-${entryIndex}-${entryActionIndex}`, text: action.label, action: actionIndex });actionIndex += 1; });
        });
        present(slot, { actions: actions.map((_, index) => ({ id: `artifact-action-${index}`, event: "activate" })), nodes }, { onEvent(event) { actions[Number(event.action.slice("artifact-action-".length))]?.run(); } });
        root.querySelectorAll('[data-application-component="artifact"]').forEach((artifact, index) => chunk[index].annotate?.(artifact));
        root.querySelectorAll("button").forEach((control, index) => actions[index].annotate?.(control));
      }
      activeListChunks.set(slotPrefix, usedChunks);
    },
    boundedEvidence(slotPrefix, label, lines) {
      const slots = [];
      for (let index = 0; ; index += 1) {
        const slot = scope.querySelector(`[data-application-slot="${slotPrefix}-${index}"]`);
        if (!slot) break;
        slots.push(slot);
      }
      const usedChunks = Math.ceil(lines.length / 32), previousChunks = activeListChunks.get(slotPrefix) ?? 0;
      if (usedChunks > slots.length) throw new Error(`shared ${slotPrefix} capacity exceeded`);
      for (let slotIndex = 0; slotIndex < Math.max(usedChunks, previousChunks); slotIndex += 1) {
        const root = slots[slotIndex], chunk = lines.slice(slotIndex * 32, slotIndex * 32 + 32), slot = `${slotPrefix}-${slotIndex}`;
        root.hidden = chunk.length === 0;
        present(slot, { actions: [], nodes: chunk.length ? [
          { parent: null, component: "disclosure", key: "evidence", text: `${label} ${slotIndex + 1}`, action: null },
          { parent: 0, component: "code-block", key: "lines", text: "text", value: chunk.join("\n"), valueCapacity: 65_536, action: null },
        ] : [{ parent: null, component: "stack", key: "empty", text: "", action: null }] });
      }
      activeListChunks.set(slotPrefix, usedChunks);
    },
  });
}
