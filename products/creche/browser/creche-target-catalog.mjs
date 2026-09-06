const CATALOG_SCHEMA = "conduit.creche/physical-host-target-catalog@1";
const ENTRY_SCHEMA = "conduit.creche/physical-host-target-entry@1";
const ADAPTER_SCHEMA = "conduit.creche/physical-host-target-adapter@1";
const FAILURE_SCHEMA = "conduit.creche/physical-host-target-catalog-failure@1";
const MAXIMUM_FAMILIES = 16;
const MAXIMUM_ENTRIES = 64;
const MAXIMUM_STRATEGIES_PER_ENTRY = 8;
const MAXIMUM_CARRIERS_PER_CLASS = 8;
const MAXIMUM_CATALOG_BYTES = 128 * 1024;
const encoder = new TextEncoder();

export const PHYSICAL_HOST_INTENTIONS = Object.freeze([
  Object.freeze({ id: "fabricate-new", label: "Fabricate new machinery", resultKind: "artifact" }),
  Object.freeze({ id: "install-existing", label: "Install on an existing computer", resultKind: "installation" }),
  Object.freeze({ id: "attach-running", label: "Attach an already running Host", resultKind: "attachment" }),
]);

export function createPhysicalHostTargetCatalog({
  generation,
  minimumGeneration = 0,
  contributions,
  bounds = {},
}) {
  const admittedBounds = requireCatalogBounds(bounds);
  if (!Number.isSafeInteger(generation) || generation <= 0) {
    refuse("InvalidCatalogGeneration", "physical Host target catalog generation must be a positive safe integer", generation);
  }
  if (!Number.isSafeInteger(minimumGeneration) || minimumGeneration < 0) {
    refuse("InvalidCatalogGeneration", "physical Host target catalog minimum generation is invalid", generation);
  }
  if (generation <= minimumGeneration) {
    refuse("StaleCatalogGeneration", "physical Host target catalog generation is not newer than the admitted generation", generation, {
      minimum_generation: minimumGeneration,
    });
  }
  if (!Array.isArray(contributions) || contributions.length === 0) {
    refuse("EmptyCatalog", "physical Host target catalog has no target-owned contributions", generation);
  }
  if (contributions.length > admittedBounds.maximumEntries) {
    refuse("CatalogBound", "physical Host target catalog exceeds its admitted entry bound", generation, {
      entries: contributions.length,
      maximum_entries: admittedBounds.maximumEntries,
    });
  }

  const seenTargetIds = new Set();
  const seenProfiles = new Set();
  const familyLabels = new Map();
  const records = contributions.map((contribution) => {
    const record = requireContribution(contribution, generation);
    const targetId = record.entry.target.id;
    const profileKey = [record.entry.family.id, record.entry.target.model_id, record.entry.target.profile_id].join("\u0000");
    if (seenTargetIds.has(targetId) || seenProfiles.has(profileKey)) {
      refuse("DuplicateIdentity", "physical Host target catalog contains a duplicate exact target or family/model/profile identity", generation, {
        target_id: targetId,
        family_id: record.entry.family.id,
        model_id: record.entry.target.model_id,
        profile_id: record.entry.target.profile_id,
      });
    }
    seenTargetIds.add(targetId);
    seenProfiles.add(profileKey);
    const priorLabel = familyLabels.get(record.entry.family.id);
    if (priorLabel && priorLabel !== record.entry.family.label) {
      refuse("IncompatibleFamily", "one physical Host family identity has conflicting labels", generation, {
        family_id: record.entry.family.id,
      });
    }
    familyLabels.set(record.entry.family.id, record.entry.family.label);
    return record;
  });

  if (familyLabels.size > admittedBounds.maximumFamilies) {
    refuse("CatalogBound", "physical Host target catalog exceeds its admitted family bound", generation, {
      families: familyLabels.size,
      maximum_families: admittedBounds.maximumFamilies,
    });
  }

  const entries = Object.freeze(records.map(({ entry }) => entry));
  const families = Object.freeze([...familyLabels].map(([id, label]) => Object.freeze({
    id,
    label,
    entries: Object.freeze(entries.filter((entry) => entry.family.id === id)),
  })));
  const snapshot = Object.freeze({
    schema: CATALOG_SCHEMA,
    generation,
    bounds: admittedBounds,
    families,
    entries,
  });
  const catalogBytes = jsonBytes(snapshot, "catalog", generation);
  if (catalogBytes > admittedBounds.maximumCatalogBytes) {
    refuse("CatalogBound", "physical Host target catalog exceeds its admitted byte bound", generation, {
      bytes: catalogBytes,
      maximum_catalog_bytes: admittedBounds.maximumCatalogBytes,
    });
  }

  function createAdapter({ targetId, host, presentationFor }) {
    const record = records.find(({ entry }) => entry.target.id === targetId);
    if (!record) {
      refuse("UnknownTarget", "selected physical Host target is absent from the current catalog generation", generation, {
        target_id: targetId,
      });
    }
    let adapter;
    try {
      adapter = record.createAdapter({ host, presentationFor });
    } catch (error) {
      refuse("IncompatibleAdapter", "physical Host target adapter factory refused its catalog entry", generation, {
        target_id: targetId,
        cause: error instanceof Error ? error.message : String(error),
      });
    }
    requireCompatibleAdapter(adapter, record.entry, generation);
    return adapter;
  }

  return Object.freeze({ ...snapshot, createAdapter });
}

function requireCatalogBounds(bounds) {
  const admitted = {
    maximumFamilies: bounds.maximumFamilies ?? MAXIMUM_FAMILIES,
    maximumEntries: bounds.maximumEntries ?? MAXIMUM_ENTRIES,
    maximumCatalogBytes: bounds.maximumCatalogBytes ?? MAXIMUM_CATALOG_BYTES,
  };
  if (!boundedInteger(admitted.maximumFamilies, 1, MAXIMUM_FAMILIES)
    || !boundedInteger(admitted.maximumEntries, 1, MAXIMUM_ENTRIES)
    || !boundedInteger(admitted.maximumCatalogBytes, 1024, MAXIMUM_CATALOG_BYTES)) {
    refuse("CatalogBound", "physical Host target catalog bounds are missing or exceed the catalog maxima", null);
  }
  return Object.freeze(admitted);
}

function requireContribution(contribution, generation) {
  if (!contribution || contribution.schema !== ENTRY_SCHEMA || typeof contribution.createAdapter !== "function") {
    refuse("IncompatibleContribution", "physical Host target contribution contract is incomplete", generation);
  }
  const family = contribution.family;
  const target = contribution.target;
  if (!family || !boundedText(family.id, 256) || !boundedText(family.label, 128)
    || !target || !boundedText(target.id, 256) || !boundedText(target.label, 128)
    || !boundedText(target.model_id, 256) || !boundedText(target.profile_id, 256)) {
    refuse("IncompatibleContribution", "physical Host target contribution identity is missing or outside its finite bound", generation);
  }
  const intentions = requireIntentions(contribution.intentions, generation, target.id);
  const fabricationStrategies = requireIdentifiedArray(
    contribution.fabrication_strategies,
    "fabrication strategy",
    MAXIMUM_STRATEGIES_PER_ENTRY,
    generation,
    target.id,
  );
  const carriers = contribution.carriers;
  if (!carriers || typeof carriers !== "object") {
    refuse("IncompatibleContribution", "physical Host target contribution carrier classes are missing", generation, { target_id: target.id });
  }
  const carrierSnapshot = {};
  for (const name of ["deployment", "installation", "attachment", "observation"]) {
    carrierSnapshot[name] = requireIdentifiedArray(
      carriers[name],
      `${name} carrier`,
      MAXIMUM_CARRIERS_PER_CLASS,
      generation,
      target.id,
    );
  }
  if (!boundedText(contribution.expected_join_contract, 256)) {
    refuse("IncompatibleContribution", "physical Host target contribution expected join contract is missing", generation, { target_id: target.id });
  }
  const adapterBounds = contribution.bounds;
  if (!adapterBounds || !boundedInteger(adapterBounds.maximumOperations, 1, 16)
    || !boundedInteger(adapterBounds.maximumOperationEvidenceBytes, 256, 80 * 1024)
    || !boundedInteger(adapterBounds.maximumRetainedEvidenceBytes, 1024, 128 * 1024)) {
    refuse("IncompatibleContribution", "physical Host target contribution workflow bounds are missing or invalid", generation, { target_id: target.id });
  }
  const entry = Object.freeze({
    schema: ENTRY_SCHEMA,
    family: Object.freeze({ id: family.id, label: family.label }),
    target: Object.freeze({
      id: target.id,
      label: target.label,
      model_id: target.model_id,
      profile_id: target.profile_id,
    }),
    intentions,
    fabrication_strategies: fabricationStrategies,
    carriers: Object.freeze(carrierSnapshot),
    bounds: Object.freeze({ ...adapterBounds }),
    expected_join_contract: contribution.expected_join_contract,
    target_profile: requireTargetProfile(contribution.target_profile, generation, target.id),
  });
  return Object.freeze({ entry, createAdapter: contribution.createAdapter });
}

function requireTargetProfile(profile, generation, targetId) {
  if (!profile || typeof profile !== "object" || Array.isArray(profile)
    || !boundedText(profile.schema, 256)) {
    refuse("IncompatibleContribution", "physical Host target profile declaration is missing", generation, { target_id: targetId });
  }
  let copy;
  try {
    copy = JSON.parse(JSON.stringify(profile));
  } catch (error) {
    refuse("IncompatibleContribution", "physical Host target profile declaration is not finite JSON", generation, {
      target_id: targetId,
      cause: error instanceof Error ? error.message : String(error),
    });
  }
  return deepFreeze(copy);
}

function deepFreeze(value) {
  if (value && typeof value === "object") {
    for (const child of Object.values(value)) deepFreeze(child);
    Object.freeze(value);
  }
  return value;
}

function requireIntentions(intentions, generation, targetId) {
  if (!Array.isArray(intentions) || intentions.length !== PHYSICAL_HOST_INTENTIONS.length) {
    refuse("IncompatibleContribution", "physical Host target contribution must classify all three intentions", generation, { target_id: targetId });
  }
  return Object.freeze(PHYSICAL_HOST_INTENTIONS.map((expected) => {
    const intention = intentions.find(({ id }) => id === expected.id);
    if (!intention || typeof intention.supported !== "boolean" || intention.resultKind !== expected.resultKind) {
      refuse("IncompatibleContribution", `physical Host target contribution misclassified ${expected.id}`, generation, { target_id: targetId });
    }
    return Object.freeze({ id: intention.id, resultKind: intention.resultKind, supported: intention.supported });
  }));
}

function requireIdentifiedArray(values, name, maximum, generation, targetId) {
  if (!Array.isArray(values) || values.length > maximum) {
    refuse("CatalogBound", `physical Host target ${name} list exceeds its admitted bound`, generation, { target_id: targetId });
  }
  const ids = new Set();
  return Object.freeze(values.map((value) => {
    if (!value || !boundedText(value.id, 256) || !boundedText(value.label, 128) || ids.has(value.id)) {
      refuse("IncompatibleContribution", `physical Host target ${name} identity is missing, duplicate, or outside its finite bound`, generation, { target_id: targetId });
    }
    ids.add(value.id);
    return Object.freeze({ id: value.id, label: value.label });
  }));
}

function requireCompatibleAdapter(adapter, entry, generation) {
  const methods = ["createOptions", "obtain", "bind", "realize", "observe", "cancel"];
  if (!adapter || adapter.schema !== ADAPTER_SCHEMA || methods.some((name) => typeof adapter[name] !== "function")) {
    refuse("IncompatibleAdapter", "physical Host target adapter contract is incomplete", generation, { target_id: entry.target.id });
  }
  if (adapter.target?.id !== entry.target.id || adapter.target?.label !== entry.target.label
    || adapter.target?.model_id !== entry.target.model_id || adapter.target?.profile_id !== entry.target.profile_id) {
    refuse("IncompatibleAdapter", "physical Host target adapter identity does not match its catalog entry", generation, { target_id: entry.target.id });
  }
  for (const expected of entry.intentions) {
    const actual = adapter.modes?.find(({ id }) => id === expected.id);
    if (!actual || actual.supported !== expected.supported || actual.resultKind !== expected.resultKind) {
      refuse("IncompatibleAdapter", `physical Host target adapter does not match catalog intention ${expected.id}`, generation, { target_id: entry.target.id });
    }
  }
  for (const name of ["maximumOperations", "maximumOperationEvidenceBytes", "maximumRetainedEvidenceBytes"]) {
    if (adapter.bounds?.[name] !== entry.bounds[name]) {
      refuse("IncompatibleAdapter", `physical Host target adapter does not match catalog bound ${name}`, generation, { target_id: entry.target.id });
    }
  }
}

function refuse(terminal, message, generation, extra = {}) {
  const evidence = Object.freeze({
    schema: FAILURE_SCHEMA,
    catalog_generation: generation,
    terminal,
    message,
    ...extra,
  });
  const error = new Error(message);
  error.name = "PhysicalHostTargetCatalogRefusal";
  error.code = terminal;
  error.evidence = evidence;
  throw error;
}

function jsonBytes(value, name, generation) {
  try {
    return encoder.encode(JSON.stringify(value)).length;
  } catch {
    refuse("MalformedCatalog", `physical Host target ${name} is not finite JSON`, generation);
  }
}

function boundedText(value, maximum) {
  return typeof value === "string" && value.length > 0 && value.length <= maximum;
}

function boundedInteger(value, minimum, maximum) {
  return Number.isSafeInteger(value) && value >= minimum && value <= maximum;
}
