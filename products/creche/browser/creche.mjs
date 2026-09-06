import { initializeBrowserHost } from "../../../targets/browser/host/assets/browser-host-bootstrap.mjs";
import { createBodyBirthRunner, createFirstHostRunner, readBodyProjection } from "./creche-lifecycle.mjs";
import { createPhysicalHostRunner } from "./creche-physical.mjs";
import { createPhysicalHostTargetCatalog } from "./creche-target-catalog.mjs";
import { createGraduationRunner, exportBodyEvidence, renderBiography } from "./creche-graduation.mjs";
import { createCrecheRouting } from "./creche-routing.mjs";
import { openFormSelection, persistedFormSelection, readReviewedFormInventory } from "./creche-form-selection.mjs";
import { createProductMasthead } from "../../../semantics/presentation/assets/product-masthead.mjs";
import { AVR_PRO_MICRO_CRECHE_TARGET_CONTRIBUTION } from "../../../targets/avr/deployment/browser/creche-adapter.mjs";
import { RP2040_CRECHE_TARGET_CONTRIBUTION } from "../../../targets/rp2040/deployment/browser/creche-adapter.mjs";
import { ESP32_CRECHE_TARGET_CONTRIBUTIONS } from "../../../targets/esp32/deployment/browser/creche-adapter.mjs";
import { STD_EXISTING_COMPUTER_CONTRIBUTIONS } from "../../../targets/std/deployment/browser/creche-adapter.mjs";
import { BROWSER_EXISTING_COMPUTER_CONTRIBUTION } from "../../../targets/browser/deployment/browser/creche-adapter.mjs";
import { ORANGE_PI_CRECHE_TARGET_CONTRIBUTION } from "../../../targets/orange-pi/deployment/browser/creche-adapter.mjs";
import { RASPBERRY_PI_CRECHE_TARGET_CONTRIBUTIONS } from "../../../targets/raspberry-pi/deployment/browser/creche-adapter.mjs";
import { CONDUITOS_CRECHE_TARGET_CONTRIBUTIONS } from "../../../targets/conduitos/deployment/browser/creche-adapter.mjs";

const steps = [
  { name: "Birth", slug: "birth" },
  { name: "First Host", slug: "first-host" },
  { name: "Physical Host", slug: "physical-host" },
  { name: "Graduate", slug: "graduate" },
];
const workspace = document.querySelector("#workspace");
let presentation;
let presentationFor;
let storage;
let host;
let routing;
let initialFormSource;
let reviewedFormInventory;
let initialFormSelection;
let selectionWrite = Promise.resolve();
let durableWrite = Promise.resolve();
let durabilityFailure = null;
let currentStep = 0;
let sequence = 0;
let presentationRevision = 0;
let productMasthead;
const targetCatalog = createPhysicalHostTargetCatalog({
  generation: 1,
  contributions: [
    RP2040_CRECHE_TARGET_CONTRIBUTION,
    AVR_PRO_MICRO_CRECHE_TARGET_CONTRIBUTION,
    ...ESP32_CRECHE_TARGET_CONTRIBUTIONS,
    ...STD_EXISTING_COMPUTER_CONTRIBUTIONS,
    BROWSER_EXISTING_COMPUTER_CONTRIBUTION,
    ORANGE_PI_CRECHE_TARGET_CONTRIBUTION,
    ...RASPBERRY_PI_CRECHE_TARGET_CONTRIBUTIONS,
    ...CONDUITOS_CRECHE_TARGET_CONTRIBUTIONS,
  ],
});

export async function startApplication(application) {
 try {
  presentation = application.presentation;
  presentationFor = application.presentationFor;
  productMasthead = createProductMasthead(presentation, "product-masthead", "creche");
  storage = application.storage;
  initialFormSource = application.text("reviewed-form-inventory");
  renderHostStatus("Starting browser Host…", "status");
  const initialized = await initializeBrowserHost({ runtimeBytes: application.bytes("runtime") });
  host = Object.freeze({ ...initialized, admitProfileGatedBrowserBoot: application.admitProfileGatedBrowserBoot });
  requireCrecheAbi(host.runtime);
  reviewedFormInventory = readReviewedFormInventory(host.runtime, initialFormSource);
  initialFormSelection = openFormSelection(reviewedFormInventory, await storage.readJson("form-selection"), galleryHandoff());
  routing = createCrecheRouting({
    host,
    applicationId: application.manifest.applicationId,
    steps,
    onPopState(index) { currentStep = index; renderNavigation(); renderStep(); },
    onFailure(error) { renderHostStatus(error instanceof Error ? error.message : String(error), "failure-status"); },
  });
  currentStep = routing.current();
  await restoreDurableBody();
  renderHostStatus("Crèche ready", "success-status");
  globalThis.__conduitCrecheHost = host;
  globalThis.__conduitCrecheDurability = Object.freeze({ settled: durabilitySettled });
  renderNavigation();
  renderStep();
  if (routing.isProductRoot()) await routing.move(currentStep, "replace");
 } catch (error) {
  renderHostStatus("Crèche unavailable", "failure-status");
  workspace.textContent = error instanceof Error ? error.message : String(error);
 }
}

function renderNavigation() {
  const actions = steps.map((_, index) => ({ id: `step-${index}`, event: "activate" }));
  const nodes = [{
    parent: null,
    component: "stepper",
    action: null,
    key: "workflow",
    text: "Birth and provision a Body",
    value: `${currentStep + 1}/${steps.length}`,
    valueCapacity: 11,
  }];
  steps.forEach(({ name }, index) => {
    nodes.push({ parent: 0, component: "button", action: index === currentStep ? null : index, key: `step-${index}`, text: `${index + 1}. ${name}` });
  });
  presentation.present("creche-navigation", { revision: ++presentationRevision, actions, nodes }, {
    onEvent(event) {
      const index = Number(event.action.slice("step-".length));
      if (Number.isInteger(index) && index >= 0 && index < steps.length) {
        void navigateToStep(index).catch((error) => renderHostStatus(
          error instanceof Error ? error.message : String(error),
          "failure-status",
        ));
      }
    },
  });
}

function renderHostStatus(text, component) {
  productMasthead.present(text, component);
}

function renderStep() {
  workspace.replaceChildren();
  const heading = document.createElement("h2");
  heading.textContent = steps[currentStep].name;
  workspace.append(heading);
  if (currentStep === 0) workspace.append(createBodyBirthRunner({
    source: initialFormSource, sourceKey: "standalone-creche", listingId: "creche-forms", host,
    presentationFor, inventory: reviewedFormInventory, initialSelection: initialFormSelection,
    onSelection: retainFormSelection, nextSequence: () => ++sequence, onBodyChanged: bodyChanged,
  }));
  if (currentStep === 1) workspace.append(createFirstHostRunner({
    host, presentationFor,
    nextSequence: () => { const admitted = ++sequence; sequence += 1; return admitted; },
    onBodyChanged: bodyChanged,
  }));
  if (currentStep === 2) workspace.append(createPhysicalHostRunner({
    host,
    presentationFor,
    hostOperations: routing,
    targetCatalog,
    onBodyChanged: bodyChanged,
  }));
  if (currentStep === 3) workspace.append(createGraduationRunner({
    host, presentationFor, nextSequence: () => ++sequence, onBodyChanged: bodyChanged,
    onEnd: renderComplete,
  }));
  refreshContext();
}

function galleryHandoff() {
  const parameters = new URLSearchParams(globalThis.location.search);
  const values = ["form", "source_document_id", "checked_form_id"].map((key) => parameters.get(key));
  if (values.every((value) => value === null)) return null;
  if (values.some((value) => value === null || value.length === 0)) {
    throw new Error("Gallery handoff must carry one complete exact Form identity");
  }
  return { name: values[0], source_document_id: values[1], checked_form_id: values[2] };
}

function retainFormSelection(selected) {
  initialFormSelection = selected === null
    ? openFormSelection(reviewedFormInventory)
    : Object.freeze({ selected: [...selected], refusals: [] });
  selectionWrite = selectionWrite.then(() => selected === null
    ? storage.deleteJson("form-selection")
    : storage.writeJson("form-selection", persistedFormSelection(reviewedFormInventory, selected)));
}

async function navigateToStep(index) {
  await routing.move(index, "push");
  currentStep = index;
  renderNavigation();
  renderStep();
}

function refreshContext() {
  workspace.querySelector('[data-creche-context="true"]')?.remove();
  const body = readBodyProjection(host.runtime);
  if (!body) return;
  const context = document.createElement("aside");
  context.className = "creche-body-context";
  context.dataset.crecheContext = "true";
  context.dataset.applicationSlot = "creche-body-context";
  workspace.prepend(context);
  presentation.present("creche-body-context", {
    revision: ++presentationRevision,
    actions: [],
    nodes: [
      { parent: null, component: "panel", action: null, key: "body-context", text: "" },
      { parent: 0, component: "paragraph", action: null, key: "body-name", text: body.friendly_name },
      { parent: 0, component: "code", action: null, key: "body-id", text: body.body_id },
    ],
  });
}

function bodyChanged() {
  refreshContext();
  retainDurableBody();
}

async function restoreDurableBody() {
  const snapshot = await storage.readJson("body-session");
  if (snapshot === null) return;
  const encoded = new TextEncoder().encode(JSON.stringify(snapshot));
  const api = host.runtime;
  if (encoded.length > api.conduit_creche_input_capacity()) {
    throw new Error("durable Crèche session exceeds the runtime restore bound");
  }
  new Uint8Array(api.memory.buffer, api.conduit_creche_input_ptr(), encoded.length).set(encoded);
  const code = api.conduit_creche_restore_durable(encoded.length);
  if (code < 0) {
    const refusal = readAbiOutput(api);
    throw new Error(refusal.message ?? `durable Crèche restore refused (${code})`);
  }
  const restored = readAbiOutput(api);
  sequence = Math.max(
    restored.birth_sequence,
    ...(snapshot.biography?.records ?? []).map((record) => record.sequence),
  );
  if (restored.here_part_id) {
    const reconciled = changeHereMembership("conduit_creche_attach_here", sequence + 1);
    sequence += 2;
    if (reconciled.host_id !== host.hostId || reconciled.boot_id !== host.bootId) {
      throw new Error("durable browser Host membership did not reconcile to the current Boot");
    }
    retainDurableBody();
  }
}

function changeHereMembership(operation, eventSequence = ++sequence) {
  const api = host.runtime;
  const hostBytes = new TextEncoder().encode(host.hostId);
  const bootBytes = new TextEncoder().encode(host.bootId);
  const input = new Uint8Array(api.memory.buffer, api.conduit_creche_input_ptr(), hostBytes.length + bootBytes.length);
  input.set(hostBytes);
  input.set(bootBytes, hostBytes.length);
  const code = api[operation](hostBytes.length, bootBytes.length, BigInt(eventSequence));
  if (code < 0) {
    const refusal = readAbiOutput(api);
    throw new Error(refusal.message ?? `browser membership operation refused (${code})`);
  }
  return readAbiOutput(api);
}

function retainDurableBody() {
  const api = host.runtime;
  const code = api.conduit_creche_durable_snapshot();
  if (code === 1) return;
  if (code < 0) throw new Error(`durable Crèche snapshot refused (${code})`);
  const snapshot = readAbiOutput(api);
  durableWrite = durableWrite
    .then(() => storage.writeJson("body-session", snapshot))
    .then(() => { durabilityFailure = null; })
    .catch((error) => {
      durabilityFailure = error instanceof Error ? error : new Error(String(error));
      renderHostStatus("Crèche storage refused", "failure-status");
    });
}

async function durabilitySettled() {
  await Promise.all([durableWrite, selectionWrite]);
  if (durabilityFailure) throw durabilityFailure;
}

function readAbiOutput(api) {
  const bytes = new Uint8Array(api.memory.buffer, api.conduit_creche_output_ptr(), api.conduit_creche_output_len());
  return JSON.parse(new TextDecoder().decode(bytes));
}

function renderComplete(receipt, biography) {
  workspace.replaceChildren();
  const section = document.createElement("section");
  section.className = "creche-complete";
  const evidenceFilename = `conduit-body-${receipt.body_id}.json`;
  const patchbayCommand = `conduit patchbay --on browser --body-evidence ${evidenceFilename}`;
  section.innerHTML = `<h2>The Body continues</h2><p>The Crèche can close; durable Body evidence remains available to compatible readers.</p><code>${escapeText(receipt.body_id)}</code><div data-application-slot="creche-complete-actions"></div><aside class="creche-handoff" aria-labelledby="creche-handoff-title"><h3 id="creche-handoff-title">Continue in Patchbay</h3><p>Save the evidence, then open that exact Body through the Conduit product entrance:</p><code>${escapeText(patchbayCommand)}</code></aside><section class="body-biography" data-application-slot="creche-complete-biography"></section>`;
  workspace.append(section);
  renderBiography(presentation, "creche-complete-biography", biography, ++presentationRevision);
  presentation.present("creche-complete-actions", {
    revision: ++presentationRevision,
    actions: [
      { id: "evidence.save", event: "activate" },
      { id: "membership.leave", event: "activate" },
      { id: "membership.revoke", event: "activate" },
      { id: "creche.finish", event: "activate" },
    ],
    nodes: [
      { parent: null, component: "action-group", action: null, key: "complete-actions", text: "Crèche completion actions" },
      { parent: 0, component: "button", action: 0, key: "save-body-evidence", text: "Save Body evidence" },
      { parent: 0, component: "button", action: 1, key: "leave-body", text: "Leave Body" },
      { parent: 0, component: "button", action: 2, key: "remove-browser", text: "Remove this browser from the Body" },
      { parent: 0, component: "button", action: 3, key: "finish-creche", text: "Finish and clear Crèche" },
    ],
  }, { async onEvent(event) {
    presentation.nextEvent("creche-complete-actions");
    if (event.action === "evidence.save") exportBodyEvidence(biography);
    if (event.action === "membership.leave") {
      changeHereMembership("conduit_creche_leave_here");
      retainDurableBody();
      await durabilitySettled();
      renderComplete(readBodyProjection(host.runtime), renderBiographyEvidence());
    }
    if (event.action === "membership.revoke") {
      changeHereMembership("conduit_creche_revoke_here");
      retainDurableBody();
      await durabilitySettled();
      renderComplete(readBodyProjection(host.runtime), renderBiographyEvidence());
    }
    if (event.action === "creche.finish") await finishCrecheLocally();
  } });
  document.querySelector('[data-application-slot="creche-navigation"]')?.remove();
}

function renderBiographyEvidence() {
  const api = host.runtime;
  const code = api.conduit_creche_biography();
  if (code < 0) throw new Error(`Body biography unavailable (${code})`);
  return readAbiOutput(api);
}

async function finishCrecheLocally() {
  await storage.deleteJson("body-session");
  host.runtime.conduit_creche_forget_local();
  sequence = 0;
  currentStep = 0;
  await routing.move(0, "replace");
  if (!document.querySelector('[data-application-slot="creche-navigation"]')) {
    const navigation = document.createElement("div");
    navigation.className = "creche-steps";
    navigation.dataset.applicationSlot = "creche-navigation";
    workspace.before(navigation);
  }
  renderNavigation();
  renderStep();
}

function requireCrecheAbi(api) {
  const required = ["memory", "conduit_syntax_input_ptr", "conduit_syntax_input_capacity", "conduit_syntax_output_ptr", "conduit_syntax_output_len", "conduit_syntax_project", "conduit_creche_input_ptr", "conduit_creche_input_capacity", "conduit_creche_output_ptr", "conduit_creche_output_len", "conduit_creche_reviewed_inventory", "conduit_creche_review_initial_workload", "conduit_creche_admit_source_interaction", "conduit_creche_birth", "conduit_creche_current", "conduit_creche_biography", "conduit_creche_durable_snapshot", "conduit_creche_restore_durable", "conduit_creche_attach_here", "conduit_creche_leave_here", "conduit_creche_revoke_here", "conduit_creche_forget_local", "conduit_creche_graduation_readiness", "conduit_creche_graduate", "conduit_creche_prepare_selected_physical_spore", "conduit_creche_prepare_selected_physical_spore_for_target", "conduit_creche_browser_configuration_catalog", "conduit_creche_review_browser_configuration", "conduit_creche_prepare_selected_browser_spore", "conduit_creche_admit_physical_spore"];
  if (required.some((name) => !(name in api))) throw new Error("Crèche runtime ABI is incomplete");
}

function escapeText(value) {
  const span = document.createElement("span"); span.textContent = value; return span.innerHTML;
}
