import { joinBrowserBody } from "/assets/browser-membership.js";

const schema = "conduit.patchbay.portable-presentation";
const bases = new Map([
  ["Local", "local"], ["InMemory", "in-memory"],
  ["FixtureFrame", "fixture frame"], ["FixtureDatagram", "fixture datagram"],
  ["WebSocket", "WebSocket"], ["UsbCdc", "USB CDC"],
]);
const state = { snapshot:null, selected:null, selectedPart:null, selectedCandidate:null, lens:"world", zoom:1, panX:0, reversed:false, graphHeight:360 };

function requireSnapshot(value) {
  const presentation=value?.presentation;
  if (!value || value.schema!==schema || !Number.isSafeInteger(value.revision) || value.revision!==presentation?.revision) throw new Error("unsupported snapshot schema");
  if (!presentation.basis || typeof presentation.identity!=="string" || !Array.isArray(presentation.subjects) || !Array.isArray(presentation.relationships) || !Array.isArray(presentation.properties) || !Array.isArray(presentation.text) || !value.renderer?.plan || !value.renderer?.manifestation || value.entrance?.body_id!==presentation.basis.body_id || value.entrance?.presentation_id!==presentation.identity) throw new Error("malformed portable renderer execution");
  if(value.parts&&(!Array.isArray(value.parts.parts)||!Array.isArray(value.parts.wants_to_join)||!Array.isArray(value.parts.actions)||value.parts.body_id!==presentation.basis.body_id))throw new Error("malformed canonical Parts projection");
  for (const property of presentation.properties) {
    const base=property.value?.ConnectionBase;
    if (base!==undefined && !bases.has(base)) throw new Error("unsupported connection base");
  }
  return value;
}

function term(dl,name,value){const dt=document.createElement("dt"),dd=document.createElement("dd");dt.textContent=name;dd.textContent=value??"not present";dl.append(dt,dd);}
function subjects(role){return state.snapshot.presentation.subjects.filter(subject=>!role||subject.role===role);}
function texts(identity){return state.snapshot.presentation.text.filter(item=>item.subject===identity).map(item=>item.text);}
function properties(identity){return state.snapshot.presentation.properties.filter(item=>item.subject===identity);}
function property(identity,name){return properties(identity).find(item=>item.name===name);}
function propertyText(value){
  if(value.Identity!==undefined)return value.Identity;
  if(value.Text!==undefined)return value.Text;
  if(value.Count!==undefined)return String(value.Count);
  if(value.Flag!==undefined)return String(value.Flag);
  if(value.ConnectionBase!==undefined)return bases.get(value.ConnectionBase);
  return "unsupported";
}

function displaySelection(identity){
  state.selected=identity;
  document.querySelectorAll("[data-subject]").forEach(item=>{const selected=item.dataset.subject===identity;item.classList.toggle("selected",selected);if(item.tagName==="BUTTON")item.setAttribute("aria-pressed",String(selected));});
  const subject=state.snapshot.presentation.subjects.find(item=>item.identity===identity);
  const summary=document.querySelector("#inspector .selected-summary"),exact=document.querySelector("#inspector .exact-selection"),exactFacts=exact.querySelector("dl");summary.replaceChildren();exactFacts.replaceChildren();
  if(!subject){exact.hidden=true;document.querySelector("#inspector .inspector-hint").textContent="Select a Gear, Port, or Cord. Selection owns detail.";return;}
  exact.hidden=false;document.querySelector("#inspector .inspector-hint").textContent=subject.accessibility_name;term(summary,"Meaning",subject.label);term(summary,"Subject",subject.role);
  const selectedProperties=properties(identity),visible=selectedProperties.filter(item=>lensProperty(state.lens,item.name));for(const item of visible){const name=item.name.startsWith("authored-control-")?"Authored configuration":item.name;term(summary,name,propertyText(item.value));}
  if(state.lens==="signs"){const signs=selectedProperties.filter(item=>item.name.startsWith("sign-"));term(summary,"Evidence",signs.length?`${signs.length} subject-specific causal Sign${signs.length===1?"":"s"}`:"No subject-specific Signs; Plan-level evidence remains below");}
  if(!visible.length&&state.lens!=="form"&&state.lens!=="signs")term(summary,"Layer",`No ${state.lens} facts for this subject; semantic selection retained`);
  term(exactFacts,"Presentation subject",identity);for(const item of selectedProperties.filter(item=>item.name==="semantic-id"||item.name.endsWith("-id")||item.name.startsWith("sign-")))term(exactFacts,item.name,propertyText(item.value));
  const manifestation=state.snapshot.renderer.manifestation;term(exactFacts,"Body",state.snapshot.presentation.basis.body_id);term(exactFacts,"Wake",state.snapshot.presentation.basis.wake_id);term(exactFacts,"Source Plan",state.snapshot.presentation.basis.plan_id);term(exactFacts,"Source Play",state.snapshot.presentation.basis.active_play_id);term(exactFacts,"Renderer Plan",manifestation.plan_id);term(exactFacts,"Renderer Play",manifestation.active_play_id);term(exactFacts,"Manifestation",manifestation.manifestation_id);term(exactFacts,"Manifestation lifecycle",manifestation.lifecycle);
}
function lensProperty(lens,name){if(lens==="world")return ["body-id","part-id","candidate-id","membership-state","current","offer-generation","profile-id","capability-count","resource-count","planner-capability-count","capability-id","kind-id","operational-state","availability","freshness","line-id","binding-id","source-host-id","source-boot-id","sink-host-id","sink-boot-id","base","in-plan","playing"].includes(name)||name.startsWith("resource-")||name.startsWith("maximum-");if(lens==="form")return !["plan-id","plan-status","realization-layer","placement-id","host-id","boot-id","implementation-id","artifact-id","admitted-capacity","active-play-id","play-state","pressure","line-id","line","base","base-instance-id"].includes(name)&&!name.startsWith("resource-")&&!name.startsWith("sign-");if(lens==="plan")return ["plan-status","realization-layer","placement-id","host-id","boot-id","implementation-id","artifact-id","admitted-capacity","line-id","line","base","base-instance-id"].includes(name)||name.startsWith("resource-");if(lens==="play")return ["active-play-id","play-state","pressure"].includes(name);return false;}

async function dispatchInteraction(input){
  const response=await fetch("/api/interaction",{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify({presentation_id:state.snapshot.presentation.identity,...input})});
  if(!response.ok)throw new Error(`interaction delivery HTTP ${response.status}`);
  const next=requireSnapshot(await response.json());render(next);return next;
}
function select(identity){return dispatchInteraction({kind:"select",subject:identity});}

async function dispatchFrontDoorAction(action){
  const feedback=document.querySelector("#front-door-feedback"),controls=document.querySelector("#front-door-actions");feedback.textContent=`${action} pending…`;controls.setAttribute("aria-busy","true");
  try{const next=await dispatchInteraction({kind:"invoke",action:action.toLowerCase(),target:state.snapshot.presentation.basis.expanded_form_id});feedback.textContent=`${action} ${next.interaction.last_disposition}`;}
  catch(error){feedback.textContent=`${action} failed without closing Patchbay: ${error.message}`;}
  finally{controls.removeAttribute("aria-busy");}
}

const partsActionLabels={Inspect:"Inspect",Admit:"Admit",Refuse:"Refuse",Revoke:"Revoke",SpawnBrowserPart:"+ Browser Part",Replan:"Plan again"};
async function dispatchPartsAction(action,target){
  const feedback=document.querySelector("#parts-feedback");feedback.textContent=`${partsActionLabels[action]??action} pending…`;feedback.dataset.disposition="pending";document.querySelector("#parts").setAttribute("aria-busy","true");
  try{
    const response=await fetch("/api/parts-interaction",{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify({presentation_id:state.snapshot.presentation.identity,body_id:state.snapshot.parts.body_id,action,target})});
    if(!response.ok)throw new Error(`Parts interaction delivery HTTP ${response.status}`);
    render(requireSnapshot(await response.json()));
  }catch(error){feedback.textContent=`Parts action failed without closing Patchbay: ${error.message}`;feedback.dataset.disposition="failed";}
  finally{document.querySelector("#parts").removeAttribute("aria-busy");}
}
function partsButton(action,target){const button=document.createElement("button");button.type="button";button.textContent=partsActionLabels[action]??action;button.dataset.partsAction=action;button.onclick=()=>dispatchPartsAction(action,target);return button;}
function renderParts(){
  const section=document.querySelector("#parts"),view=state.snapshot.parts;
  section.hidden=!view;if(!view)return;
  document.querySelector("#parts-title").textContent=`Parts · ${shortId(view.body_id)}`;document.querySelector("#parts-lifecycle").textContent=view.awake?"AWAKE":"LULLED";
  const notice=document.querySelector("#parts-possibilities");notice.hidden=!view.new_realization_possibilities;notice.textContent=view.new_realization_possibilities?"New realization possibilities are available. The current Plan remains unchanged until Plan again is explicitly requested.":"";
  const parts=document.querySelector("#part-rows");parts.replaceChildren();
  for(const row of view.parts){const li=document.createElement("li"),summary=document.createElement("div"),stateText=document.createElement("strong"),badges=document.createElement("div"),actions=document.createElement("div");li.className="parts-row";summary.textContent=row.label;stateText.className="parts-row-state";stateText.textContent=`${row.state.toUpperCase()} · ${row.available?"AVAILABLE":"OFFLINE"}`;badges.className="parts-badges";for(const label of [row.in_plan?"IN PLAN":null,row.playing?"PLAYING":null].filter(Boolean)){const badge=document.createElement("span");badge.className="parts-badge";badge.textContent=label;badges.append(badge);}summary.append(badges);actions.className="parts-row-actions";for(const action of row.actions)actions.append(partsButton(action,row.details.part_id));li.append(summary,stateText,actions);parts.append(li);}
  const candidates=document.querySelector("#candidate-rows");candidates.replaceChildren();
  if(!view.wants_to_join.length){const li=document.createElement("li");li.textContent="No candidates currently want to join.";candidates.append(li);}
  for(const row of view.wants_to_join){const li=document.createElement("li"),summary=document.createElement("div"),stateText=document.createElement("strong"),actions=document.createElement("div");li.className="parts-row";summary.textContent=row.label;stateText.className="parts-row-state";stateText.textContent=`${row.state.replace(/([A-Z])/g," $1").trim().toUpperCase()} · AVAILABLE`;actions.className="parts-row-actions";for(const action of row.actions)actions.append(partsButton(action,row.candidate_id));li.append(summary,stateText,actions);candidates.append(li);}
  const toolbar=document.querySelector("#parts-actions");toolbar.replaceChildren();for(const action of view.actions)toolbar.append(partsButton(action,view.body_id));
  const details=document.querySelector("#parts-details dl");details.replaceChildren();const selectedPart=view.parts.find(row=>row.details.part_id===state.snapshot.interaction.selected_part),selectedCandidate=view.wants_to_join.find(row=>row.candidate_id===state.snapshot.interaction.selected_candidate);
  if(selectedPart){const d=selectedPart.details;term(details,"Part",d.part_id);term(details,"Host",d.host_id);term(details,"Boot",d.boot_id);term(details,"Offer generation",d.offer_generation);term(details,"Capabilities",d.capabilities.map(item=>`${item.kind_id} (${item.capability_id})`).join(", ")||"none");term(details,"Admission proof",d.proof_reference);term(details,"Plan placements",d.planned_placements.join(", ")||"none");term(details,"Authority bindings",d.planned_authority_bindings);term(details,"Expected Signs",d.expected_signs);}
  else if(selectedCandidate){term(details,"Candidate",selectedCandidate.candidate_id);term(details,"Host",selectedCandidate.host_id);term(details,"Boot",selectedCandidate.boot_id);term(details,"Offer generation",selectedCandidate.offer_generation);term(details,"Capabilities",selectedCandidate.capability_offers.map(item=>`${item.kind_id} (${item.capability_id})`).join(", ")||"none");}
  else term(details,"Selection","Inspect a Part or candidate to disclose exact facts.");
  const feedback=document.querySelector("#parts-feedback");feedback.textContent=state.snapshot.interaction.parts_feedback??"No Parts action requested.";feedback.dataset.disposition=(state.snapshot.interaction.parts_disposition??"").toLowerCase();
}

function applyViewport(){const width=740/state.zoom,height=state.graphHeight/state.zoom;document.querySelector("#graph").setAttribute("viewBox",`${state.panX} 0 ${width} ${height}`);}
function shortId(value){return value&&value!=="unsupported"?`${value.slice(0,10)}…`:"not present";}
function renderRouteRecovery(svg,routes,startY){
  let y=startY;
  for(const route of routes){
    const candidates=state.snapshot.presentation.relationships.filter(item=>item.source===route.identity&&item.kind==="Contains").map(item=>state.snapshot.presentation.subjects.find(subject=>subject.identity===item.target)).filter(item=>item?.role==="Cord");
    const group=svgElement("g",{class:"route-recovery",role:"button",tabindex:"0","aria-label":route.accessibility_name});group.dataset.subject=route.identity;
    group.append(svgElement("rect",{x:"35",y,width:"670",height:"176",rx:"12",class:"route-recovery-frame"}));
    const heading=svgElement("text",{x:"55",y:y+24,class:"route-recovery-title"});heading.textContent=`ROUTE RECOVERY · connection ${shortId(route.label)}`;group.append(heading);
    const prior=shortId(propertyText(property(route.identity,"new-plan-prior-id")?.value??{})),replacement=shortId(propertyText(property(route.identity,"new-plan-replacement-id")?.value??{})),samePlan=shortId(propertyText(property(route.identity,"same-plan-id")?.value??{}));
    const newPlanLabel=svgElement("text",{x:"55",y:y+51,class:"route-recovery-label"});newPlanLabel.textContent=`Plan ${prior}  UNSATISFIED  →  replacement Plan ${replacement}`;group.append(newPlanLabel);
    const samePlanLabel=svgElement("text",{x:"55",y:y+111,class:"route-recovery-label"});samePlanLabel.textContent=`Plan ${samePlan}  admitted alternative continuation`;
    group.append(samePlanLabel);
    const priorCandidates=candidates.filter(item=>propertyText(property(item.identity,"phase")?.value??{})==="prior"),samePlanCandidates=candidates.filter(item=>propertyText(property(item.identity,"phase")?.value??{})==="same-plan");
    const drawCandidate=(candidate,index,rowY)=>{const status=propertyText(property(candidate.identity,"route-status")?.value??{}),base=propertyText(property(candidate.identity,"base")?.value??{}),candidateGroup=svgElement("g",{class:`route-candidate status-${status}`,role:"button",tabindex:"0","aria-label":`${candidate.accessibility_name}; ${status}`});candidateGroup.dataset.subject=candidate.identity;const x=70+index*285;candidateGroup.append(svgElement("line",{x1:x,y1:rowY,x2:x+235,y2:rowY,class:"route-candidate-line"}));const marker=svgElement("text",{x:x+117,y:rowY-8,class:"route-candidate-marker","text-anchor":"middle"});marker.textContent=status==="unavailable"?`× ${base} · LINE LOST`:status==="selected"?`● ${base} · SELECTED`:`○ ${base} · ADMITTED`;candidateGroup.append(marker);activate(candidateGroup,candidate.identity);group.append(candidateGroup);};
    priorCandidates.forEach((candidate,index)=>drawCandidate(candidate,index,y+75));samePlanCandidates.forEach((candidate,index)=>drawCandidate(candidate,index,y+139));
    activate(group,route.identity);svg.append(group);y+=196;
  }
  return y;
}
function renderGraph(){
  const svg=document.querySelector("#graph");svg.replaceChildren(svg.querySelector("title"));
  let gears=subjects("Gear");if(state.reversed)gears=[...gears].reverse();const ports=subjects("Port"),cords=subjects("Cord").filter(item=>property(item.identity,"source-port")),routes=subjects("Route"),diagnostics=subjects("Diagnostic");
  const routingTop=55,gearTop=routingTop+cords.length*10+20;
  const positions=new Map();gears.forEach((gear,index)=>positions.set(gear.identity,{x:35+(index%3)*235,y:gearTop+Math.floor(index/3)*190}));
  const semanticToSubject=new Map([...gears,...ports,...cords].map(item=>[propertyText(property(item.identity,"semantic-id")?.value??{}),item.identity]));const portPoints=new Map();
  for(const gear of gears){const position=positions.get(gear.identity),gearPorts=state.snapshot.presentation.relationships.filter(item=>item.source===gear.identity&&item.kind==="Contains").map(item=>state.snapshot.presentation.subjects.find(subject=>subject.identity===item.target)).filter(item=>item?.role==="Port"),receiving=gearPorts.filter(item=>propertyText(property(item.identity,"direction")?.value)==="receiving"),outgoing=gearPorts.filter(item=>propertyText(property(item.identity,"direction")?.value)==="outgoing");receiving.forEach((port,index)=>portPoints.set(port.identity,{x:position.x,y:position.y+70+index*24}));outgoing.forEach((port,index)=>portPoints.set(port.identity,{x:position.x+200,y:position.y+70+index*24}));}
  for(const cord of cords){
    const source=semanticToSubject.get(propertyText(property(cord.identity,"source-port")?.value??{})),sink=semanticToSubject.get(propertyText(property(cord.identity,"sink-port")?.value??{})),from=portPoints.get(source),to=portPoints.get(sink);if(!from||!to)continue;
    const laneY=routingTop+cords.indexOf(cord)*10,sourceOutside=from.x+12,sinkOutside=to.x-12;
    const path=`M ${from.x} ${from.y} H ${sourceOutside} V ${laneY} H ${sinkOutside} V ${to.y} H ${to.x}`;
    const group=svgElement("g",{class:"cord",role:"button",tabindex:"0","aria-label":cord.accessibility_name});group.dataset.subject=cord.identity;group.append(svgElement("path",{d:path,class:"relationship cord-line",fill:"none"}));
    const x=(sourceOutside+sinkOutside)/2,y=laneY-5,overlays=[
      ["plan",propertyText(property(cord.identity,"line")?.value??property(cord.identity,"line-id")?.value??{})],
      ["play",propertyText(property(cord.identity,"play-state")?.value??{})],
      ["signs",`${properties(cord.identity).filter(item=>item.name.startsWith("sign-")).length} causal Signs`],
    ];
    for(const [lens,value] of overlays){const overlay=svgElement("text",{x,y,class:`lens-overlay cord-overlay ${lens}-overlay`,"text-anchor":"middle"});overlay.textContent=value.slice(0,30);group.append(overlay);}activate(group,cord.identity);svg.append(group);
  }
  for(const gear of gears){const position=positions.get(gear.identity),group=svgElement("g",{class:"gear",role:"button",tabindex:"0","aria-label":gear.accessibility_name});group.dataset.subject=gear.identity;const rect=svgElement("rect",{x:position.x,y:position.y,width:"200",height:"145",rx:"12",class:"node gear-face"}),iconToken=propertyText(property(gear.identity,"icon-token")?.value??{}),iconName=propertyText(property(gear.identity,"icon-name")?.value??{}),icon=svgElement("text",{x:position.x+100,y:position.y+38,class:"gear-icon","text-anchor":"middle","aria-label":iconName});icon.textContent=iconGlyph(iconToken);const label=svgElement("text",{x:position.x+100,y:position.y+60,class:"node-label gear-label","text-anchor":"middle"});label.textContent=gear.label;group.append(rect,icon,label);const authored=properties(gear.identity).find(item=>item.name.startsWith("authored-control-"));if(authored){const control=svgElement("text",{x:position.x+100,y:position.y+85,class:"authored-control","text-anchor":"middle"});control.textContent=`authored · ${propertyText(authored.value)}`.slice(0,30);group.append(control);}for(const [lens,value] of [["plan",`${propertyText(property(gear.identity,"host-id")?.value??{})} · ${propertyText(property(gear.identity,"realization-layer")?.value??{})}`],["play",`${propertyText(property(gear.identity,"play-state")?.value??{})} · ${propertyText(property(gear.identity,"pressure")?.value??{})}`],["signs",`${properties(gear.identity).filter(item=>item.name.startsWith("sign-")).length} causal Signs`]]){const overlay=svgElement("text",{x:position.x+100,y:position.y+112,class:`lens-overlay ${lens}-overlay`,"text-anchor":"middle"});overlay.textContent=value.slice(0,34);group.append(overlay);}activate(group,gear.identity);svg.append(group);for(const port of ports.filter(item=>portPoints.has(item.identity)&&state.snapshot.presentation.relationships.some(relation=>relation.source===gear.identity&&relation.target===item.identity))){const point=portPoints.get(port.identity),direction=propertyText(property(port.identity,"direction")?.value),portGroup=svgElement("g",{class:`port ${direction}`,role:"button",tabindex:"0","aria-label":port.accessibility_name});portGroup.dataset.subject=port.identity;const anchor=svgElement("circle",{cx:point.x,cy:point.y,r:"7",class:"port-anchor"}),portLabel=svgElement("text",{x:point.x+(direction==="receiving"?12:-12),y:point.y+4,class:"port-label","text-anchor":direction==="receiving"?"start":"end"});portLabel.textContent=direction==="receiving"?`> ${port.label}`:`${port.label} >`;portGroup.append(anchor,portLabel);activate(portGroup,port.identity);svg.append(portGroup);}}
  diagnostics.forEach((diagnostic,index)=>{const group=svgElement("g",{class:"diagnostic-overlay",role:"button",tabindex:"0","aria-label":diagnostic.accessibility_name});group.dataset.subject=diagnostic.identity;const x=545,y=12+index*32;group.append(svgElement("rect",{x,y,width:"160",height:"24",rx:"12",class:"diagnostic-badge"}));const label=svgElement("text",{x:x+80,y:y+17,class:"diagnostic-label","text-anchor":"middle"});label.textContent=`diagnostic · ${diagnostic.label}`;group.append(label);activate(group,diagnostic.identity);svg.append(group);});
  const routeStart=Math.max(365,75+Math.ceil(gears.length/3)*190),routeEnd=renderRouteRecovery(svg,routes,routeStart);state.graphHeight=Math.max(360,routeEnd+20);applyViewport();displaySelection(state.selected);
}

function svgElement(name,attributes){const element=document.createElementNS("http://www.w3.org/2000/svg",name);for(const [key,value] of Object.entries(attributes))element.setAttribute(key,String(value));return element;}
function activate(element,identity){element.onclick=event=>{event.stopPropagation();select(identity);};element.onkeydown=event=>{if(event.key==="Enter"||event.key===" "){event.preventDefault();select(identity);}};}
function iconGlyph(token){return ({"case-upper":"Aa","presentation":"▣","type":"T","clock":"◷","repeat-2":"↻","combine":"⋈","keyboard":"⌨","file-output":"⇲","conduit-generic-gear":"⚙"})[token]??"◆";}
function selectLens(lens){state.lens=lens;document.body.dataset.lens=lens;document.querySelector("#lens-label").textContent=`${lens.toUpperCase()} LENS`;document.querySelectorAll("[data-lens]").forEach(button=>button.setAttribute("aria-pressed",String(button.dataset.lens===lens)));renderGraph();displaySelection(state.selected);}

function fillLines(selector,items){const list=document.querySelector(selector);list.replaceChildren();for(const value of items){const li=document.createElement("li");li.textContent=value;list.append(li);}}
function renderCards(){const cards=document.querySelector("#route-cards");cards.replaceChildren();for(const route of subjects("Route")){const article=document.createElement("article"),heading=document.createElement("h3");article.className="route-card";heading.textContent=`Route ${route.label}`;article.append(heading);for(const line of texts(route.identity)){const p=document.createElement("p");p.textContent=line;article.append(p);}const children=state.snapshot.presentation.relationships.filter(item=>item.source===route.identity&&item.kind==="Contains").map(item=>item.target);const ul=document.createElement("ul");for(const identity of children){const candidate=state.snapshot.presentation.subjects.find(item=>item.identity===identity);if(!candidate)continue;const li=document.createElement("li");li.textContent=[candidate.label,...properties(identity).map(item=>`${item.name}=${propertyText(item.value)}`)].join(" · ");ul.append(li);}article.append(ul);cards.append(article);}}

function render(snapshot){
  const entering=state.snapshot===null;if(entering)state.lens=({World:"world",Intent:"form",Realization:"plan"})[snapshot.entrance.layer]??state.lens;state.snapshot=snapshot;const p=snapshot.presentation,b=p.basis,renderer=snapshot.renderer,manifestation=renderer.manifestation;
  document.querySelector("#status").textContent=`Presentation revision ${p.revision} · content ${p.identity} · Manifestation ${manifestation.lifecycle} · read-only`;
  document.querySelector("#run-summary").textContent=`${manifestation.lifecycle} · Plan ${b.plan_id} · Play ${b.active_play_id}`;
  document.querySelector("#plan-form").disabled=b.plan_id!==null;document.querySelector("#play-plan").disabled=b.plan_id===null||b.active_play_id!==null;
  const facts=document.querySelector("#form-facts");facts.replaceChildren();term(facts,"Seed",b.seed_id);term(facts,"Body",b.body_id);term(facts,"Wake",b.wake_id);term(facts,"Source document",b.source_document_id);term(facts,"Checked Form",b.checked_form_id);
  const list=document.querySelector("#subjects");list.replaceChildren();for(const subject of p.subjects){const li=document.createElement("li"),button=document.createElement("button");button.type="button";button.dataset.subject=subject.identity;button.dataset.role=subject.role;button.setAttribute("aria-pressed","false");button.textContent=`${subject.role}: ${subject.accessibility_name}`;button.onclick=()=>select(subject.identity);li.append(button);list.append(li);}renderParts();renderGraph();
  const placements=renderer.plan.fragments.flatMap(fragment=>fragment.placements);const placement=placements.find(item=>item.placement_id===manifestation.placement_id);const connections=[...new Map(renderer.plan.fragments.flatMap(fragment=>fragment.connections).map(connection=>[connection.connection_id,connection])).values()];
  const plan=document.querySelector("#plan dl");plan.replaceChildren();term(plan,"Expanded Form",b.expanded_form_id);term(plan,"Source Plan",b.plan_id);term(plan,"Renderer Face",placement?.kind_id);term(plan,"Renderer Plan",manifestation.plan_id);term(plan,"Renderer Play",manifestation.active_play_id);term(plan,"Manifestation",manifestation.manifestation_id);term(plan,"Lifecycle",manifestation.lifecycle);term(plan,"Placement",placement?.placement_id);term(plan,"Host",placement?.host_id);term(plan,"Boot",placement?.boot_id);term(plan,"Implementation",placement?.implementation_id);term(plan,"Artifact",placement?.artifact_id);term(plan,"Execution profile",placement?.execution_profile_id);term(plan,"Offer generation",placement?.offer_generation);term(plan,"Limits",placement?`active=${placement.limits.max_active_instances} queue-items=${placement.limits.max_queue_items} queue-bytes=${placement.limits.max_queue_bytes}`:undefined);fillLines("#realizations",[...placements.flatMap(item=>[`${item.gear_id} · host ${item.host_id} · boot ${item.boot_id} · implementation ${item.implementation_id} · artifact ${item.artifact_id}`,...item.inputs.concat(item.outputs).map(port=>`Port ${port.port_id} · ${port.direction} · Info ${port.value_kind} · ${port.temporal}`),...item.resources.map(resource=>`Resource ${resource.pool_id} · class ${resource.class_id} · units ${resource.units}`),...item.host_operations.map(operation=>`Base ${operation.contract_id} · target ${operation.target_kind??"not present"} · in-flight ${operation.maximum_in_flight} · input-bytes ${operation.maximum_input_bytes} · output-bytes ${operation.maximum_output_bytes}`)]),...connections.map(connection=>{const line=connection.selected_line,binding=line?.binding;return `Cord ${connection.connection_id} · ${connection.source_port_id} -> ${connection.sink_port_id} · Info ${connection.value_kind} · Line ${line?.line_id??"not present"} · base ${bases.get(binding?.base)??"not present"} · binding ${binding?.binding_id??"not present"} · base-instance ${binding?.base_instance_id??"not present"}`;})]);
  const play=document.querySelector("#play dl");play.replaceChildren();term(play,"Active Play",b.active_play_id);term(play,"Plan",b.plan_id);fillLines("#sign",[...subjects("Sign").map(subject=>subject.label),...manifestation.signs.map(sign=>`Renderer ${sign.sign_id} · ${sign.lifecycle}`)]);
  const interaction=document.querySelector("#interaction-proof");interaction.replaceChildren();term(interaction,"Interaction revision",snapshot.interaction.revision);term(interaction,"Request",snapshot.interaction.last_request_id);term(interaction,"Disposition",snapshot.interaction.last_disposition);term(interaction,"Interaction Plan",snapshot.interaction.interaction_plan_id);term(interaction,"Interaction Play",snapshot.interaction.interaction_play_id);
  const diagnosticLines=subjects("Diagnostic").flatMap(subject=>texts(subject.identity));
  fillLines("#diagnostics ol",diagnosticLines);document.querySelector("#diagnostic-summary").textContent=diagnosticLines.length?`${diagnosticLines.length} checked diagnostic`:"No checked diagnostics";renderCards();fillLines("#topology ul",subjects().filter(subject=>["Body","Part","Candidate","Host","Capability","Line"].includes(subject.role)).flatMap(subject=>[`${subject.role}: ${subject.accessibility_name}`,...texts(subject.identity)]));fillLines("#linear ol",p.text.map(item=>item.text));displaySelection(snapshot.entrance.selected_subject);
}

async function load(){try{const response=await fetch("/api/snapshot",{cache:"no-store"});if(!response.ok)throw new Error(`HTTP ${response.status}`);const snapshot=requireSnapshot(await response.json());if(state.snapshot&&snapshot.revision<state.snapshot.revision)return;render(snapshot);}catch(error){document.querySelector("#status").textContent=state.snapshot?`Renderer disconnected; retained revision ${state.snapshot.revision}`:`Snapshot unavailable: ${error.message}`;}}
async function joinCurrentBody(){
  const response=await fetch("/api/body-admission",{cache:"no-store"});
  if(response.status===404)return;
  if(!response.ok)throw new Error(`Body admission HTTP ${response.status}`);
  const {url}=await response.json();
  const wasm=await fetch("/assets/conduit-browser-runtime.wasm",{cache:"no-store"});
  if(!wasm.ok)throw new Error(`browser Host runtime HTTP ${wasm.status}`);
  window.__patchbayMembership=await joinBrowserBody({bodyUrl:url,wasmBytes:await wasm.arrayBuffer(),onState:()=>load()});
}
document.body.dataset.lens=state.lens;document.querySelectorAll("[data-lens]").forEach(button=>button.onclick=()=>selectLens(button.dataset.lens));load().then(()=>joinCurrentBody()).catch(error=>{document.querySelector("#status").textContent=`Browser Host admission unavailable: ${error.message}`;});window.addEventListener("online",load);window.setInterval(load,250);window.patchbayReload=load;
document.querySelector("#zoom-in").onclick=()=>{state.zoom=Math.min(2,state.zoom+.2);applyViewport();};document.querySelector("#zoom-out").onclick=()=>{state.zoom=Math.max(.5,state.zoom-.2);applyViewport();};document.querySelector("#pan-right").onclick=()=>{state.panX=Math.min(300,state.panX+40);applyViewport();};document.querySelector("#arrange").onclick=event=>{state.reversed=!state.reversed;event.currentTarget.setAttribute("aria-pressed",String(state.reversed));renderGraph();displaySelection(state.selected);};document.querySelector("#theme").onclick=event=>{const active=document.body.classList.toggle("high-contrast");event.currentTarget.setAttribute("aria-pressed",String(active));event.currentTarget.textContent=active?"Standard contrast":"High contrast";};document.querySelector("#toggle-linear").onclick=()=>dispatchInteraction({kind:"invoke",action:"toggle-linear-view",target:state.snapshot.presentation.basis.expanded_form_id});
document.querySelector("#plan-form").onclick=()=>dispatchFrontDoorAction("Plan");document.querySelector("#play-plan").onclick=()=>dispatchFrontDoorAction("Play");
