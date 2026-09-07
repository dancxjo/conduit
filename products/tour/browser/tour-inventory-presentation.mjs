let revision = 0;
const encoder = new TextEncoder();
const decoder = new TextDecoder();

export function readReviewedGallery(api) {
  if (api.conduit_browser_form_reviewed_gallery() < 0) throw new Error("reviewed Form Gallery is unavailable");
  const bytes = new Uint8Array(
    api.memory.buffer,
    api.conduit_browser_form_output_ptr(),
    api.conduit_browser_form_output_len(),
  );
  const projected = JSON.parse(decoder.decode(bytes));
  if (projected?.schema !== "conduit.tour/reviewed-form-gallery@1"
    || !Number.isInteger(projected.maximum_forms) || projected.maximum_forms < 1
    || !Array.isArray(projected.forms) || projected.forms.length > projected.maximum_forms) {
    throw new Error("reviewed Form Gallery is malformed or over capacity");
  }
  for (const form of projected.forms) {
    if (typeof form?.name !== "string" || typeof form.title !== "string" || typeof form.source !== "string"
      || typeof form.source_document_id !== "string" || typeof form.checked_form_id !== "string"
      || !Array.isArray(form.required_kinds)
      || !Number.isSafeInteger(form.realizability?.current_offer_count)
      || form.realizability?.required_kind_count !== form.required_kinds.length
      || !Array.isArray(form.realizability?.requirements)
      || form.realizability.requirements.length !== form.required_kinds.length) {
      throw new Error("reviewed Form Gallery entry is malformed");
    }
  }
  return Object.freeze(projected);
}

export function reviewedFormStage(form) {
  return Object.freeze({
    identity: form.checked_form_id,
    label: form.title,
    source: form.source,
    mode: "run",
    recursive: false,
    faceBack: false,
    multiHost: false,
    showPlan: false,
    sourceDocumentId: form.source_document_id,
    checkedFormId: form.checked_form_id,
  });
}

export function createReviewedFormGallery(document, presentation, surface, gallery, crecheUrl, onOpen) {
  let query = "";
  let selected = null;
  const actions = [{ id: "gallery.search", event: "input" }];
  for (const [index] of gallery.forms.entries()) {
    actions.push(
      { id: `gallery.open.${index}`, event: "activate" },
      { id: `gallery.inspect.${index}`, event: "activate" },
    );
  }

  const render = () => {
    const overCapacity = encoder.encode(query).length > 128;
    const terms = query.trim().toLocaleLowerCase().split(/\s+/u).filter(Boolean);
    const visible = gallery.forms.filter((form) => !overCapacity && terms.every((term) => (
      `${form.title} ${form.name} ${form.required_kinds.join(" ")}`.toLocaleLowerCase().includes(term)
    )));
    const nodes = [
      { parent: null, component: "stack", key: "gallery", text: "", action: null },
      { parent: 0, component: "heading", key: "gallery-heading", text: "Form Gallery", action: null },
      { parent: 0, component: "paragraph", key: "gallery-purpose", text: "Open a reviewed canonical Form in the same laboratory. Browsing acquires no resource or authority; Run admits work separately.", action: null },
      { parent: 0, component: "form-field", key: "gallery-search", text: "", action: null },
      { parent: 3, component: "field-label", key: "search-label", text: "Search reviewed Forms", action: null },
      { parent: 3, component: "field-help", key: "search-help", text: "Title, Form name, or required kind; 128-byte bound.", action: null },
      { parent: 3, component: "text-input", key: "search-input", text: "Search reviewed Forms", value: query, valueCapacity: 512, action: 0 },
      { parent: 0, component: "status", key: "gallery-status", text: overCapacity ? "Search is outside the admitted 128-byte bound." : `${visible.length} reviewed ${visible.length === 1 ? "Form" : "Forms"}`, action: null },
      { parent: 0, component: "grid", key: "gallery-cards", text: "", action: null },
    ];
    for (const [index, form] of gallery.forms.entries()) {
      const card = nodes.length;
      const available = form.realizability.status === "runnable-on-current-browser-host";
      const realization = form.realizability.requirements.map((requirement) => {
        const realizationClass = requirement.realization_class === "pure-kernel-or-local"
          ? "local"
          : requirement.realization_class === "bounded-browser-host-operation"
            ? "browser Host"
            : "unrealized";
        return `${requirement.kind_id}=${requirement.offer_state === "current-host-offer" ? "current" : "missing"}/${realizationClass}`;
      }).join("; ");
      nodes.push(
        { parent: 8, component: "artifact", key: `form-${index}`, text: form.title, action: null },
        { parent: card, component: "paragraph", key: `form-detail-${index}`, text: `Kinds and realization: ${realization}. Offers ${form.realizability.current_offer_count}/${form.realizability.required_kind_count}; ${available ? "runnable here" : "not runnable here"}.`, action: null },
        { parent: card, component: "code", key: `form-id-${index}`, text: form.checked_form_id, action: null },
        { parent: card, component: "action-group", key: `form-actions-${index}`, text: `${form.title} actions`, action: null },
        { parent: card + 3, component: "button", key: `form-open-${index}`, text: "Open in laboratory", valueCapacity: 0, action: 1 + index * 2 },
        { parent: card + 3, component: "button", key: `form-inspect-${index}`, text: "Inspect Patchbay", valueCapacity: 0, action: 2 + index * 2 },
        { parent: card + 3, component: "link", key: `form-add-${index}`, text: "Add to new Body", value: formHandoff(document, crecheUrl, form), valueCapacity: 2_048, action: null },
      );
    }
    presentation.present("tour-form-gallery", { revision: ++revision, actions, nodes }, { onEvent(event) {
      presentation.nextEvent("tour-form-gallery");
      if (event.action === "gallery.search") query = decoder.decode(event.value);
      else {
        const match = /^gallery\.(open|inspect)\.(\d+)$/u.exec(event.action);
        if (!match || !gallery.forms[Number(match[2])]) return;
        onOpen(gallery.forms[Number(match[2])], match[1]);
      }
      render();
    } });
    for (const [index, form] of gallery.forms.entries()) {
      const card = surface.querySelector(`[data-application-key="form-${index}"]`);
      card.dataset.galleryCard = "";
      card.dataset.checkedFormId = form.checked_form_id;
      card.dataset.status = form.realizability.status;
      card.hidden = !visible.includes(form);
      if (form.checked_form_id === selected) card.setAttribute("aria-current", "true");
    }
  };
  render();
  return Object.freeze({
    surface,
    heading: surface.querySelector('[data-application-key="gallery-heading"]'),
    select(checkedFormId) {
      selected = checkedFormId;
      for (const card of surface.querySelectorAll("[data-gallery-card]")) {
        if (card.dataset.checkedFormId === selected) card.setAttribute("aria-current", "true");
        else card.removeAttribute("aria-current");
      }
    },
  });
}

function formHandoff(document, crecheUrl, form) {
  const handoff = new URL(crecheUrl, document.baseURI);
  if (handoff.origin !== new URL(document.baseURI).origin || !handoff.pathname.startsWith("/")) {
    throw new Error("Crèche product handoff is outside the admitted site boundary");
  }
  handoff.searchParams.set("form", form.name);
  handoff.searchParams.set("source_document_id", form.source_document_id);
  handoff.searchParams.set("checked_form_id", form.checked_form_id);
  return `${handoff.pathname}${handoff.search}`;
}

export function presentTourInventory(presentation, inventory) {
  // Two inventory nodes, at most 32 offers, and three navigation nodes fit
  // the existing 40-node application presentation envelope.
  const pageSize = 32;
  const pageCount = Math.max(1, Math.ceil(inventory.entries.length / pageSize));
  let page = 0;
  const render = () => {
    const installed = inventory.entries.filter((entry) => entry.implementation_id !== null);
    const nodes = [
      {
        parent: null,
        component: "disclosure",
        key: "inventory",
        text: `Available gears · ${installed.length} exact browser implementations · ${inventory.limits.maximum_gears} Gear / ${inventory.limits.maximum_cords} Cord bound`,
        action: null,
      },
      { parent: 0, component: "definition-table", key: "offers", text: "Exact browser planning offers", action: null },
    ];
    for (const [offset, entry] of inventory.entries.slice(page * pageSize, (page + 1) * pageSize).entries()) {
      const index = page * pageSize + offset;
      const availability = entry.implementation_id ? "available" : "unavailable";
      nodes.push({
        parent: 1,
        component: "definition",
        key: `offer-${availability}-${index}`,
        text: entry.kind_id,
        value: `${entry.family} · ${entry.classification} · ${entry.reason}`,
        valueCapacity: 1024,
        action: null,
      });
    }
    nodes.push(
      { parent: 0, component: "status", key: "inventory-page", text: `Offers page ${page + 1} of ${pageCount}`, action: null },
      { parent: 0, component: "button", key: "inventory-previous", text: "Previous offers", action: page > 0 ? 0 : null },
      { parent: 0, component: "button", key: "inventory-next", text: "Next offers", action: page + 1 < pageCount ? 1 : null },
    );
    presentation.present("tour-inventory", { revision: ++revision, actions: [
      { id: "tour.inventory.previous", event: "activate" },
      { id: "tour.inventory.next", event: "activate" },
    ], nodes }, { onEvent(event) {
      presentation.nextEvent("tour-inventory");
      if (event.action === "tour.inventory.previous" && page > 0) page -= 1;
      else if (event.action === "tour.inventory.next" && page + 1 < pageCount) page += 1;
      else return;
      render();
    } });
  };
  render();
}
