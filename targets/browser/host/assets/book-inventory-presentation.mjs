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

export function createReviewedFormGallery(document, gallery, crecheUrl, onOpen) {
  const surface = document.createElement("section");
  surface.className = "form-gallery";
  const header = document.createElement("header");
  const heading = document.createElement("h1");
  heading.tabIndex = -1;
  heading.textContent = "Form Gallery";
  const explanation = document.createElement("p");
  explanation.textContent = "Open a reviewed canonical Form in the same laboratory, inspect its exact Patchbay projection, and run it through this browser Host.";
  const search = document.createElement("label");
  search.className = "form-gallery-search";
  search.textContent = "Search reviewed Forms";
  const input = document.createElement("input");
  input.type = "search";
  input.maxLength = 128;
  input.autocomplete = "off";
  const status = document.createElement("output");
  status.setAttribute("role", "status");
  status.setAttribute("aria-live", "polite");
  search.append(input, status);
  header.append(heading, explanation, search);
  const list = document.createElement("ul");
  list.className = "form-gallery-list";
  for (const form of gallery.forms) list.append(createGalleryCard(document, form, crecheUrl, onOpen));
  surface.append(header, list);

  const filter = () => {
    if (encoder.encode(input.value).length > 128) {
      status.textContent = "Search is outside the admitted 128-byte bound.";
      return;
    }
    const terms = input.value.trim().toLocaleLowerCase().split(/\s+/u).filter(Boolean);
    let visible = 0;
    for (const card of list.children) {
      const matches = terms.every((term) => card.dataset.searchText.includes(term));
      card.hidden = !matches;
      if (matches) visible += 1;
    }
    status.textContent = `${visible} reviewed ${visible === 1 ? "Form" : "Forms"}`;
  };
  input.addEventListener("input", filter);
  filter();
  return Object.freeze({
    surface,
    heading,
    select(checkedFormId) {
      for (const card of list.children) {
        if (card.dataset.checkedFormId === checkedFormId) card.setAttribute("aria-current", "true");
        else card.removeAttribute("aria-current");
      }
    },
  });
}

function createGalleryCard(document, form, crecheUrl, onOpen) {
  const item = document.createElement("li");
  item.className = "form-gallery-card";
  item.dataset.checkedFormId = form.checked_form_id;
  item.dataset.searchText = `${form.title} ${form.name} ${form.required_kinds.join(" ")}`.toLocaleLowerCase();
  const heading = document.createElement("h2");
  heading.textContent = form.title;
  const requirements = document.createElement("p");
  requirements.textContent = `Semantic requirements: ${form.required_kinds.join(", ")}.`;
  const availability = document.createElement("p");
  availability.className = "form-gallery-availability";
  availability.dataset.status = form.realizability.status;
  availability.textContent = form.realizability.status === "runnable-on-current-browser-host"
    ? `Runnable here now · ${form.realizability.current_offer_count} of ${form.realizability.required_kind_count} checked kinds have current browser Host offers.`
    : `Not runnable here · ${form.realizability.required_kind_count - form.realizability.current_offer_count} checked kind offer(s) are missing.`;
  const realization = document.createElement("ul");
  realization.className = "form-gallery-realization";
  for (const requirement of form.realizability.requirements) {
    const entry = document.createElement("li");
    const realizationClass = requirement.realization_class === "pure-kernel-or-local"
      ? "local/kernel"
      : requirement.realization_class === "bounded-browser-host-operation"
        ? "bounded Host operation"
        : "no current realization";
    entry.textContent = `${requirement.kind_id} · ${requirement.offer_state === "current-host-offer" ? "current offer" : "missing offer"} · ${realizationClass}`;
    realization.append(entry);
  }
  const authority = document.createElement("p");
  authority.className = "form-gallery-authority";
  authority.textContent = "Browsing acquires no resource or authority; Run admits work separately.";
  const identity = document.createElement("code");
  identity.textContent = form.checked_form_id;
  const actions = document.createElement("div");
  actions.className = "form-gallery-actions";
  const open = document.createElement("button");
  open.type = "button";
  open.textContent = "Open in laboratory";
  open.addEventListener("click", () => onOpen(form, "open"));
  const inspect = document.createElement("button");
  inspect.type = "button";
  inspect.textContent = "Inspect Patchbay";
  inspect.addEventListener("click", () => onOpen(form, "inspect"));
  const add = document.createElement("a");
  const handoff = new URL(crecheUrl, document.baseURI);
  handoff.searchParams.set("form", form.name);
  handoff.searchParams.set("source_document_id", form.source_document_id);
  handoff.searchParams.set("checked_form_id", form.checked_form_id);
  add.href = handoff.href;
  add.textContent = "Add to new Body";
  actions.append(open, inspect, add);
  item.append(heading, requirements, availability, realization, authority, identity, actions);
  return item;
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
    presentation.present("book-inventory", { revision: ++revision, actions: [
      { id: "book.inventory.previous", event: "activate" },
      { id: "book.inventory.next", event: "activate" },
    ], nodes }, { onEvent(event) {
      presentation.nextEvent("book-inventory");
      if (event.action === "book.inventory.previous" && page > 0) page -= 1;
      else if (event.action === "book.inventory.next" && page + 1 < pageCount) page += 1;
      else return;
      render();
    } });
  };
  render();
}
