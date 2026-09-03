import { initializeBrowserHost } from "./browser-host-membership.mjs";
import { joinBrowserBody } from "./browser-body-membership.mjs";
import { mountProductNavigation } from "./product-navigation.mjs";

const encoder = new TextEncoder();
const decoder = new TextDecoder("utf-8", { fatal: true });
const MAXIMUM_EVIDENCE_BYTES = 32 * 1024;
const MAXIMUM_URL_BYTES = 2048;
let application;
let host;
let entranceApi;
let membershipSession;
let membershipRuntimeBytes;
let relationship = "none";
let evidenceUrl = "";
let bodyUrl = "";
let planId = "";
let implementationId = "";
let revision = 0;

export async function startApplication(context) {
  application = context;
  mountProductNavigation();
  presentHost("Starting browser Host…", "status");
  const runtimeBytes = context.bytes("runtime");
  membershipRuntimeBytes = runtimeBytes.slice();
  host = await initializeBrowserHost(runtimeBytes);
  entranceApi = (await WebAssembly.instantiate(context.bytes("patchbay-entrance-runtime"), {})).instance.exports;
  requireEntranceAbi(entranceApi);
  presentHost("Browser Host ready", "success-status");
  readEntranceParameters();
  renderEntrance();
  renderEmptyWorkbench();
  globalThis.__conduitPatchbay = Object.freeze({
    host,
    relationship: () => relationship,
    membership: () => membershipSession,
    openExternal: () => openExternal(),
    joinHosted: () => joinHosted(),
  });
  if (evidenceUrl && new URL(location.href).searchParams.get("mode") === "external") await openExternal();
  if (evidenceUrl && bodyUrl && new URL(location.href).searchParams.get("mode") === "hosted") await joinHosted();
}

function readEntranceParameters() {
  const parameters = new URL(location.href).searchParams;
  evidenceUrl = boundedParameter(parameters.get("evidence") ?? "", "evidence URL");
  bodyUrl = boundedParameter(parameters.get("body") ?? "", "Body URL");
  planId = boundedParameter(parameters.get("plan") ?? "", "Patchbay Plan identity", 512);
  implementationId = boundedParameter(parameters.get("implementation") ?? "", "Patchbay implementation identity", 512);
}

function boundedParameter(value, label, maximum = MAXIMUM_URL_BYTES) {
  if (encoder.encode(value).length > maximum || /[\0\n]/.test(value)) throw new Error(`${label} exceeds its finite bound`);
  return value;
}

function presentHost(text, component) {
  application.presentation.present("patchbay-host-status", {
    revision: ++revision,
    actions: [],
    nodes: [{ parent: null, component, action: null, key: "host", text }],
  });
}

function renderEntrance(feedback = "Opening Patchbay does not create or join a Body.", outcome = "status") {
  const actions = [
    { id: "evidence", event: "input" },
    { id: "body", event: "input" },
    { id: "open-external", event: "activate" },
    { id: "join-hosted", event: "activate" },
    { id: "clear", event: "activate" },
  ];
  application.presentation.present("patchbay-entrance", {
    revision: ++revision,
    actions,
    nodes: [
      { parent: null, component: "stack", action: null, key: "entrance", text: "" },
      { parent: 0, component: "paragraph", action: null, key: "explanation", text: "Open bounded authoritative Body evidence externally, or explicitly join this browser Host when a Body admission endpoint is available." },
      { parent: 0, component: "paragraph", action: null, key: "evidence-label", text: "Body evidence URL" },
      { parent: 0, component: "text-input", action: 0, key: "evidence", text: "Body evidence URL", value: evidenceUrl, valueCapacity: MAXIMUM_URL_BYTES },
      { parent: 0, component: "paragraph", action: null, key: "body-label", text: "Body admission URL" },
      { parent: 0, component: "text-input", action: 1, key: "body", text: "Body admission URL", value: bodyUrl, valueCapacity: MAXIMUM_URL_BYTES },
      { parent: 0, component: "action-group", action: null, key: "actions", text: "" },
      { parent: 6, component: "button", action: 2, key: "open-external", text: "Inspect externally" },
      { parent: 6, component: "button", action: 3, key: "join-hosted", text: "Join this browser to this Body" },
      { parent: 6, component: "button", action: 4, key: "clear", text: "Return to no Body" },
      { parent: 0, component: outcome, action: null, key: "feedback", text: feedback },
    ],
  }, { onEvent(event) {
    application.presentation.nextEvent("patchbay-entrance");
    if (event.action === "evidence") evidenceUrl = boundedParameter(event.value, "evidence URL");
    if (event.action === "body") bodyUrl = boundedParameter(event.value, "Body URL");
    if (event.action === "open-external") void openExternal().catch(showFailure);
    if (event.action === "join-hosted") void joinHosted().catch(showFailure);
    if (event.action === "clear") clearBody();
  } });
  renderRelationship(feedback, outcome);
}

function renderRelationship(text, component = "status") {
  const label = relationship === "hosted"
    ? "Hosted Patchbay · this browser Host is a current member"
    : relationship === "external"
      ? "External Patchbay · this browser Host is not part of the viewed Body"
      : "No Body open · Patchbay remains available";
  application.presentation.present("patchbay-relationship", {
    revision: ++revision,
    actions: [],
    nodes: [
      { parent: null, component: "stack", action: null, key: "relationship", text: "" },
      { parent: 0, component, action: null, key: "relationship-status", text: label },
      { parent: 0, component: "paragraph", action: null, key: "detail", text },
    ],
  });
}

async function fetchEvidence() {
  if (!evidenceUrl) throw new Error("Enter a Body evidence URL first");
  const url = new URL(evidenceUrl, location.href);
  if (!new Set(["http:", "https:"]).has(url.protocol)) throw new Error("Body evidence URL must use HTTP or HTTPS");
  const response = await fetch(url, { cache: "no-store", credentials: "omit" });
  if (!response.ok) throw new Error(`Body evidence unavailable (${response.status})`);
  const declared = Number(response.headers.get("content-length"));
  if (Number.isFinite(declared) && declared > MAXIMUM_EVIDENCE_BYTES) throw new Error("Body evidence exceeds its admitted bound");
  const bytes = new Uint8Array(await response.arrayBuffer());
  if (bytes.length === 0 || bytes.length > MAXIMUM_EVIDENCE_BYTES) throw new Error("Body evidence exceeds its admitted bound");
  return bytes;
}

async function openExternal() {
  membershipSession?.close();
  membershipSession = null;
  const projection = openProjection(1, await fetchEvidence());
  relationship = "external";
  renderProjection(projection);
  renderEntrance("Authoritative evidence is open for external reading; membership was not changed.", "success-status");
  return projection;
}

async function joinHosted() {
  if (!bodyUrl) throw new Error("Enter a Body admission URL before joining");
  if (!planId || !implementationId) throw new Error("Hosted Patchbay requires exact Plan and implementation identities");
  if (membershipSession) membershipSession.close();
  renderEntrance("Requesting ordinary browser Host admission…", "warning-status");
  membershipSession = await joinBrowserBody({
    bodyUrl: boundedParameter(bodyUrl, "Body URL"),
    wasmBytes: membershipRuntimeBytes,
    host,
    renewPresence: false,
    reconnectPresence: false,
    onState(next) {
      renderEntrance(`Membership state: ${next}`, next.startsWith("refused:") ? "failure-status" : "warning-status");
      if (next === "admitted") void confirmHosted().catch(showFailure);
    },
  });
  return membershipSession;
}

async function confirmHosted() {
  const projection = openProjection(2, await fetchEvidence());
  if (membershipSession.bodyId() !== projection.body_id) {
    throw new Error("joined Body and opened evidence name different identities");
  }
  relationship = "hosted";
  renderProjection(projection);
  renderEntrance("Current Host/Boot membership and the exact Patchbay placement are validated.", "success-status");
  return projection;
}

function openProjection(mode, evidence) {
  const values = mode === 2 ? [host.hostId, host.bootId, planId, implementationId] : ["", "", "", ""];
  const encoded = values.map((value) => encoder.encode(value));
  const total = encoded.reduce((sum, value) => sum + value.length, evidence.length);
  if (total > entranceApi.conduit_patchbay_entrance_input_capacity()) throw new Error("Patchbay entrance input exceeds its admitted bound");
  const inputPointer = entranceApi.conduit_patchbay_entrance_input_ptr();
  const input = new Uint8Array(entranceApi.memory.buffer, inputPointer, total);
  let offset = 0;
  for (const value of [...encoded, evidence]) { input.set(value, offset); offset += value.length; }
  const status = entranceApi.conduit_patchbay_open_body(mode, ...encoded.map((value) => value.length), evidence.length);
  new Uint8Array(entranceApi.memory.buffer, inputPointer, total).fill(0);
  if (status < 0) throw new Error(status === -703 ? "current browser Host/Boot membership is absent or stale" : `Body evidence refused (${status})`);
  const outputPointer = entranceApi.conduit_patchbay_entrance_output_ptr();
  const outputLength = entranceApi.conduit_patchbay_entrance_output_len();
  return JSON.parse(decoder.decode(new Uint8Array(entranceApi.memory.buffer, outputPointer, outputLength)));
}

function renderProjection(projection) {
  const definitions = [
    ["Body", projection.body_id],
    ["Relationship", projection.relationship],
    ["Membership revision", String(projection.membership_revision)],
    ["Current Host", projection.current_host_id ?? "not a member"],
    ["Current Boot", projection.current_boot_id ?? "not a member"],
  ];
  application.presentation.present("patchbay-workbench", {
    revision: ++revision,
    actions: [],
    nodes: [
      { parent: null, component: "artifact", action: null, key: "body", text: projection.friendly_name },
      { parent: 0, component: "definition-table", action: null, key: "identity", text: "Exact Body relationship" },
      ...definitions.map(([text, value], index) => ({ parent: 1, component: "definition", action: null, key: `identity-${index}`, text, value, valueCapacity: 512 })),
      { parent: 0, component: "definition-table", action: null, key: "history", text: "Body history and evidence" },
      ...projection.entries.map((entry, index) => ({ parent: definitions.length + 2, component: "definition", action: null, key: `history-${index}`, text: entry.heading, value: `${entry.explanation} sequence ${entry.sequence} · Sign ${entry.evidence_sign_id}`, valueCapacity: 1024 })),
    ],
  });
}

function renderEmptyWorkbench() {
  application.presentation.present("patchbay-workbench", {
    revision: ++revision,
    actions: [],
    nodes: [{ parent: null, component: "missing-evidence", action: null, key: "empty", text: "No Body evidence is open. Patchbay has not created, joined, or inferred one." }],
  });
  renderRelationship("Choose an explicit external or hosted relationship when authoritative Body evidence is available.");
}

function clearBody() {
  membershipSession?.close();
  membershipSession = null;
  relationship = "none";
  history.replaceState({}, "", location.pathname);
  renderEmptyWorkbench();
  renderEntrance();
}

function showFailure(error) {
  renderEntrance(error instanceof Error ? error.message : String(error), "failure-status");
}

function requireEntranceAbi(api) {
  const required = ["memory", "conduit_patchbay_entrance_input_ptr", "conduit_patchbay_entrance_input_capacity", "conduit_patchbay_entrance_output_ptr", "conduit_patchbay_entrance_output_len", "conduit_patchbay_open_body"];
  if (required.some((name) => !(name in api))) throw new Error("Patchbay entrance runtime ABI is incomplete");
}
