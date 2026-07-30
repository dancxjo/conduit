import init, {
  explain_panel,
  parse_panel,
  patchbay_move_node,
  patchbay_replace_source,
} from "./conduit_web.js";

const source = document.querySelector("#source");
const result = document.querySelector("#result");
const runButton = document.querySelector("#run");
const stopButton = document.querySelector("#stop");
const undoResetButton = document.querySelector("#undo-reset");
const consoleBadge = document.querySelector("#console-status-badge");
const selectedNodeLabel = document.querySelector("#selected-node-label");
const moveLeftBtn = document.querySelector("#move-left");
const moveRightBtn = document.querySelector("#move-right");

const lessons = await (await fetch("../lessons/v1.json", { cache: "no-store" })).json();
const browserPlan = await (await fetch("./browser-plan.json", { cache: "no-store" })).json();

// Standard Library Reference Panels definition
const referencePanels = [
  {
    id: "file-copier",
    title: "File Copier Pipeline",
    objective: "Bounded file stream copy with pressure control",
    prose: "Transfers data from source file to destination file through a bounded 8-value cord.",
    source: `panel 1\n\nnode reader : conduit/file-read {\n    path = "source.txt"\n}\n\nnode writer : conduit/file-write {\n    path = "destination.txt"\n}\n\ncord reader.out -> writer.in {\n    capacity = 8\n    max_value_bytes = 65536\n    max_queued_bytes = 524288\n    low_watermark = 2\n    high_watermark = 8\n    pressure = block\n}`
  },
  {
    id: "dir-watcher",
    title: "Directory Watcher & Filter",
    objective: "Stream directory inspection, uppercase filter, and logging",
    prose: "Watches a directory stream, passes through a filter, converts to uppercase, and writes to log.",
    source: `panel 1\n\nnode watcher : conduit/file-read {\n    path = "watch_directory"\n}\n\nnode filter : conduit/passthrough\nnode processor : conduit/uppercase\nnode logger : conduit/log\n\ncord watcher.out -> filter.in {\n    capacity = 16\n    max_value_bytes = 4096\n    max_queued_bytes = 65536\n    low_watermark = 2\n    high_watermark = 16\n    pressure = block\n}\n\ncord filter.out -> processor.in {\n    capacity = 16\n    max_value_bytes = 4096\n    max_queued_bytes = 65536\n    low_watermark = 2\n    high_watermark = 16\n    pressure = block\n}\n\ncord processor.out -> logger.in {\n    capacity = 16\n    max_value_bytes = 4096\n    max_queued_bytes = 65536\n    low_watermark = 2\n    high_watermark = 16\n    pressure = block\n}`
  },
  {
    id: "http-webhook-relay",
    title: "HTTP Webhook Relay",
    objective: "Ingest HTTP webhooks and forward via HTTP client",
    prose: "Listens on port 8080 and relays incoming payloads to a remote endpoint.",
    source: `panel 1\n\nnode receiver : conduit/http-server {\n    port = 8080\n}\n\nnode filter : conduit/passthrough\nnode forwarding_client : conduit/http-client {\n    endpoint = "https://hooks.example.com/relay"\n}\n\ncord receiver.out -> filter.in {\n    capacity = 16\n    max_value_bytes = 32768\n    max_queued_bytes = 524288\n    low_watermark = 4\n    high_watermark = 16\n    pressure = block\n}\n\ncord filter.out -> forwarding_client.in {\n    capacity = 16\n    max_value_bytes = 32768\n    max_queued_bytes = 524288\n    low_watermark = 4\n    high_watermark = 16\n    pressure = block\n}`
  },
  {
    id: "ollama-text",
    title: "Local Ollama AI Generation",
    objective: "Stream text generation request to local Ollama LLM endpoint",
    prose: "Sends a JSON prompt to local Ollama HTTP server without cloud fallback or hidden state.",
    source: `panel 1\n\nnode prompt : conduit/literal {\n    value = "{\\"model\\": \\"llama3\\", \\"prompt\\": \\"Summarize Conduit architecture.\\", \\"stream\\": false}"\n}\n\nnode ollama_api : conduit/http-client {\n    endpoint = "http://127.0.0.1:11434/api/generate"\n}\n\nnode output : conduit/stdout\n\ncord prompt.out -> ollama_api.in {\n    capacity = 4\n    max_value_bytes = 8192\n    max_queued_bytes = 32768\n    low_watermark = 1\n    high_watermark = 4\n    pressure = block\n}\n\ncord ollama_api.out -> output.in {\n    capacity = 4\n    max_value_bytes = 65536\n    max_queued_bytes = 262144\n    low_watermark = 1\n    high_watermark = 4\n    pressure = block\n}`
  },
  {
    id: "network-health",
    title: "Network Health & Circuit Breaker",
    objective: "UDP socket health monitoring with backoff retry and circuit breaker",
    prose: "Probes DNS port 53, evaluates health gate, trips circuit breaker if unreachable, and applies backoff.",
    source: `panel 1\n\nnode probe_ping : conduit/udp-socket {\n    port = 53\n}\n\nnode health_check : conduit/health-gate\nnode breaker : conduit/circuit-breaker\nnode backoff_retry : conduit/backoff\n\ncord probe_ping.out -> health_check.in {\n    capacity = 8\n    max_value_bytes = 2048\n    max_queued_bytes = 16384\n    low_watermark = 2\n    high_watermark = 8\n    pressure = block\n}\n\ncord health_check.out -> breaker.in {\n    capacity = 8\n    max_value_bytes = 2048\n    max_queued_bytes = 16384\n    low_watermark = 2\n    high_watermark = 8\n    pressure = block\n}\n\ncord breaker.out -> backoff_retry.in {\n    capacity = 8\n    max_value_bytes = 2048\n    max_queued_bytes = 16384\n    low_watermark = 2\n    high_watermark = 8\n    pressure = block\n}`
  },
  {
    id: "wifi-station-join",
    title: "Wi-Fi Station Profile",
    objective: "Witness Wi-Fi Station join contract without background scanning",
    prose: "Resolves Wi-Fi station capability requirement on Linux or Pico W host.",
    source: `panel 1\n\nnode wifi_sta : conduit/wifi-station {\n    ssid = "ConduitNet"\n}\n\nnode status_logger : conduit/log\n\ncord wifi_sta.out -> status_logger.in {\n    capacity = 4\n    max_value_bytes = 1024\n    max_queued_bytes = 4096\n    low_watermark = 1\n    high_watermark = 4\n    pressure = block\n}`
  }
];

async function fetchArtifact(artifact) {
  const url = new URL(artifact.path, import.meta.url);
  const response = await fetch(url, { cache: "no-store" });
  if (!response.ok) throw new Error(`artifact-fetch:${artifact.id}:${response.status}`);
  const bytes = await response.arrayBuffer();
  if (bytes.byteLength !== artifact.bytes) {
    throw new Error(`artifact-size:${artifact.id}`);
  }
  return { artifact, bytes, url };
}

async function sha256Hex(bytes) {
  const digest = await crypto.subtle.digest("SHA-256", bytes);
  return [...new Uint8Array(digest)]
    .map((value) => value.toString(16).padStart(2, "0"))
    .join("");
}

if (browserPlan.schema !== "conduit.tour-browser-plan/v1") {
  throw new Error("unsupported Tour browser plan");
}
const adapterArtifact = browserPlan.artifacts.find(
  (artifact) => artifact.id === "browser-host-adapter",
);
const loadedAdapter = await fetchArtifact(adapterArtifact);
if (await sha256Hex(loadedAdapter.bytes) !== adapterArtifact.sha256) {
  throw new Error("browser host adapter integrity mismatch");
}
const {
  BrowserHostReason,
  DedicatedWorkerExecutionAdapter,
  Placement,
  observeBrowserHost,
  resolveBrowserPlacement,
  verifyExactArtifact,
} = await import(loadedAdapter.url);

const loadedArtifacts = new Map([[adapterArtifact.id, loadedAdapter]]);
for (const artifact of browserPlan.artifacts) {
  if (loadedArtifacts.has(artifact.id)) continue;
  const loaded = await fetchArtifact(artifact);
  const verified = await verifyExactArtifact(loaded.bytes, artifact.sha256);
  if (!verified.ok) {
    throw new Error(`${BrowserHostReason.ArtifactIntegrity}:${artifact.id}`);
  }
  loadedArtifacts.set(artifact.id, loaded);
}
await init({
  module_or_path: loadedArtifacts.get("conduit-web-wasm").bytes,
});

const placementFact = (id, available, lifetime, scheduling, transfer, terminalRisks) => ({
  id,
  available,
  lifetime,
  scheduling,
  transfer,
  limits: { queueBytes: browserPlan.bounds.maximum_message_bytes },
  terminalRisks,
});
const hostReport = observeBrowserHost({
  hostId: "conduit/tour-browser-host",
  observationId: browserPlan.observation_id,
  reporter: {
    realmId: "conduit/tour-static-realm",
    entityId: "conduit/tour-browser-workload",
    passportIdentity: "conduit/tour-browser-passport-v1",
    statusObservation: {
      realmId: "conduit/tour-static-realm",
      entityId: "conduit/tour-browser-workload",
      passportIdentity: "conduit/tour-browser-passport-v1",
      reporterIdentity: "conduit/tour-static-status-reporter",
      timeBasis: "conduit/tour-fixture-clock",
      observedAtTick: 9,
      validUntilTick: 100,
      status: "active",
    },
  },
  tick: 10,
  validUntilTick: 100,
  context: {
    secureContext: globalThis.isSecureContext,
    origin: globalThis.location.origin,
    crossOriginIsolated: globalThis.crossOriginIsolated,
  },
  placements: [
    placementFact(
      Placement.DedicatedWorker,
      typeof Worker === "function",
      "worker",
      "event-loop",
      "structured-clone",
      ["worker-death", "page-close"],
    ),
    placementFact(
      Placement.Wasm,
      typeof WebAssembly === "object",
      "worker",
      "placement-owned",
      "linear-memory",
      ["trap", "worker-death"],
    ),
  ],
  permissions: [],
  activation: false,
  resources: {
    queueBytes: browserPlan.bounds.maximum_message_bytes,
    pendingMessages: browserPlan.bounds.maximum_pending,
  },
});
if (hostReport.ok === false) {
  throw new Error(`${hostReport.code}:${hostReport.detail}`);
}

// Cytoscape initialization
let cy = cytoscape({
  container: document.getElementById("cy"),
  style: [
    {
      selector: "node",
      style: {
        "label": "data(label)",
        "shape": "round-rectangle",
        "width": "label",
        "height": "label",
        "padding": "12px",
        "color": "#f8fafc",
        "background-color": "data(bg)",
        "border-width": "2px",
        "border-color": "data(borderColor)",
        "font-family": "Fira Code, monospace",
        "font-size": "13px",
        "text-valign": "center",
        "text-halign": "center"
      }
    },
    {
      selector: "node:selected",
      style: {
        "border-width": "4px",
        "border-color": "#38bdf8",
        "shadow-blur": "15px",
        "shadow-color": "#38bdf8"
      }
    },
    {
      selector: "edge",
      style: {
        "label": "data(label)",
        "width": 3,
        "line-color": "#64748b",
        "target-arrow-color": "#64748b",
        "target-arrow-shape": "triangle",
        "curve-style": "bezier",
        "font-size": "10px",
        "color": "#94a3b8",
        "font-family": "Fira Code, monospace",
        "text-background-color": "#090d12",
        "text-background-opacity": 0.8,
        "text-background-padding": "3px"
      }
    },
    {
      selector: "edge:selected",
      style: {
        "line-color": "#38bdf8",
        "target-arrow-color": "#38bdf8",
        "width": 4
      }
    }
  ],
  elements: [],
  layout: { name: "preset" }
});

let current = lessons.lessons[0];
let acceptedSource = "";
let selectedNode = null;
let positions = {};
let activeAdapter = null;
let runEpoch = 0;
let topologyView = "logical";
const evidence = [];
const draftKey = (id) => `conduit-tour-draft/${id}`;
const recoveryKey = (id) => `conduit-tour-reset-recovery/${id}`;

function getNodeColor(kind) {
  if (!kind) return { bg: "#1e293b", border: "#475569" };
  if (kind.includes("literal")) return { bg: "#064e3b", border: "#10b981" };
  if (kind.includes("stdout") || kind.includes("stdin")) return { bg: "#0c4a6e", border: "#38bdf8" };
  if (kind.includes("uppercase") || kind.includes("passthrough")) return { bg: "#312e81", border: "#818cf8" };
  if (kind.includes("http") || kind.includes("wifi") || kind.includes("socket")) return { bg: "#581c87", border: "#c084fc" };
  if (kind.includes("gpio") || kind.includes("counter") || kind.includes("cell")) return { bg: "#78350f", border: "#fbbf24" };
  return { bg: "#1e293b", border: "#64748b" };
}

function updateCytoscapeGraph() {
  const parsed = JSON.parse(parse_panel(source.value));
  if (!parsed.ok) {
    cy.elements().remove();
    return;
  }

  const elements = [];
  const nodesMap = new Map();
  const cordsMap = [];

  // Parse lines to extract node definitions and cord connections
  const lines = source.value.split("\n");
  let currentCord = null;

  for (const line of lines) {
    const trimmed = line.trim();
    const nodeMatch = trimmed.match(/^node\s+([A-Za-z0-9_-]+)\s*:\s*([A-Za-z0-9_\/-]+)/);
    if (nodeMatch) {
      nodesMap.set(nodeMatch[1], nodeMatch[2]);
    }
    const cordMatch = trimmed.match(/^cord\s+([A-Za-z0-9_-]+)\.([A-Za-z0-9_-]+)\s*->\s*([A-Za-z0-9_-]+)\.([A-Za-z0-9_-]+)/);
    if (cordMatch) {
      currentCord = {
        source: cordMatch[1],
        sourcePort: cordMatch[2],
        target: cordMatch[3],
        targetPort: cordMatch[4],
        capacity: "8",
        pressure: "block"
      };
      cordsMap.push(currentCord);
    } else if (currentCord) {
      const capMatch = trimmed.match(/capacity\s*=\s*(\d+)/);
      if (capMatch) currentCord.capacity = capMatch[1];
      const pressMatch = trimmed.match(/pressure\s*=\s*([A-Za-z0-9_-]+)/);
      if (pressMatch) currentCord.pressure = pressMatch[1];
      if (trimmed.includes("}")) currentCord = null;
    }
  }

  // Create Nodes
  let idx = 0;
  const nodeCount = nodesMap.size;
  nodesMap.forEach((kind, id) => {
    const colors = getNodeColor(kind);
    const pos = positions[id] ?? {
      x: 100 + (idx % 3) * 220,
      y: 100 + Math.floor(idx / 3) * 120
    };
    elements.push({
      group: "nodes",
      data: {
        id,
        label: `${id}\n[${kind}]`,
        kind,
        bg: colors.bg,
        borderColor: colors.border
      },
      position: pos
    });
    idx++;
  });

  // Create Edges (Cords)
  cordsMap.forEach((c, index) => {
    elements.push({
      group: "edges",
      data: {
        id: `cord_${index}_${c.source}_${c.target}`,
        source: c.source,
        target: c.target,
        label: `${c.sourcePort} → ${c.targetPort}\ncap:${c.capacity} [${c.pressure}]`
      }
    });
  });

  cy.json({ elements });
  cy.fit(undefined, 30);
}

// Cytoscape Event Listeners
cy.on("dragfree", "node", (event) => {
  const node = event.target;
  const pos = node.position();
  const nodeId = node.id();
  const transaction = JSON.parse(
    patchbay_move_node(source.value, nodeId, Math.round(pos.x), Math.round(pos.y))
  );
  if (transaction.ok) {
    positions = transaction.positions;
    result.textContent = `Presentation moved node ${nodeId}; semantic hash remains ${transaction.semantic_hash}.`;
  }
});

cy.on("tap", "node", (event) => {
  const node = event.target;
  selectNode(node.id());
});

function recordEvidence(event) {
  evidence.push(event);
  const maximum = Math.min(
    browserPlan.bounds.maximum_evidence_events,
    current.budgets?.evidence_events || browserPlan.bounds.maximum_evidence_events,
  );
  if (evidence.length > maximum) evidence.splice(0, evidence.length - maximum);
  document.querySelector("#evidence").textContent =
    evidence.length === 0 ? "No run evidence yet." : JSON.stringify(evidence, null, 2);
}

function renderPlan(binding = null) {
  document.querySelector("#plan").textContent = JSON.stringify({
    source_profile: current.profile || "conduit/std",
    semantic_contract: browserPlan.semantic_contract,
    implementation_id: browserPlan.implementation_id,
    plan_identity: browserPlan.plan_identity,
    host_observation: hostReport.observationId,
    placement: binding?.placement ?? browserPlan.placement,
    bounds: browserPlan.bounds,
    artifacts: browserPlan.artifacts.map(({ id, sha256, bytes }) => ({ id, sha256, bytes })),
  }, null, 2);
}

function stopActive(cause, message) {
  runEpoch += 1;
  if (activeAdapter) {
    activeAdapter.terminate(cause);
    activeAdapter = null;
  }
  runButton.disabled = false;
  stopButton.disabled = true;
  consoleBadge.textContent = "Ready";
  consoleBadge.className = "badge status-badge idle";
  if (message) result.textContent = message;
}

function show(lesson) {
  stopActive("lesson-changed");
  current = lesson;
  document.querySelector("#title").textContent = lesson.title;
  document.querySelector("#goal").textContent = lesson.objective || lesson.title;
  document.querySelector("#prose").textContent = lesson.prose || "";
  document.querySelector("#execution-note").textContent =
    `${lesson.profile || "conduit/std"}: exact ${browserPlan.placement} placement on ${hostReport.hostId}.`;
  document.querySelector("#command").textContent =
    (lesson.commands ?? [lesson.command || "conduct inspect"]).join("  ·  ");

  // Active state highlighting in nav
  document.querySelectorAll(".nav-list button").forEach((btn) => {
    btn.classList.toggle("active", btn.dataset.id === lesson.id);
  });

  const draft = localStorage.getItem(draftKey(lesson.id));
  source.value = draft ?? lesson.source;
  const parsedDraft = JSON.parse(parse_panel(source.value));
  acceptedSource = parsedDraft.ok ? source.value : lesson.source;
  selectedNode = null;
  positions = {};
  topologyView = "logical";
  undoResetButton.disabled = localStorage.getItem(recoveryKey(lesson.id)) === null;
  document.querySelector("#topology-inspector").open =
    lesson.presentation?.layout === "logical-expanded";
  evidence.length = 0;
  recordEvidence({ kind: "lesson-selected", lesson: lesson.id });
  renderPlan();
  check();
}

function selectNode(node) {
  selectedNode = node;
  selectedNodeLabel.textContent = `Selected: ${node}`;
  moveLeftBtn.disabled = false;
  moveRightBtn.disabled = false;

  const start = source.value.indexOf(`node ${node} `);
  if (start >= 0) {
    source.focus();
    source.setSelectionRange(start, start + node.length + 5);
  }
  cy.nodes().unselect();
  const cyNode = cy.getElementById(node);
  if (cyNode) cyNode.select();
}

function check() {
  const value = JSON.parse(parse_panel(source.value));
  result.textContent = value.ok
    ? `Valid panel: ${value.nodes} nodes, ${value.cords} cords.`
    : value.diagnostic;
  updateCytoscapeGraph();
  renderTopology();
}

function renderTopology() {
  const explanation = JSON.parse(explain_panel(source.value));
  document.querySelector("#logical-view").classList.toggle("active", topologyView === "logical");
  document.querySelector("#expanded-view").classList.toggle("active", topologyView === "expanded");
  document.querySelector("#topology").textContent = explanation.ok
    ? explanation[topologyView]
    : explanation.diagnostic;
}

// Populate Lessons Nav
for (const lesson of lessons.lessons) {
  const button = document.createElement("button");
  button.textContent = lesson.title;
  button.dataset.id = lesson.id;
  button.onclick = () => show(lesson);
  const item = document.createElement("li");
  item.append(button);
  document.querySelector("#lessons").append(item);
}

// Populate Reference Panels Nav
for (const refPanel of referencePanels) {
  const button = document.createElement("button");
  button.textContent = refPanel.title;
  button.dataset.id = refPanel.id;
  button.onclick = () => show(refPanel);
  const item = document.createElement("li");
  item.append(button);
  document.querySelector("#reference-panels").append(item);
}

source.addEventListener("input", () => {
  localStorage.setItem(draftKey(current.id), source.value);
  const transaction = JSON.parse(patchbay_replace_source(acceptedSource, source.value));
  if (transaction.ok) acceptedSource = source.value;
  check();
});

source.addEventListener("select", () => {
  const before = source.value.slice(0, source.selectionStart);
  const match = before.match(/node\s+([A-Za-z][A-Za-z0-9_-]*)\s*$/);
  if (match) {
    selectNode(match[1]);
  }
});

document.querySelector("#check").onclick = check;
document.querySelector("#logical-view").onclick = () => {
  topologyView = "logical";
  renderTopology();
};
document.querySelector("#expanded-view").onclick = () => {
  topologyView = "expanded";
  renderTopology();
};

function moveSelected(delta) {
  if (!selectedNode) return;
  const cyNode = cy.getElementById(selectedNode);
  if (!cyNode) return;
  const pos = cyNode.position();
  const transaction = JSON.parse(
    patchbay_move_node(source.value, selectedNode, pos.x + delta, pos.y),
  );
  if (!transaction.ok) {
    result.textContent = transaction.diagnostic;
    return;
  }
  positions = transaction.positions;
  cyNode.position({ x: pos.x + delta, y: pos.y });
  result.textContent = `Presentation moved; semantic hash remains ${transaction.semantic_hash}.`;
}

moveLeftBtn.onclick = () => moveSelected(-20);
moveRightBtn.onclick = () => moveSelected(20);

async function run() {
  stopActive("superseded");
  const epoch = ++runEpoch;
  const binding = resolveBrowserPlacement(hostReport, {
    tick: 11,
    placement: Placement.DedicatedWorker,
    minimumResources: {
      queueBytes: browserPlan.bounds.maximum_message_bytes,
      pendingMessages: browserPlan.bounds.maximum_pending,
    },
  });
  if (!binding.ok) {
    result.textContent = `${binding.code}: ${binding.detail}`;
    recordEvidence({ kind: "placement-rejected", code: binding.code });
    return;
  }
  renderPlan(binding);
  const workerArtifact = loadedArtifacts.get("tour-worker");
  const wasmArtifact = loadedArtifacts.get("conduit-web-wasm");
  const adapter = new DedicatedWorkerExecutionAdapter({
    ...binding,
    planIdentity: browserPlan.plan_identity,
    artifactUrl: workerArtifact.url,
    maximumPending: browserPlan.bounds.maximum_pending,
    maximumMessageBytes: browserPlan.bounds.maximum_message_bytes,
    responseTimeoutMs: browserPlan.bounds.response_timeout_ms,
  }, recordEvidence);
  activeAdapter = adapter;
  const started = adapter.start();
  if (!started.ok) {
    activeAdapter = null;
    result.textContent = `${started.code}: ${started.detail}`;
    return;
  }
  runButton.disabled = true;
  stopButton.disabled = false;
  consoleBadge.textContent = "Running";
  consoleBadge.className = "badge status-badge running";
  result.textContent = "Executing graph in browser placement worker…";

  try {
    const configured = await adapter.request("configure", {
      wasmUrl: wasmArtifact.url.href,
      wasmSha256: wasmArtifact.artifact.sha256,
    });
    if (!configured.ok) throw new Error(configured.code);
    const executed = await adapter.request("run", { source: source.value });
    if (epoch !== runEpoch) return;
    if (!executed.ok) throw new Error(executed.code);
    const value = executed.value;
    const visibleResult = value.ok
      ? `${value.stdout || "Run completed successfully."}\nEvidence: ${value.completed_nodes} nodes, ${value.cords_conducted} cords conducted.`
      : value.diagnostic;

    const lessonComplete = current.validation?.kind === "stdout"
      ? value.ok && value.stdout === current.validation.value
      : (current.validation?.value ? !value.ok && value.diagnostic.includes(current.validation.value) : value.ok);

    result.textContent = lessonComplete
      ? `✓ Lesson complete!\n${visibleResult}`
      : visibleResult;

    recordEvidence({
      kind: lessonComplete ? "lesson-completed" : (value.ok ? "run-completed" : "run-rejected"),
      lesson: current.id,
      completedNodes: value.completed_nodes,
      cordsConducted: value.cords_conducted,
    });
  } catch (error) {
    if (epoch === runEpoch) result.textContent = `Run failed: ${error}`;
  } finally {
    if (epoch === runEpoch) {
      adapter.terminate("completed");
      activeAdapter = null;
      runButton.disabled = false;
      stopButton.disabled = true;
      consoleBadge.textContent = "Idle";
      consoleBadge.className = "badge status-badge idle";
    }
  }
}

runButton.onclick = run;
stopButton.onclick = () => stopActive(
  "learner-cancelled",
  "Run cancelled; exact worker placement is terminal.",
);

source.addEventListener("keydown", (event) => {
  if (event.shiftKey && event.key === "Enter") {
    event.preventDefault();
    run();
  }
});

document.querySelector("#reset").onclick = () => {
  stopActive("reset");
  localStorage.setItem(recoveryKey(current.id), source.value);
  source.value = current.source;
  acceptedSource = source.value;
  selectedNode = null;
  positions = {};
  selectedNodeLabel.textContent = "No node selected";
  moveLeftBtn.disabled = true;
  moveRightBtn.disabled = true;
  localStorage.removeItem(draftKey(current.id));
  undoResetButton.disabled = false;
  check();
};

undoResetButton.onclick = () => {
  const recovered = localStorage.getItem(recoveryKey(current.id));
  if (recovered === null) return;
  source.value = recovered;
  localStorage.setItem(draftKey(current.id), recovered);
  localStorage.removeItem(recoveryKey(current.id));
  undoResetButton.disabled = true;
  const parsed = JSON.parse(parse_panel(recovered));
  acceptedSource = parsed.ok ? recovered : current.source;
  check();
};

document.querySelector("#download").onclick = () => {
  const link = document.createElement("a");
  link.href = URL.createObjectURL(new Blob([source.value], { type: "text/plain" }));
  link.download = `${current.id}.panel`;
  link.click();
  URL.revokeObjectURL(link.href);
};

show(current);
if (new URLSearchParams(location.search).has("autorun")) await run();
