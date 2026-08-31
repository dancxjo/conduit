import { initializeBrowserHost } from "./browser-host-bootstrap.mjs";
import { createBodyBirthRunner, createFirstHostRunner, readBodyProjection } from "./creche-lifecycle.mjs";
import { createPhysicalHostRunner } from "./creche-physical.mjs";
import { createPhysicalHostTargetCatalog } from "./creche-target-catalog.mjs";
import { createGraduationRunner, renderBiography } from "./creche-graduation.mjs";
import { RP2040_CRECHE_TARGET_CONTRIBUTION } from "./targets/rp2040/browser-deployment/creche-adapter.mjs";
import { ESP32_CRECHE_TARGET_CONTRIBUTIONS } from "./targets/esp32/browser-deployment/creche-adapter.mjs";
import { STD_EXISTING_COMPUTER_CONTRIBUTIONS } from "./targets/std/browser-deployment/creche-adapter.mjs";
import { BROWSER_EXISTING_COMPUTER_CONTRIBUTION } from "./targets/browser/browser-deployment/creche-adapter.mjs";

const MORSE_NETWORK = `form morse_network {
    message: text/literal("SOS")
    morse: text/morse(120)
    light: presentation/indicator
    message > morse > light
}`;
const steps = ["Birth", "First Host", "Physical Host", "Graduate"];
const workspace = document.querySelector("#workspace");
const navigation = document.querySelector(".creche-steps");
let host;
let currentStep = 0;
let sequence = 0;
const targetCatalog = createPhysicalHostTargetCatalog({
  generation: 1,
  contributions: [
    RP2040_CRECHE_TARGET_CONTRIBUTION,
    ...ESP32_CRECHE_TARGET_CONTRIBUTIONS,
    ...STD_EXISTING_COMPUTER_CONTRIBUTIONS,
    BROWSER_EXISTING_COMPUTER_CONTRIBUTION,
  ],
});

try {
  host = await initializeBrowserHost();
  requireCrecheAbi(host.runtime);
  document.querySelector("#host-state").textContent = "Crèche ready";
  globalThis.__conduitCrecheHost = host;
  renderNavigation();
  renderStep();
} catch (error) {
  document.querySelector("#host-state").textContent = "Crèche unavailable";
  workspace.textContent = error instanceof Error ? error.message : String(error);
}

function renderNavigation() {
  navigation.replaceChildren();
  steps.forEach((name, index) => {
    const button = document.createElement("button");
    button.type = "button";
    button.textContent = `${index + 1}. ${name}`;
    button.setAttribute("aria-current", index === currentStep ? "step" : "false");
    button.addEventListener("click", () => { currentStep = index; renderNavigation(); renderStep(); });
    navigation.append(button);
  });
}

function renderStep() {
  workspace.replaceChildren();
  const heading = document.createElement("h2");
  heading.textContent = steps[currentStep];
  workspace.append(heading);
  if (currentStep === 0) workspace.append(createBodyBirthRunner({
    source: MORSE_NETWORK, sourceKey: "standalone-creche", listingId: "creche-seed", host,
    nextSequence: () => ++sequence, onDraft: () => {}, onBodyChanged: refreshContext,
  }));
  if (currentStep === 1) workspace.append(createFirstHostRunner({
    host,
    nextSequence: () => { const admitted = ++sequence; sequence += 1; return admitted; },
    onBodyChanged: refreshContext,
  }));
  if (currentStep === 2) workspace.append(createPhysicalHostRunner({
    host,
    targetCatalog,
  }));
  if (currentStep === 3) workspace.append(createGraduationRunner({
    host, nextSequence: () => ++sequence, onBodyChanged: refreshContext,
    onEnd: renderComplete,
  }));
  refreshContext();
}

function refreshContext() {
  workspace.querySelector(".creche-body-context")?.remove();
  const body = readBodyProjection(host.runtime);
  if (!body) return;
  const context = document.createElement("aside");
  context.className = "creche-body-context";
  context.innerHTML = `<strong>${escapeText(body.friendly_name)}</strong><code>${escapeText(body.body_id)}</code>`;
  workspace.prepend(context);
}

function renderComplete(receipt, biography) {
  workspace.replaceChildren();
  const section = document.createElement("section");
  section.className = "creche-complete";
  section.innerHTML = `<h2>The Body continues</h2><p>The Crèche can close; durable Body evidence remains available to compatible readers.</p><code>${escapeText(receipt.body_id)}</code><section class="body-biography" aria-label="Body biography"><h3>Body biography · compatible reader</h3><ol></ol></section>`;
  renderBiography(section.querySelector(".body-biography"), biography);
  workspace.append(section);
  navigation.remove();
}

function requireCrecheAbi(api) {
  const required = ["memory", "conduit_creche_input_ptr", "conduit_creche_input_capacity", "conduit_creche_output_ptr", "conduit_creche_output_len", "conduit_creche_admit_source_interaction", "conduit_creche_birth", "conduit_creche_current", "conduit_creche_biography", "conduit_creche_attach_here", "conduit_creche_graduation_readiness", "conduit_creche_graduate", "conduit_creche_prepare_selected_physical_spore", "conduit_creche_prepare_selected_physical_spore_for_target", "conduit_creche_admit_physical_spore"];
  if (required.some((name) => !(name in api))) throw new Error("Crèche runtime ABI is incomplete");
}

function escapeText(value) {
  const span = document.createElement("span"); span.textContent = value; return span.innerHTML;
}
