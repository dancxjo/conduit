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
      || [form.category, form.description, form.instruction, form.note].some((value) => typeof value !== "string")
      || typeof form.source_document_id !== "string" || typeof form.checked_form_id !== "string"
      || !Number.isInteger(form.presentation_profile) || form.presentation_profile < 0 || form.presentation_profile > 3
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
    outputProfile: form.presentation_profile,
  });
}

export function createReviewedFormGallery(api, presentation, surface, gallery, crecheUrl, onOpen) {
  let query = "";
  let selected = null;
  const handoff = new URL(crecheUrl, surface.ownerDocument.baseURI);
  if (handoff.origin !== new URL(surface.ownerDocument.baseURI).origin || !handoff.pathname.startsWith("/")) {
    throw new Error("Crèche product handoff is outside the admitted site boundary");
  }
  const admittedCrechePath = `${handoff.pathname}${handoff.search}`;

  const render = () => {
    const request = encoder.encode(JSON.stringify({
      revision: ++revision,
      query,
      selected_checked_form_id: selected,
      creche_url: admittedCrechePath,
    }));
    if (request.length > api.conduit_browser_form_input_capacity()) {
      throw new Error("reviewed Form Gallery presentation request is over capacity");
    }
    new Uint8Array(api.memory.buffer, api.conduit_browser_form_input_ptr(), request.length).set(request);
    if (api.conduit_browser_form_reviewed_gallery_view(request.length) < 0) {
      throw new Error("reviewed Form Gallery presentation was refused");
    }
    const encodedView = new Uint8Array(
      api.memory.buffer,
      api.conduit_browser_form_output_ptr(),
      api.conduit_browser_form_output_len(),
    ).slice();
    presentation.present("tour-form-gallery", encodedView, { onEvent(event) {
      presentation.nextEvent("tour-form-gallery");
      if (event.action === "gallery.search") query = decoder.decode(event.value);
      else {
        const match = /^gallery\.(open|inspect)\.(\d+)$/u.exec(event.action);
        if (!match || !gallery.forms[Number(match[2])]) return;
        onOpen(gallery.forms[Number(match[2])], match[1]);
      }
      render();
    } });
  };
  render();
  return Object.freeze({
    surface,
    heading: () => surface.querySelector('[data-application-key="gallery-heading"]'),
    select(checkedFormId) {
      selected = checkedFormId;
      render();
    },
  });
}

export function presentTourInventory(presentation, inventory) {
  // Two inventory nodes, at most 32 offers, and three navigation nodes keep
  // each page within 40 nodes of the finite 128-node presentation envelope.
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


// Compose the experience around the existing runner. Controls, source and
// output retain their original handlers and their production runtime ownership.
export function presentGalleryExperience(runner, form, presentation) {
  runner.classList.add("gallery-runner");
  runner.dataset.galleryForm = form.name;
  const document = runner.ownerDocument;
  const introduction = document.createElement("header");
  const slot = `gallery-experience-${++revision}`;
  introduction.dataset.applicationSlot = slot;
  introduction.className = "gallery-experience";
  runner.prepend(introduction);
  presentation.present(slot, { revision, actions: [], nodes: [
    { parent: null, component: "stack", key: "experience", text: "", action: null },
    { parent: 0, component: "paragraph", key: "experience-category", text: form.category, action: null },
    { parent: 0, component: "heading", key: "experience-title", text: form.title, action: null },
    { parent: 0, component: "paragraph", key: "experience-description", text: form.description, action: null },
    { parent: 0, component: "paragraph", key: "experience-instruction", text: form.instruction, action: null },
    { parent: 0, component: "paragraph", key: "experience-note", text: form.note, action: null },
  ] });
  const inspector = document.createElement("details");
  inspector.className = "gallery-inspector";
  const summary = document.createElement("summary");
  summary.textContent = "Open the wiring & source";
  inspector.append(summary);
  const result = runner.querySelector(".result");
  const controls = runner.querySelector('[data-application-slot^="tour-runner-actions-"]');
  result.prepend(controls);
  presentGalleryMessage(runner, form, presentation, result, controls);
  const inputButton = runner.querySelector(".input-button");
  inputButton.textContent = form.name === "secret-knock-demo" ? "Knock here" : "Hold to light";
  result.querySelector("h2").textContent = "Live result";
  const indicator = result.querySelector(".indicator");
  if (!["morse_network", "button_across_room", "firefly-choir"].includes(form.name)) indicator.hidden = true;
  inspector.append(runner.querySelector(".compact-patchbay"), runner.querySelector(".editor"));
  runner.append(inspector);
  runner.addEventListener("click", (event) => {
    if (event.target.closest('[data-application-key="run"]') && matchMedia("(max-width: 760px)").matches) {
      result.querySelector(".morse").scrollIntoView({ block: "center" });
    }
  });
}


function presentGalleryMessage(runner, form, presentation, result, controls) {
  const original = { morse_network: "SOS", memory_lantern: "READY", desk_telegraph: "CALLING", "night-radio": "NIGHT REPORT" }[form.name];
  if (!original) return;
  const token = JSON.stringify(original);
  const offset = form.source.indexOf(token);
  if (offset < 0 || form.source.indexOf(token, offset + token.length) !== -1) return;
  const prefix = form.source.slice(0, offset);
  const suffix = form.source.slice(offset + token.length);
  const source = runner.querySelector("textarea");
  const surface = runner.ownerDocument.createElement("div");
  const slot = `gallery-message-${++revision}`;
  surface.dataset.applicationSlot = slot;
  surface.className = "gallery-message";
  result.insertBefore(surface, controls);
  const render = () => {
    let value = null;
    if (source.value.startsWith(prefix) && source.value.endsWith(suffix)) {
      try { value = JSON.parse(source.value.slice(prefix.length, -suffix.length)); } catch { /* An edited source remains editable in the inspector. */ }
    }
    const available = typeof value === "string" && value.length <= 24;
    presentation.present(slot, { revision: ++revision, actions: [{ id: "gallery.message", event: "input" }], nodes: [
      { parent: null, component: "stack", key: "message-editor", text: "", action: null },
      { parent: 0, component: "paragraph", key: "message-label", text: "Your message", action: null },
      { parent: 0, component: "text-input", key: "message", text: "Your message", value: available ? value : "", valueCapacity: 96, action: available ? 0 : null },
      { parent: 0, component: "paragraph", key: "message-help", text: available
        ? form.name === "morse_network" ? "Up to 24 letters, numbers, or spaces. Press Run to send it in light." : "Up to 24 characters. Press Run to try your message."
        : "This source has custom edits. Use the source editor, or restore the canonical source to use this field.", action: null },
    ] }, { onEvent(event) {
      presentation.nextEvent(slot);
      const next = decoder.decode(event.value);
      if (event.action !== "gallery.message" || !available || next.length > 24) { render(); return; }
      source.value = `${prefix}${JSON.stringify(next)}${suffix}`;
      source.dispatchEvent(new Event("input", { bubbles: true }));
    } });
    surface.querySelector("input").maxLength = 24;
  };
  source.addEventListener("input", render);
  // Restore uses the runner's existing source change/refresh path.
  runner.addEventListener("click", (event) => {
    if (event.target.closest('[data-application-key="restore"]')) queueMicrotask(render);
  });
  render();
}
