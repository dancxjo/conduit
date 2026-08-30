import { createBrowserDeviceBase } from "../device-base.mjs";
import { createBrowserUsbDeviceBase } from "../usb-device-base.mjs";
import {
  createRp2040BrowserDeploymentAdapter,
  requestRp2040SpawnJoin,
  RP2040_BROWSER_DEPLOYMENT,
} from "../targets/rp2040/browser-deployment/index.mjs";

const MAX_IMAGE_BYTES = 2 * 1024 * 1024;
const encoder = new TextEncoder();
const decoder = new TextDecoder();

export function createPhysicalHostRunner({ host }) {
  const runner = document.createElement("section");
  runner.className = "physical-host-runner";
  runner.innerHTML = `
    <ol class="physical-stages" aria-label="Add one physical Host">
      <li data-stage="image"><strong>1 · Select IMAGE</strong><span>waiting</span></li>
      <li data-stage="spore"><strong>2 · Create spore</strong><span>waiting</span></li>
      <li data-stage="deploy"><strong>3 · Deploy</strong><span>waiting</span></li>
      <li data-stage="boot"><strong>4 · Observe Boot + join</strong><span>waiting</span></li>
      <li data-stage="admit"><strong>5 · Admit Part + offers</strong><span>waiting</span></li>
    </ol>
    <div class="physical-actions">
      <label class="image-picker">Choose reviewed Pico W UF2<input type="file" accept=".uf2,application/octet-stream"></label>
      <button class="prepare" type="button" disabled>Prepare Body spore</button>
      <button class="deploy" type="button" disabled>Connect BOOTSEL and deploy</button>
      <button class="observe" type="button" disabled>Connect running Pico and observe join</button>
      <button class="admit" type="button" disabled>Admit physical Part</button>
    </div>
    <p class="physical-status" role="status">Choose one exact prebuilt Pico W UF2. No device permission is requested yet.</p>
    <details><summary>Exact B7 evidence</summary><pre><code></code></pre></details>`;

  const state = {
    imageBytes: null,
    imageDigest: null,
    prepared: null,
    deployment: null,
    observation: null,
    admission: null,
  };
  const input = runner.querySelector("input");
  input.addEventListener("change", () => selectImage(runner, state, input.files?.[0]));
  runner.querySelector(".prepare").addEventListener("click", () => prepare(runner, host, state));
  runner.querySelector(".deploy").addEventListener("click", () => deploy(runner, host, state));
  runner.querySelector(".observe").addEventListener("click", () => observe(runner, host, state));
  runner.querySelector(".admit").addEventListener("click", () => admit(runner, host, state));
  return runner;
}

async function selectImage(runner, state, file) {
  if (!file) return;
  try {
    if (file.size === 0 || file.size > MAX_IMAGE_BYTES) {
      throw new Error(`IMAGE must contain 1–${MAX_IMAGE_BYTES} bytes`);
    }
    const bytes = new Uint8Array(await file.arrayBuffer());
    const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", bytes));
    state.imageBytes = bytes;
    state.imageDigest = `sha256:${Array.from(digest, (byte) => byte.toString(16).padStart(2, "0")).join("")}`;
    completeStage(runner, "image", `${file.name} · ${bytes.length} bytes`);
    runner.querySelector(".prepare").disabled = false;
    status(runner, "IMAGE selected and hashed. No spore, deployment, Boot, or membership exists.");
    renderEvidence(runner, state);
  } catch (error) {
    refuse(runner, "IMAGE selection", error);
  }
}

function prepare(runner, host, state) {
  try {
    const current = currentBody(host.runtime);
    if (!current) throw new Error("Birth the Body in Step 11 before preparing a physical Host");
    if (!state.imageBytes || !state.imageDigest) throw new Error("select one exact UF2 first");
    const entropy = crypto.getRandomValues(new Uint8Array(32));
    const digest = encoder.encode(state.imageDigest);
    const input = new Uint8Array(
      host.runtime.memory.buffer,
      host.runtime.conduit_book_body_input_ptr(),
      entropy.length + digest.length,
    );
    input.set(entropy);
    input.set(digest, entropy.length);
    const code = host.runtime.conduit_book_body_prepare_selected_physical_spore(
      digest.length,
      BigInt(Date.now()),
    );
    entropy.fill(0);
    if (code < 0) throw outputError(host.runtime, "spore preparation", code);
    state.prepared = readOutput(host.runtime);
    if (state.prepared.image_content_digest !== state.imageDigest) {
      throw new Error("prepared spore lost the selected IMAGE content identity");
    }
    completeStage(runner, "spore", short(state.prepared.spore_id));
    runner.querySelector(".prepare").disabled = true;
    runner.querySelector(".deploy").disabled = false;
    status(runner, "Spore prepared. Deployment, Boot, join, membership, offers, Plan, and Play remain absent.");
    renderEvidence(runner, state);
  } catch (error) {
    refuse(runner, "spore preparation", error);
  }
}

async function deploy(runner, host, state) {
  const button = runner.querySelector(".deploy");
  button.disabled = true;
  let adapter = null;
  try {
    const usb = createBrowserUsbDeviceBase({
      api: host.runtime,
      hostId: host.hostId,
      bootId: host.bootId,
      status: null,
      output: null,
    });
    const base = await usb.acquireUsb({
      configurationValue: RP2040_BROWSER_DEPLOYMENT.configurationValue,
      interfaceNumber: RP2040_BROWSER_DEPLOYMENT.interfaceNumber,
      alternateSetting: RP2040_BROWSER_DEPLOYMENT.alternateSetting,
      inEndpoint: RP2040_BROWSER_DEPLOYMENT.inEndpoint,
      outEndpoint: RP2040_BROWSER_DEPLOYMENT.outEndpoint,
      maximumTransferBytes: RP2040_BROWSER_DEPLOYMENT.maximumTransferBytes,
      maximumInTransfers: 2048,
      maximumOutTransfers: 2048,
    });
    adapter = createRp2040BrowserDeploymentAdapter({ base });
    const plan = await adapter.sealDeployment({
      deploymentPlanId: `deployment-plan/${state.prepared.spore_id}`,
      deploymentOperationId: `deployment/${state.prepared.spore_id}`,
      targetId: state.prepared.target_id,
      sporeId: state.prepared.spore_id,
      imageId: state.prepared.image_id,
      imageContentId: state.prepared.image_content_digest,
      imageBytes: state.imageBytes,
      explicitAction: true,
    });
    state.deployment = await adapter.deploy(plan);
    completeStage(runner, "deploy", state.deployment.terminal);
    runner.querySelector(".observe").disabled = false;
    status(runner, "Deployment requested reboot. That proves no Boot, join, membership, offers, readiness, Plan, or Play.");
    renderEvidence(runner, state);
  } catch (error) {
    if (adapter) {
      state.deployment = {
        ...adapter.evidence(),
        failure_chain: errorChain(error),
      };
      renderEvidence(runner, state);
    }
    const permissionHint = /access denied/i.test(error?.message ?? "")
      ? " On Linux, run `sudo scripts/install-pico-headless-flash.sh` and reconnect the Pico in BOOTSEL mode."
      : "";
    const freshHostHint = " This USB acquisition is terminal; reload the Tour to create a fresh browser Host before trying again.";
    refuse(runner, "deployment", new Error(`${errorChain(error).join(" <- ")}${permissionHint}${freshHostHint}`));
  }
}

async function observe(runner, host, state) {
  const button = runner.querySelector(".observe");
  button.disabled = true;
  try {
    const devices = createBrowserDeviceBase({
      api: host.runtime,
      hostId: host.hostId,
      bootId: host.bootId,
      status: null,
      output: null,
    });
    const base = await devices.acquireSerial({
      maximumTransferBytes: 4096,
      maximumReads: 2,
      maximumWrites: 1,
    });
    state.observation = await requestRp2040SpawnJoin({ base, prepared: state.prepared });
    completeStage(runner, "boot", short(state.observation.boot_id));
    runner.querySelector(".admit").disabled = false;
    status(runner, "Fresh Boot advertisement and invitation-bound join observed. Admission remains an explicit action.");
    renderEvidence(runner, state);
  } catch (error) {
    button.disabled = false;
    refuse(runner, "Boot/join observation", error);
  }
}

function admit(runner, host, state) {
  try {
    const observation = state.observation;
    const encoded = encoder.encode(JSON.stringify({
      spore_id: observation.spore_id,
      image_id: observation.image_id,
      advertisement: observation.advertisement,
      invitation_id: observation.invitation_id,
      body_id: observation.body_id,
      host_id: observation.host_id,
      boot_id: observation.boot_id,
      nonce: observation.nonce,
      signature: observation.signature,
      observed_at_millis: observation.observed_at_millis,
    }));
    if (encoded.length > host.runtime.conduit_book_body_input_capacity()) {
      throw new Error("join observation exceeds the admitted Body input bound");
    }
    new Uint8Array(
      host.runtime.memory.buffer,
      host.runtime.conduit_book_body_input_ptr(),
      encoded.length,
    ).set(encoded);
    const code = host.runtime.conduit_book_body_admit_physical_spore(encoded.length);
    if (code < 0) throw outputError(host.runtime, "Part admission", code);
    state.admission = readOutput(host.runtime);
    completeStage(runner, "admit", `revision ${state.admission.membership_revision}`);
    runner.querySelector(".admit").disabled = true;
    status(runner, `Physical Part admitted; ${state.admission.offer_count} current offers are ready. No Plan or Play was created.`);
    renderEvidence(runner, state);
  } catch (error) {
    refuse(runner, "Part admission", error);
  }
}

function currentBody(api) {
  const code = api.conduit_book_body_current();
  if (code === 1) return null;
  if (code < 0) throw outputError(api, "Body projection", code);
  return readOutput(api);
}

function readOutput(api) {
  return JSON.parse(decoder.decode(new Uint8Array(
    api.memory.buffer,
    api.conduit_book_body_output_ptr(),
    api.conduit_book_body_output_len(),
  )));
}

function outputError(api, operation, code) {
  const evidence = api.conduit_book_body_output_len() > 0 ? readOutput(api) : null;
  return new Error(evidence?.message ?? `${operation} refused (${code})`);
}

function completeStage(runner, name, value) {
  const stage = runner.querySelector(`[data-stage="${name}"]`);
  stage.classList.add("complete");
  stage.querySelector("span").textContent = value;
}

function status(runner, message) {
  const element = runner.querySelector(".physical-status");
  element.classList.remove("error");
  element.textContent = message;
}

function refuse(runner, operation, error) {
  const element = runner.querySelector(".physical-status");
  element.classList.add("error");
  element.textContent = `${operation} refused: ${error instanceof Error ? error.message : String(error)}`;
}

function errorChain(error) {
  const messages = [];
  for (let current = error; current && messages.length < 4; current = current.cause) {
    const code = typeof current.code === "string" ? `${current.code}: ` : "";
    messages.push(`${code}${current instanceof Error ? current.message : String(current)}`);
  }
  return messages;
}

function short(value) {
  return value.length > 24 ? `${value.slice(0, 21)}…` : value;
}

function renderEvidence(runner, state) {
  const prepared = state.prepared ? { ...state.prepared, invitation_secret: "redacted" } : null;
  runner.querySelector("details code").textContent = JSON.stringify({
    image: state.imageDigest ? { content_digest: state.imageDigest, bytes: state.imageBytes.length } : null,
    prepared,
    deployment: state.deployment,
    observation: state.observation,
    admission: state.admission,
  }, null, 2);
}
