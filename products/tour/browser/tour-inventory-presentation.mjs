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
