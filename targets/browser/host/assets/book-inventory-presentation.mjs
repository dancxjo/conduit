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
      || !Array.isArray(form.required_kinds)) throw new Error("reviewed Form Gallery entry is malformed");
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
  return Object.freeze({ surface, heading });
}

function createGalleryCard(document, form, crecheUrl, onOpen) {
  const item = document.createElement("li");
  item.className = "form-gallery-card";
  item.dataset.searchText = `${form.title} ${form.name} ${form.required_kinds.join(" ")}`.toLocaleLowerCase();
  const heading = document.createElement("h2");
  heading.textContent = form.title;
  const requirements = document.createElement("p");
  requirements.textContent = `Semantic requirements: ${form.required_kinds.join(", ")}.`;
  const identity = document.createElement("code");
  identity.textContent = form.checked_form_id;
  const actions = document.createElement("div");
  actions.className = "form-gallery-actions";
  const open = document.createElement("button");
  open.type = "button";
  open.textContent = "Open in laboratory";
  open.addEventListener("click", () => onOpen(form));
  const add = document.createElement("a");
  const handoff = new URL(crecheUrl, document.baseURI);
  handoff.searchParams.set("form", form.name);
  handoff.searchParams.set("source_document_id", form.source_document_id);
  handoff.searchParams.set("checked_form_id", form.checked_form_id);
  add.href = handoff.href;
  add.textContent = "Add to new Body";
  actions.append(open, add);
  item.append(heading, requirements, identity, actions);
  return item;
}

export function presentBookInventory(presentation, inventory) {
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
  for (const [index, entry] of inventory.entries.entries()) {
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
  presentation.present("book-inventory", { revision: ++revision, actions: [], nodes });
}
