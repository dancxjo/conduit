import { acquireBrowserBodyHost } from "./browser-body-host.mjs";
import { presentBodyPlan } from "./body-plan-inspection.mjs";

// Product orchestration only: the admitted WASM instance and shared Host
// adapters own execution. Claims and reports are self-reported loopback facts.
export function createBodyExecutionControl({ root, apiUrl, renderSnapshot }) {
  const document = root.ownerDocument;
  const section = document.createElement("section");
  section.className = "body-execution";
  section.setAttribute("aria-label", "Body execution");
  const start = document.createElement("button");
  start.type = "button";start.textContent = "Start proposed Body Play";start.disabled = true;
  const stop = document.createElement("button");
  stop.type = "button";stop.textContent = "Cancel Body Play";stop.disabled = true;
  const status = document.createElement("p");
  status.id = "body-execution-status";status.setAttribute("role", "status");
  status.textContent = "No Body Play started. Plan the active Forms first.";
  const input = document.createElement("div");
  input.id = "body-execution-input";input.tabIndex = 0;
  input.setAttribute("role", "group");input.setAttribute("aria-label", "Body Play input");
  input.textContent = "Body Play input: click here for button transitions; focus here and type for keyboard Forms.";
  const output = document.createElement("div");output.id = "body-execution-output";
  const evidence = document.createElement("pre");evidence.id = "body-execution-evidence";
  const exact = document.createElement("details"), summary = document.createElement("summary");
  summary.textContent = "Exact Body Play evidence";exact.append(summary, evidence);
  const selection = document.createElement("details"), selectionTitle = document.createElement("summary");
  selectionTitle.textContent = "Inspect selected Body Plan";
  const planInspection = document.createElement("div");planInspection.id = "body-plan-inspection";
  selection.append(selectionTitle, planInspection);
  section.append(start, stop, status, input, output, selection, exact);root.append(section);
  let host = null, planning = null, busy = false, owner = null, stopped = false, closed = null;

  const update = () => {
    start.disabled = busy || !host || planning?.lifecycle !== "AwaitingPlan" ||
      Boolean(planning.unavailable_proposal_sign_id) ||
      planning.execution_claims?.some(claim => claim.phase === "Claimed" || claim.phase === "Started") ||
      !planning.current_hosts.some(item => item.host_id === host.hostId && item.boot_id === host.bootId);
    stop.disabled = !busy || stopped;
  };
  const post = async action => {
    const response = await fetch(apiUrl("body-execution"), {
      method: "POST", headers: { "content-type": "application/json" },
      body: JSON.stringify({ schema: "conduit.patchbay/body-execution-request@1", action }),
    });
    if (!response.ok) throw new Error(await response.text() || `Body execution HTTP ${response.status}`);
    const snapshot = await response.json();
    renderSnapshot(snapshot);
    return snapshot.body_planning;
  };
  const close = () => {
    if (owner && !closed) closed = owner.close();
    return closed;
  };
  const cancel = () => {
    if (!busy) return;
    stopped = true;
    // Close only once. The running task owns reporting, including uncertain
    // outcomes while a claim/start response is still in flight.
    try { close(); } catch (error) { status.textContent = `Body cleanup failed: ${error.message}`; }
    update();
  };
  const terminalReport = async (play, receipt) => {
    if (receipt?.schema !== "conduit.tour/manifestation-receipt@3" || receipt.active_play_id !== play.active_play_id) {
      throw new Error("Body terminal receipt does not match the claimed Play");
    }
    await post({ kind: "Terminal", play, disposition: receipt.disposition, terminal_sign_id: receipt.terminal_sign_id });
    evidence.textContent = JSON.stringify({ proof_class: "SelfReported", play, receipt }, null, 2);
    status.textContent = `Body Play ${receipt.disposition} · ${play.active_play_id} · Wake Lull is separate.`;
  };
  const execute = async () => {
    if (start.disabled) return;
    const executingHost = host;
    busy = true;stopped = false;closed = null;update();
    let play = null, terminalAttempted = false;
    try {
      const response = await fetch(apiUrl("body-execution-proposal"), { cache: "no-store" });
      if (!response.ok) throw new Error(await response.text());
      const proposal = await response.json();
      if (stopped) throw new Error("Body start cancelled before acquisition");
      owner = acquireBrowserBodyHost({ ...executingHost, proposal, inputTarget: input, outputRoot: output });
      presentBodyPlan(planInspection, proposal);
      const result = await post({ kind: "Claim", plan_id: proposal.plan.plan_id, host_id: executingHost.hostId, boot_id: executingHost.bootId });
      const claim = result.execution_claims.at(-1);
      if (claim?.phase !== "Claimed" || claim.play.plan_id !== proposal.plan.plan_id ||
          claim.host_id !== executingHost.hostId || claim.boot_id !== executingHost.bootId) throw new Error("invalid coordinator start claim");
      play = claim.play;
      if (stopped) throw new Error("Body start cancelled before Play");
      const started = owner.start(play.play_sequence);
      if (Object.keys(play).some(key => started.play[key] !== play[key])) throw new Error("WASM started a different Body Play");
      await post({ kind: "Started", play, wake_at_start: started.wake_at_start });
      if (stopped) throw new Error("Body Play cancelled during start reporting");
      evidence.textContent = JSON.stringify({ proof_class: "SelfReported", play, wake_at_start: started.wake_at_start }, null, 2);
      status.textContent = `Body Play running · ${play.active_play_id}`;
      input.focus();
      const receipt = await owner.run();
      terminalAttempted = true;await terminalReport(play, receipt);
    } catch (error) {
      status.textContent = `Body execution stopped: ${error.message}`;
      try {
        const result = close();
        if (play && !terminalAttempted) {
          if (result?.receipt) await terminalReport(play, result.receipt);
          else if (["not-attempted", "refused-before-play"].includes(result?.startOutcome)) {
            await post({ kind: "RefusedBeforeStart", play, reason: error.message.slice(0, 256) || "Host refused before Play" });
          } else status.textContent += " · start outcome unknown; coordinator claim retained.";
        }
      } catch (cleanupError) { status.textContent += ` · cleanup/report failed: ${cleanupError.message}; claim outcome requires inspection.`; }
    } finally {
      try { close(); } catch (error) { status.textContent += ` · cleanup failed: ${error.message}`; }
      owner = null;busy = false;update();
    }
  };
  start.onclick = execute;stop.onclick = cancel;
  document.defaultView.addEventListener("pagehide", cancel);
  return Object.freeze({
    capabilityIds() {
      const api = host?.api;
      if (!api || typeof api.conduit_browser_body_capabilities !== "function" || api.conduit_browser_body_capabilities() < 0) throw new Error("local Body execution capabilities unavailable");
      const length = api.conduit_browser_form_output_len(), pointer = api.conduit_browser_form_output_ptr();
      if (!Number.isSafeInteger(length) || length < 1 || length > 16 * 1024) throw new Error("Body capability output bound exceeded");
      const value = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(new Uint8Array(api.memory.buffer, pointer, length)));
      if (value.schema !== "conduit.browser/body-capabilities@1" || !Array.isArray(value.capability_ids) || value.capability_ids.length > 70 || value.capability_ids.some(id => typeof id !== "string" || id.length < 1 || id.length > 256)) throw new Error("invalid Body execution capabilities");
      return new Set(value.capability_ids);
    },
    configureHost(current) { host = current;update(); },
    render(snapshot) { planning = snapshot.body_planning;update(); },
    unavailable() { host = null;cancel();update(); },
  });
}
