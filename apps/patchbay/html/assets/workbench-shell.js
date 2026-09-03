const workbenchSchema = "conduit.patchbay/browser-body-workbench@1";
let latestSnapshot = null;
let navigate = null;
let invoke = null;

function variantName(value) {
  return typeof value === "string" ? value : Object.keys(value ?? {})[0];
}

function requireWorkbench(snapshot) {
  const value = snapshot.workbench;
  if (!value) return null;
  const current = value.current, history = value.history;
  if (value.schema !== workbenchSchema || !Number.isSafeInteger(value.evidence_revision) || value.evidence_revision < 1) throw new Error("malformed Body workbench revision");
  if (!current || current.body_id !== snapshot.presentation.basis.body_id || history?.body_id !== current.body_id || !Array.isArray(current.current_hosts) || !Array.isArray(history.entries) || history.entries.length < 1 || history.entries.length > 64) throw new Error("malformed Body workbench identity");
  if (history.place !== "Body" || history.aspect !== "Signs" || history.exact_depth !== "Exact" || history.alternate_manifestation !== "Linear") throw new Error("malformed Body history location");
  let previous = 0;
  for (const entry of history.entries) {
    const record = entry.exact_record;
    if (!record || !Number.isSafeInteger(entry.evidence_sequence) || entry.evidence_sequence <= previous || entry.evidence_sequence !== record.sequence || entry.exact_body_id !== current.body_id || entry.inspect_sign_id !== record.sign_id || entry.inspect_subject !== `sign/${record.sign_id}` || entry.inspect_place !== "Body" || entry.inspect_aspect !== "Signs" || entry.inspect_depth !== "Exact" || typeof entry.title !== "string" || entry.title.length < 1 || entry.title.length > 64 || typeof entry.narrative !== "string" || entry.narrative.length > 512 || typeof entry.linear !== "string" || entry.linear.length > 1024) throw new Error("malformed bounded Body history entry");
    previous = entry.evidence_sequence;
  }
  return value;
}

function lifecycleLabel(value) {
  return variantName(value) === "Awake" ? "Awake" : "Lulled";
}

function readerLabel(value) {
  const kind = variantName(value);
  if (kind === "HostedByBody") return "Hosted by this Body";
  if (kind === "ExternalReadingHostedBody") return "External reader · Body retains a hosted Patchbay";
  return "External reader · no hosted Patchbay placement";
}

function exactRecord(entry) {
  const dl = document.createElement("dl");
  const terms = [
    ["Body", entry.exact_body_id],
    ["Sequence", String(entry.exact_record.sequence)],
    ["Sign", entry.exact_record.sign_id],
    ["Record kind", variantName(entry.exact_record.kind)],
    ["Typed record", JSON.stringify(entry.exact_record.kind)],
  ];
  for (const [name, value] of terms) {
    const dt = document.createElement("dt"), dd = document.createElement("dd");
    dt.textContent = name; dd.textContent = value; dl.append(dt, dd);
  }
  return dl;
}

function canInspect(entry) {
  const navigation = latestSnapshot?.navigation;
  const body = navigation?.navigation.places.find(place => place.place === "Body");
  const signs = body?.aspects.find(aspect => aspect.aspect === "Signs");
  return signs?.focusable_subjects.includes(entry.inspect_subject) ?? false;
}

function renderHistory(workbench) {
  const list = document.querySelector("#history-entries"), linear = document.querySelector("#history-linear ol");
  list.replaceChildren(); linear.replaceChildren();
  for (const entry of workbench.history.entries) {
    const item = document.createElement("li"), marker = document.createElement("p"), title = document.createElement("h3"), narrative = document.createElement("p"), exact = document.createElement("details"), summary = document.createElement("summary"), linearItem = document.createElement("li");
    item.className = "history-entry"; marker.className = "history-sequence"; marker.textContent = `Evidence ${entry.evidence_sequence}`; title.textContent = entry.title; narrative.textContent = entry.narrative; summary.textContent = "Exact evidence"; exact.append(summary, exactRecord(entry));
    if (canInspect(entry)) {
      const inspect = document.createElement("button"); inspect.type = "button"; inspect.textContent = "Focus this Sign"; inspect.onclick = () => navigate({kind:"focus", subject:entry.inspect_subject, depth:"Exact"}); exact.append(inspect);
    }
    linearItem.textContent = entry.linear;
    item.append(marker, title, narrative, exact); list.append(item); linear.append(linearItem);
  }
}

function setView(view) {
  document.body.dataset.workbenchView = view;
  document.querySelector("#history-workspace").hidden = view !== "history";
  for (const name of ["program", "body", "history"]) document.querySelector(`#show-${name}`).setAttribute("aria-pressed", String(name === view));
}

async function enter(view) {
  if (!latestSnapshot?.navigation) return;
  if (view === "program") await navigate({kind:"enter", place:"Program"});
  else {
    await navigate({kind:"enter", place:"Body"});
    if (view === "history") await navigate({kind:"show", aspect:"Signs"});
  }
}

export function installBodyWorkbench(operations) {
  navigate = operations.navigate;
  invoke = operations.invoke;
  for (const view of ["program", "body", "history"]) document.querySelector(`#show-${view}`).onclick = () => enter(view);
}

export function renderBodyWorkbench(snapshot) {
  latestSnapshot = snapshot;
  const workbench = requireWorkbench(snapshot);
  if (!workbench) {
    clearBodyWorkbench(snapshot.presentation.basis.body_id ? "Durable Body evidence is not attached to this reader." : "No Body is attached yet.");
    return;
  }
  const current = workbench.current, cursor = snapshot.navigation?.cursor;
  const view = cursor?.place === "Body" ? (cursor.aspect === "Signs" ? "history" : "body") : "program";
  setView(view);
  document.querySelector("#body-name").textContent = current.friendly_name;
  document.querySelector("#body-program").textContent = current.program_label;
  document.querySelector("#body-status").textContent = current.status_line;
  document.querySelector("#body-placement").textContent = `${readerLabel(current.reader)} · ${current.placement_line}`;
  const action = document.querySelector("#body-lifecycle-action"), actionName = variantName(current.salient_action), intent = `conduit.intent/${actionName.toLowerCase()}@1`, preferred = snapshot.presentation.actions.find(candidate => candidate.intent === intent && candidate.availability === "Available"), semantic = preferred ?? snapshot.presentation.actions.find(candidate => candidate.availability === "Available" && ["conduit.intent/wake@1","conduit.intent/hold@1","conduit.intent/stop@1","conduit.intent/lull@1"].includes(candidate.intent));
  action.textContent = semantic?.label ?? actionName; action.disabled = !semantic; action.title = semantic ? "Current semantic lifecycle action" : `${actionName} is not admitted by this Presentation`;
  action.onclick = semantic ? () => invoke(semantic) : null;
  document.querySelector("#parts-lifecycle").textContent = lifecycleLabel(current.lifecycle).toUpperCase();
  document.querySelector("#parts-title").textContent = `${current.friendly_name} · ${current.admitted_parts} Parts`;
  document.querySelector("#show-body").disabled = false;
  document.querySelector("#show-history").disabled = false;
  renderHistory(workbench);
}

export function clearBodyWorkbench(reason) {
  latestSnapshot = null;
  document.querySelector("#body-name").textContent = "Body unavailable";
  document.querySelector("#body-program").textContent = "No current Program projection";
  document.querySelector("#body-status").textContent = reason;
  document.querySelector("#body-placement").textContent = "No hosted or external attachment is being claimed.";
  const action = document.querySelector("#body-lifecycle-action"); action.disabled = true; action.onclick = null;
  document.querySelector("#show-body").disabled = true;
  document.querySelector("#show-history").disabled = true;
  document.querySelector("#history-entries").replaceChildren();
  document.querySelector("#history-linear ol").replaceChildren();
  if (["body", "history"].includes(document.body.dataset.workbenchView)) setView("program");
}

export { requireWorkbench };
