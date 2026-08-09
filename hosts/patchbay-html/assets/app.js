const schema = "conduit.patchbay.renderer-snapshot/1";
const providers = new Map([
  ["Local", "local"], ["InMemory", "in-memory"],
  ["FixtureFrame", "fixture frame"], ["FixtureDatagram", "fixture datagram"],
  ["WebSocket", "WebSocket"], ["UsbCdc", "USB CDC"],
]);
const state = { snapshot: null, selected: null, zoom:1, panX:0, reversed:false, graphHeight:360 };

function requireSnapshot(value) {
  if (!value || value.schema !== schema || !Number.isSafeInteger(value.revision)) throw new Error("unsupported snapshot schema");
  if (!value.document || !Array.isArray(value.document.forms) || !Array.isArray(value.routes) || !Array.isArray(value.linear)) throw new Error("malformed typed snapshot");
  for (const route of value.routes) {
    for (const candidate of [...route.new_plan.candidates, ...route.same_plan.candidates]) {
      if (!providers.has(candidate.provider) || typeof candidate.binding_id !== "string" || typeof candidate.provider_instance_id !== "string") throw new Error("malformed typed route candidate");
    }
  }
  return value;
}

function term(dl, name, value) {
  const dt = document.createElement("dt"); dt.textContent = name;
  const dd = document.createElement("dd"); dd.textContent = value ?? "not present";
  dl.append(dt, dd);
}

function openForm() {
  return state.snapshot.document.forms.find((item) => item.name === state.snapshot.document.open_form);
}

function subjectName(item) { return `${item.kind}: ${item.label}`; }

function routeArticle(title, lines) {
  const article=document.createElement("article");article.className="route-card";
  const heading=document.createElement("h3");heading.textContent=title;article.append(heading);
  for(const line of lines){const paragraph=document.createElement("p");paragraph.textContent=line;article.append(paragraph);}
  return article;
}

function select(identity) {
  state.selected = identity;
  document.querySelectorAll("[data-subject]").forEach((item) => {
    const selected = item.dataset.subject === identity;
    item.classList.toggle("selected", selected);
    if (item.tagName === "BUTTON") item.setAttribute("aria-pressed", String(selected));
  });
  const item = openForm()?.items.find((candidate) => candidate.identity === identity);
  const dl = document.querySelector("#inspector dl"); dl.replaceChildren();
  term(dl, "Semantic subject", identity); term(dl, "Kind", item?.kind); term(dl, "Meaning", item?.label);
  term(dl, "Checked Form", openForm()?.checked_form_id);
  term(dl, "Plan", state.snapshot.plan?.plan_id); term(dl, "Active Play", state.snapshot.play?.active_play_id);
}

function graphNodes(form) {
  const nodes = form.items.filter((item) => item.kind !== "Cord").map((item) => ({ id:item.identity, label:item.label, item }));
  for (const cord of form.cords) cord.stages.forEach((stage, index) => {
    if (stage === "Literal" || (typeof stage === "object" && stage.InlineCell)) nodes.push({ id:`${cord.identity}/stage/${index}`, label:stage.InlineCell?.operation ?? "literal" });
  });
  return state.reversed ? nodes.reverse() : nodes;
}

function applyViewport() {
  const width=740/state.zoom,height=state.graphHeight/state.zoom;
  document.querySelector("#graph").setAttribute("viewBox",`${state.panX} 0 ${width} ${height}`);
}

function stageId(stage, cord, index, nodes) {
  if (typeof stage === "string") return `${cord.identity}/stage/${index}`;
  if (stage.Reference) {
    return nodes.find((node) => node.id.endsWith(`/cell/${stage.Reference}`) || node.id.endsWith(`/port/${stage.Reference}`) || node.id.endsWith(`/startup/${stage.Reference}`))?.id;
  }
  return `${cord.identity}/stage/${index}`;
}

function renderGraph(form) {
  const svg = document.querySelector("#graph"); svg.replaceChildren(svg.querySelector("title"));
  const nodes = graphNodes(form); const positions = new Map();
  nodes.forEach((node, index) => positions.set(node.id, { x:40 + (index % 3) * 230, y:40 + Math.floor(index / 3) * 120 }));
  for (const cord of form.cords) {
    const cordGroup = document.createElementNS("http://www.w3.org/2000/svg", "g");
    cordGroup.dataset.subject = cord.identity; cordGroup.setAttribute("role", "img"); cordGroup.setAttribute("aria-label", `Cord ${cord.identity}`);
    for (let index=1; index<cord.stages.length; index++) {
      const from = positions.get(stageId(cord.stages[index-1], cord, index-1, nodes));
      const to = positions.get(stageId(cord.stages[index], cord, index, nodes));
      if (!from || !to) continue;
      const line = document.createElementNS("http://www.w3.org/2000/svg", "line");
      line.setAttribute("x1", from.x+80); line.setAttribute("y1", from.y+25); line.setAttribute("x2", to.x+80); line.setAttribute("y2", to.y+25);
      line.setAttribute("stroke", "#8ee3b8"); line.setAttribute("stroke-width", "3"); line.dataset.cord = cord.identity; cordGroup.append(line);
    }
    svg.append(cordGroup);
  }
  for (const node of nodes) {
    const position = positions.get(node.id); const group = document.createElementNS("http://www.w3.org/2000/svg", "g");
    group.dataset.subject = node.id; group.setAttribute("role", "button"); group.setAttribute("tabindex", "0"); group.setAttribute("aria-label", node.label);
    const rect = document.createElementNS("http://www.w3.org/2000/svg", "rect"); rect.setAttribute("x", position.x); rect.setAttribute("y", position.y); rect.setAttribute("width", "160"); rect.setAttribute("height", "50"); rect.setAttribute("rx", "8"); rect.setAttribute("class", "node");
    const text = document.createElementNS("http://www.w3.org/2000/svg", "text"); text.setAttribute("x", position.x+12); text.setAttribute("y", position.y+30); text.setAttribute("class", "node-label"); text.textContent = node.label.slice(0,20);
    group.append(rect,text); group.addEventListener("click",()=>select(node.id)); group.addEventListener("keydown",event=>{ if(event.key==="Enter"||event.key===" "){event.preventDefault();select(node.id);} }); svg.append(group);
  }
  state.graphHeight=Math.max(360,100+Math.ceil(nodes.length/3)*120);applyViewport();
}

function render(snapshot) {
  state.snapshot = snapshot; const form = openForm(); if (!form) throw new Error("open Form missing");
  document.querySelector("#status").textContent = `Snapshot revision ${snapshot.revision} · read-only · authoritative IDs preserved`;
  const formFacts=document.querySelector("#form-facts");formFacts.replaceChildren();term(formFacts,"Source document",snapshot.document.source_document_id);term(formFacts,"Checked Form",form.checked_form_id);term(formFacts,"Face",`${form.items.filter(item=>item.kind.startsWith("Face")).length} runtime ports`);term(formFacts,"Back",`${form.items.filter(item=>item.kind==="Cell").length} cells · ${form.cords.length} cords`);
  const list = document.querySelector("#subjects"); list.replaceChildren();
  for (const item of form.items) { const li=document.createElement("li"), button=document.createElement("button"); button.type="button"; button.dataset.subject=item.identity; button.setAttribute("aria-pressed","false"); button.textContent=subjectName(item); button.onclick=()=>select(item.identity); li.append(button); list.append(li); }
  renderGraph(form);
  const plan=document.querySelector("#plan dl"); plan.replaceChildren(); term(plan,"Source",snapshot.plan?.source_document_id); term(plan,"Checked Form",snapshot.plan?.checked_form_id); term(plan,"Expanded Form",snapshot.plan?.expanded_form_id); term(plan,"Plan",snapshot.plan?.plan_id);
  const realizations=document.querySelector("#realizations");realizations.replaceChildren();for(const fragment of snapshot.plan?.exact.fragments??[]){for(const placement of fragment.placements){const li=document.createElement("li");li.textContent=`${placement.operation_id} · host ${placement.host_id} · boot ${placement.boot_id} · implementation ${placement.implementation_id} · artifact ${placement.artifact_id}`;realizations.append(li);}}
  const play=document.querySelector("#play dl"); play.replaceChildren(); term(play,"Active Play",snapshot.play?.active_play_id); term(play,"Plan",snapshot.play?.plan_id); term(play,"Terminal",snapshot.play?.terminal);
  const diagnostics=document.querySelector("#diagnostics ol");diagnostics.replaceChildren();for(const item of snapshot.document.attempted_edit?.diagnostics??[]){const li=document.createElement("li");li.textContent=`${item.code}: ${item.message} · bytes ${item.span.start}–${item.span.end} · line ${item.span.line}:${item.span.column}`;diagnostics.append(li);}
  const evidence=document.querySelector("#evidence"); evidence.replaceChildren(); for(const item of snapshot.play?.evidence??[]){const li=document.createElement("li");li.textContent=`${item.evidence_id} — ${typeof item.kind==="string"?item.kind:Object.keys(item.kind)[0]}`;evidence.append(li);}
  const cards=document.querySelector("#route-cards"); cards.replaceChildren();
  for(const route of snapshot.routes){const grid=document.createElement("div");grid.className="route-grid";grid.append(routeArticle("New-Plan recovery",[`Plan ${route.new_plan.prior_plan_id} → unsatisfied → Plan ${route.new_plan.replacement_plan_id}`,`Evidence: ${route.new_plan.evidence_ids.join(", ")}`]),routeArticle("Same-Plan fallback",[`Plan ${route.same_plan.plan_id} remains unchanged.`,`Evidence: ${route.same_plan.evidence_ids.join(", ")}`]),routeArticle("Refused ambient route",[`Binding ${route.refused_binding_id} was not sealed into the Plan.`,`Evidence: ${route.refused_evidence_id}`]));for(const [container,data] of [[grid.children[0],route.new_plan],[grid.children[1],route.same_plan]]){const ul=document.createElement("ul");for(const candidate of data.candidates){const li=document.createElement("li");const unavailable=candidate.binding_id===data.unavailable_binding_id,selected=candidate.binding_id===data.selected_binding_id;li.textContent=`${unavailable?"unavailable — ":selected?"selected — ":"standby — "}${candidate.order}. ${providers.get(candidate.provider)} · ${candidate.binding_id} · ${candidate.provider_instance_id}`;if(unavailable)li.className="unavailable";if(selected)li.className="selected-route";ul.append(li);}container.append(ul);} cards.append(grid);}
  const hosts=document.querySelector("#topology ul");hosts.replaceChildren();for(const host of snapshot.topology?.hosts??[]){const li=document.createElement("li");li.textContent=`host ${host.host_id} · boot ${host.boot_id} · ${host.state}`;hosts.append(li);}
  const linear=document.querySelector("#linear ol");linear.replaceChildren();for(const line of snapshot.linear){const li=document.createElement("li");li.textContent=line;linear.append(li);}
  select(snapshot.document.selection?.identity ?? form.items[0]?.identity);
}

async function load() {
  try { const response=await fetch("/api/snapshot",{cache:"no-store"}); if(!response.ok)throw new Error(`HTTP ${response.status}`); render(requireSnapshot(await response.json())); }
  catch(error){ document.querySelector("#status").textContent=state.snapshot?`Renderer disconnected; retained revision ${state.snapshot.revision}`:`Snapshot unavailable: ${error.message}`; }
}

load(); window.addEventListener("online",load); window.patchbayReload=load;
document.querySelector("#zoom-in").onclick=()=>{state.zoom=Math.min(2,state.zoom+.2);applyViewport();};
document.querySelector("#zoom-out").onclick=()=>{state.zoom=Math.max(.5,state.zoom-.2);applyViewport();};
document.querySelector("#pan-right").onclick=()=>{state.panX=Math.min(300,state.panX+40);applyViewport();};
document.querySelector("#arrange").onclick=event=>{state.reversed=!state.reversed;event.currentTarget.setAttribute("aria-pressed",String(state.reversed));renderGraph(openForm());select(state.selected);};
document.querySelector("#theme").onclick=event=>{const active=document.body.classList.toggle("light");event.currentTarget.setAttribute("aria-pressed",String(active));event.currentTarget.textContent=active?"Dark theme":"Light theme";};
