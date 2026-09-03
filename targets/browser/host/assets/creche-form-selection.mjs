const encoder = new TextEncoder();
const decoder = new TextDecoder();
const MAXIMUM_SEARCH_BYTES = 128;

export function readReviewedFormInventory(runtime, source) {
  const bytes = encoder.encode(source);
  if (bytes.length === 0 || bytes.length > runtime.conduit_creche_input_capacity()) {
    throw new Error("reviewed Form inventory is outside the admitted runtime bound");
  }
  new Uint8Array(runtime.memory.buffer, runtime.conduit_creche_input_ptr(), bytes.length).set(bytes);
  const code = runtime.conduit_creche_reviewed_inventory(bytes.length);
  const output = JSON.parse(decoder.decode(new Uint8Array(
    runtime.memory.buffer,
    runtime.conduit_creche_output_ptr(),
    runtime.conduit_creche_output_len(),
  )));
  if (code < 0) throw new Error(output.message ?? `reviewed Form inventory refused (${code})`);
  return validateInventory(output);
}

export function openFormSelection(inventory, persisted = null, handoff = null) {
  validateInventory(inventory);
  const candidates = [];
  if (persisted !== null) {
    if (persisted?.schema !== "conduit.creche/form-selection@1"
      || !Array.isArray(persisted.forms)
      || persisted.forms.length > inventory.maximum_selection) {
      throw new Error("persisted Crèche Form selection is malformed or over capacity");
    }
    candidates.push(...persisted.forms);
  }
  if (handoff !== null) candidates.push(handoff);
  const selected = [];
  const refusals = [];
  for (const candidate of candidates) {
    const current = exactCurrent(inventory, candidate);
    if (!current) {
      refusals.push({ disposition: "stale-form-identity", candidate });
      continue;
    }
    if (selected.some((form) => form.checked_form_id === current.checked_form_id)) continue;
    if (selected.length === inventory.maximum_selection) {
      refusals.push({ disposition: "selection-capacity-exhausted", candidate });
      continue;
    }
    selected.push(current);
  }
  return Object.freeze({ selected, refusals });
}

export function toggleForm(inventory, selected, name) {
  const form = inventory.forms.find((candidate) => candidate.name === name);
  if (!form) throw new Error(`reviewed Form ${JSON.stringify(name)} is absent`);
  if (selected.some((candidate) => candidate.checked_form_id === form.checked_form_id)) {
    return selected.filter((candidate) => candidate.checked_form_id !== form.checked_form_id);
  }
  if (selected.length >= inventory.maximum_selection) {
    throw new Error("initial Form selection capacity is exhausted");
  }
  return [...selected, form];
}

export function searchForms(inventory, query) {
  if (typeof query !== "string" || encoder.encode(query).length > MAXIMUM_SEARCH_BYTES) {
    throw new Error("Form search is outside its admitted bound");
  }
  const terms = query.trim().toLocaleLowerCase().split(/\s+/u).filter(Boolean);
  return inventory.forms.filter((form) => {
    const text = `${form.title} ${form.name} ${form.required_kinds.join(" ")}`.toLocaleLowerCase();
    return terms.every((term) => text.includes(term));
  });
}

export function persistedFormSelection(inventory, selected) {
  return {
    schema: "conduit.creche/form-selection@1",
    inventory_source_document_id: inventory.source_document_id,
    forms: selected.map(exactIdentity),
  };
}

export function encodedFormSelection(selected) {
  return JSON.stringify(selected.map(exactIdentity));
}

function exactCurrent(inventory, candidate) {
  if (!candidate || typeof candidate !== "object") return null;
  return inventory.forms.find((form) => form.name === candidate.name
    && form.source_document_id === candidate.source_document_id
    && form.checked_form_id === candidate.checked_form_id) ?? null;
}

function exactIdentity(form) {
  return {
    name: form.name,
    source_document_id: form.source_document_id,
    checked_form_id: form.checked_form_id,
  };
}

function validateInventory(inventory) {
  if (inventory?.schema !== "conduit.creche/reviewed-form-inventory@1"
    || typeof inventory.source_document_id !== "string"
    || !Number.isSafeInteger(inventory.maximum_selection)
    || inventory.maximum_selection < 0
    || !Array.isArray(inventory.forms)
    || inventory.forms.length > inventory.maximum_selection) {
    throw new Error("reviewed Form inventory is malformed or over capacity");
  }
  const names = new Set();
  const identities = new Set();
  for (const form of inventory.forms) {
    if (typeof form?.name !== "string" || typeof form.title !== "string"
      || form.source_document_id !== inventory.source_document_id
      || typeof form.checked_form_id !== "string" || !Array.isArray(form.required_kinds)
      || names.has(form.name) || identities.has(form.checked_form_id)) {
      throw new Error("reviewed Form inventory contains an invalid or duplicate entry");
    }
    names.add(form.name);
    identities.add(form.checked_form_id);
  }
  return inventory;
}
