import { joinBrowserBody } from "/assets/browser-membership.js";
import { createApplicationPresentationHost } from "/assets/application-presentation.mjs";
import { arrangeFlow, configureFlowStorage, fitFlow, flowStorageSettled, flowViewport, focusFlow, panFlow, renderFlow, zoomFlow } from "/assets/flow.js";
import { installPanelFurniture } from "/assets/panel-furniture.js";
import { lensForCursor, projectCurrent } from "/assets/portable-navigation.js";
import { BrowserWebSocketLine } from "/assets/websocket-line.mjs";
import { instantiateTextLabLive, runTextLabLive } from "/assets/text-lab-live-runtime.mjs";

const schema = "conduit.patchbay.portable-presentation";
const state = { snapshot:null, projected:null, selected:null, selectedPart:null, selectedCandidate:null, seedQuery:"", gearQuery:"", authoringValues:new Map(), cordSource:null, rerouteCord:null, lens:"world", inspectorOpen:false, inspectorDepth:false, inspectorTransition:null, truthTransition:null };
const applicationPresentation = createApplicationPresentationHost();
let admittedRuntimeBytes = null;
let statusRevision = 0;
const sharedRevisions = new Map();
function presentDefinitions(slot, entries) {
  const revision = (sharedRevisions.get(slot) ?? 0) + 1;
  sharedRevisions.set(slot, revision);
  applicationPresentation.present(slot, {
    revision,
    actions: [],
    nodes: [
      { parent: null, component: "definition-table", key: "facts", text: "Exact facts", action: null },
      ...entries.map(([name, value], index) => ({ parent: 0, component: "definition", key: `fact-${index}`, text: name, value: String(value ?? "not present"), valueCapacity: 65_536, action: null })),
    ],
  });
}
function presentActions(slot, entries) {
  const revision = (sharedRevisions.get(slot) ?? 0) + 1;
  sharedRevisions.set(slot, revision);
  applicationPresentation.present(slot, {
    revision,
    actions: entries.map((_, index) => ({ id: `action-${index}`, event: "activate" })),
    nodes: [
      { parent: null, component: "action-group", key: "actions", text: "Available actions", action: null },
      ...entries.map((entry, index) => ({ parent: 0, component: "button", key: `action-${index}`, text: entry.label, state: entry.disabled ? "unavailable" : "ready", action: entry.disabled ? null : index })),
    ],
  }, { onEvent(event) {
    applicationPresentation.nextEvent(slot);
    entries[Number(event.action.slice("action-".length))]?.run();
  } });
}
function presentSharedStatus(slot, text, component = "status") {
  const revision = (sharedRevisions.get(slot) ?? 0) + 1;
  sharedRevisions.set(slot, revision);
  applicationPresentation.present(slot, { revision, actions: [], nodes: [{ parent: null, component, key: "status", text, action: null }] });
}
function presentStatus(text, component = "status") {
  applicationPresentation.present("patchbay-status", {
    revision: ++statusRevision,
    actions: [],
    nodes: [{ parent: null, component, key: "renderer-status", text, action: null }],
  });
}

function requireSnapshot(value) {
  const presentation=value?.presentation;
  if (!value || value.schema!==schema || !Number.isSafeInteger(value.revision) || value.revision!==presentation?.revision) throw new Error("unsupported snapshot schema");
  if (!presentation.basis || typeof presentation.identity!=="string" || !Array.isArray(presentation.subjects) || !Array.isArray(presentation.relationships) || !Array.isArray(presentation.properties) || !Array.isArray(presentation.text) || !Array.isArray(presentation.actions) || !Array.isArray(presentation.disclosures) || !Array.isArray(value.temporal_context) || !value.renderer?.plan || !value.renderer?.manifestation || value.entrance?.body_id!==presentation.basis.body_id || value.entrance?.presentation_id!==presentation.identity) throw new Error("malformed portable renderer execution");
  for(const fact of value.temporal_context){if(typeof fact.relative_time!=="string"||typeof fact.role!=="string"||typeof fact.subject!=="string"||typeof fact.reference?.identity!=="string"||typeof fact.reference?.instant?.clock_basis!=="string"||typeof fact.source?.clock_basis!=="string"||!presentation.subjects.some(subject=>subject.identity===fact.subject))throw new Error("malformed temporal context");}
  for(const action of presentation.actions){if(typeof action.identity!=="string"||typeof action.target!=="string"||typeof action.label!=="string"||!(action.availability==="Available"||action.availability?.Unavailable||action.availability?.Refused))throw new Error("malformed semantic action");}
  if(value.parts&&(!Array.isArray(value.parts.parts)||!Array.isArray(value.parts.wants_to_join)||!Array.isArray(value.parts.actions)||value.parts.body_id!==presentation.basis.body_id||!value.parts.truth_explanation||Object.values(value.parts.truth_explanation).some(explanation=>typeof explanation!=="string")))throw new Error("malformed canonical Parts projection");
  if(value.authoring&&(!Array.isArray(value.authoring.palette)||value.authoring.palette.length>69||typeof value.authoring.source_document_id!=="string"||typeof value.authoring.expanded_form_id!=="string"||!Number.isSafeInteger(value.authoring.source_revision)))throw new Error("malformed bounded authoring projection");
  if(value.watches&&(!Array.isArray(value.watches.eligible_subjects)||!Array.isArray(value.watches.watches)||value.watches.watches.length>8||!Number.isSafeInteger(value.watches.revision)))throw new Error("malformed bounded debugger Watches");
  if(value.watches?.watches.some(watch=>!Array.isArray(watch.learned_projections??[])||(watch.learned_projections??[]).length>8||(watch.learned_projections??[]).some(projection=>!Number.isSafeInteger(projection.observation_sequence)||projection.max_updates_per_second<1||projection.projection==="signal"&&(!Array.isArray(projection.detail?.points)||projection.detail.points.length>96)||projection.projection==="tensor"&&(!Array.isArray(projection.detail?.bounded_slice_milli)||projection.detail.bounded_slice_milli.length>32)||projection.projection==="probabilistic"&&(!Array.isArray(projection.detail?.alternatives)||projection.detail.alternatives.length>8)||projection.projection==="training"&&(!Array.isArray(projection.detail?.objectives)||projection.detail.objectives.length>8))))throw new Error("malformed bounded learned Watch projection");
  if((value.timeline&&!value.timeline_projection)||(!value.timeline&&value.timeline_projection)||value.timeline&&(!Array.isArray(value.timeline.events)||value.timeline.events.length>128||value.timeline.retained_bytes>65536||!Number.isSafeInteger(value.timeline.revision)||!Array.isArray(value.timeline_projection.states)||!Array.isArray(value.timeline_projection.watch_states)))throw new Error("malformed bounded debugger timeline");
  if(value.debugger_control&&(!Array.isArray(value.debugger_control.eligible_subjects)||value.debugger_control.eligible_subjects.length>16||!Number.isSafeInteger(value.debugger_control.revision)))throw new Error("malformed bounded debugger execution control");
  for (const property of presentation.properties) {
    const base=property.value?.BaseImplementationId;
    if (base!==undefined && (typeof base!=="string" || base.length===0)) throw new Error("malformed Base implementation identity");
  }
  projectCurrent(value);
  return value;
}

function term(dl,name,value,fullValue){const dt=document.createElement("dt"),dd=document.createElement("dd");dt.textContent=name;dd.textContent=value??"not present";if(fullValue&&fullValue!==value)dd.title=fullValue;dl.append(dt,dd);}
function summaryText(value){const full=propertyText(value);return value.Identity!==undefined&&full.length>38?`${full.slice(0,24)}…${full.slice(-8)}`:full;}
function subjects(role){return state.snapshot.presentation.subjects.filter(subject=>!role||subject.role===role);}
function projectedSubjects(role){return state.projected.subjects.filter(subject=>!role||subject.role===role);}
function texts(identity){return state.snapshot.presentation.text.filter(item=>item.subject===identity).map(item=>item.text);}
function properties(identity){return state.snapshot.presentation.properties.filter(item=>item.subject===identity);}
function projectedProperties(identity){return state.projected.properties.filter(item=>item.subject===identity);}
function property(identity,name){return properties(identity).find(item=>item.name===name);}
function semanticActions(identity){return state.projected.actions.filter(action=>action.target===identity);}
function currentPresentationBasis(){return {presentation_id:state.snapshot.presentation.identity,presentation_revision:state.snapshot.presentation.revision};}
function semanticIdentity(subject){return property(subject,"semantic-id")?.value?.Identity??null;}
function authoringEdit(operation,primary,extra={}){const a=state.snapshot.authoring;if(!a)throw new Error("canonical authoring is unavailable");return dispatchInteraction({kind:"edit",edit:{source_document_id:a.source_document_id,source_revision:a.source_revision,expanded_form_id:a.expanded_form_id,operation,primary,...extra}});}
function actionAvailability(action){
  if(action.availability==="Available")return {available:true,state:"Available",explanation:null};
  if(action.availability?.Unavailable)return {available:false,state:"Unavailable",explanation:action.availability.Unavailable.explanation};
  if(action.availability?.Refused)return {available:false,state:"Refused",explanation:action.availability.Refused.explanation};
  throw new Error("unsupported semantic action availability");
}
function renderSemanticActions(identity){
  const desired=semanticActions(identity),revision=(sharedRevisions.get("semantic-actions")??0)+1;sharedRevisions.set("semantic-actions",revision);
  applicationPresentation.present("semantic-actions",{
    revision,
    actions:desired.map((_,index)=>({id:`semantic-${index}`,event:"activate"})),
    nodes:[
      {parent:null,component:"action-group",key:"semantic-actions",text:"Selected subject actions",action:null},
      ...desired.flatMap((action,index)=>{const availability=actionAvailability(action);return [
        {parent:0,component:"button",key:`semantic-${index}`,text:action.label.toUpperCase(),state:availability.available?"ready":"unavailable",action:availability.available?index:null},
        {parent:0,component:availability.state==="Refused"?"refused-evidence":availability.state==="Unavailable"?"missing-evidence":"status",key:`availability-${index}`,text:availability.explanation?`${availability.state}: ${availability.explanation}`:availability.state,action:null},
      ];}),
    ],
  },{onEvent(event){applicationPresentation.nextEvent("semantic-actions");const index=Number(event.action.slice("semantic-".length));dispatchSemanticAction(desired[index]);}});
}
function propertyText(value){
  if(value.Identity!==undefined)return value.Identity;
  if(value.Text!==undefined)return value.Text;
  if(value.Count!==undefined)return String(value.Count);
  if(value.Flag!==undefined)return String(value.Flag);
  if(value.BaseImplementationId!==undefined)return value.BaseImplementationId;
  return "unsupported";
}

function displaySelection(identity){
  state.selected=identity;
  document.querySelectorAll("[data-subject]").forEach(item=>{const selected=item.dataset.subject===identity;item.classList.toggle("selected",selected);if(item.tagName==="BUTTON")item.setAttribute("aria-pressed",String(selected));});
  const subject=state.projected.subjects.find(item=>item.identity===identity);
  document.body.dataset.inspectorOpen=String(state.inspectorOpen);
  document.querySelector("#toggle-inspector").setAttribute("aria-expanded",String(state.inspectorOpen));
  const summary=document.querySelector("#inspector .selected-summary"),exact=document.querySelector("#inspector .exact-selection");
  document.querySelector("#clear-selection").hidden=!subject;
  document.querySelector("#center-flow").disabled=!subject;
  if(!subject){presentDefinitions("selection-summary",[]);presentDefinitions("selection-exact",[]);renderSemanticActions(null);renderAuthoringActions(null);renderTimeline(null);renderWatches(null);exact.hidden=true;document.querySelector("#inspector .inspector-hint").textContent="Select a Host, Body, Seed, Gear, Port, or Cord. Selection owns detail.";return;}
  const depth=state.projected.cursor?.depth??"Exact";exact.hidden=!(["Detail","Exact"].includes(depth));document.querySelector("#inspector .inspector-hint").textContent=subject.accessibility_name;
  const selectedProperties=projectedProperties(identity),visible=selectedProperties.filter(item=>lensProperty(state.lens,item.name)),summaryFacts=[["Meaning",subject.label],["Subject",subject.role],...visible.map(item=>[item.name.startsWith("authored-control-")?"Authored configuration":item.name,summaryText(item.value)])];
  if(state.lens==="signs"){const signs=selectedProperties.filter(item=>item.name.startsWith("sign-"));summaryFacts.push(["Evidence",signs.length?`${signs.length} subject-specific causal Sign${signs.length===1?"":"s"}`:"No subject-specific Signs; Plan-level evidence remains below"]);}
  if(!visible.length&&state.lens!=="form"&&state.lens!=="signs")summaryFacts.push(["Layer",`No ${state.lens} facts for this subject; semantic selection retained`]);
  presentDefinitions("selection-summary",summaryFacts);
  const exactFacts=[["Presentation subject",identity],...selectedProperties.filter(item=>item.name==="semantic-id"||item.name.endsWith("-id")||item.name.startsWith("sign-")).map(item=>[item.name,propertyText(item.value)])];
  if(depth==="Exact"){const manifestation=state.snapshot.renderer.manifestation;exactFacts.push(["Body",state.snapshot.presentation.basis.body_id],["Wake",state.snapshot.presentation.basis.wake_id],["Source Plan",state.snapshot.presentation.basis.plan_id],["Source Play",state.snapshot.presentation.basis.active_play_id],["Renderer Plan",manifestation.plan_id],["Renderer Play",manifestation.active_play_id],["Manifestation",manifestation.manifestation_id],["Manifestation lifecycle",manifestation.lifecycle]);}
  presentDefinitions("selection-exact",exactFacts);
  renderSemanticActions(identity);
  renderAuthoringActions(subject);
  renderTimeline(subject);
  renderWatches(subject);
}

function watchValue(entry){
  if(!entry)return "No retained observation";
  if(entry.fault_code!==null&&entry.fault_code!==undefined)return `Fault ${entry.fault_code}`;
  return entry.value?.summary??entry.event;
}
function milli(value){return value==null?"missing":(value/1000).toFixed(3);}
function learnedPlot(detail,label){
  const figure=document.createElement("figure"),svg=document.createElementNS("http://www.w3.org/2000/svg","svg"),caption=document.createElement("figcaption"),points=detail.points??detail.trajectory??[],present=points.filter(point=>point.value_milli!=null);figure.className="learned-plot";svg.setAttribute("viewBox","0 0 240 84");svg.setAttribute("role","img");svg.setAttribute("aria-label",label);caption.textContent=`${points.length} retained points · ${detail.evicted_points??0} evicted`;
  if(present.length){const values=present.flatMap(point=>[point.value_milli,point.lower_milli,point.upper_milli].filter(value=>value!=null)),minimum=Math.min(...values),maximum=Math.max(...values),span=Math.max(1,maximum-minimum),x=index=>10+index*Math.max(1,220/Math.max(1,points.length-1)),y=value=>74-(value-minimum)*64/span;const bands=present.filter(point=>point.lower_milli!=null&&point.upper_milli!=null);if(bands.length>1){const polygon=document.createElementNS(svg.namespaceURI,"polygon"),upper=bands.map(point=>`${x(points.indexOf(point))},${y(point.upper_milli)}`),lower=[...bands].reverse().map(point=>`${x(points.indexOf(point))},${y(point.lower_milli)}`);polygon.setAttribute("points",[...upper,...lower].join(" "));polygon.setAttribute("class","uncertainty-band");svg.append(polygon);}let segment=[];const flush=()=>{if(segment.length){const line=document.createElementNS(svg.namespaceURI,"polyline");line.setAttribute("points",segment.join(" "));line.setAttribute("class","signal-line");svg.append(line);segment=[];}};points.forEach((point,index)=>{if(point.value_milli==null){flush();const gap=document.createElementNS(svg.namespaceURI,"line");gap.setAttribute("x1",x(index));gap.setAttribute("x2",x(index));gap.setAttribute("y1","8");gap.setAttribute("y2","76");gap.setAttribute("class","signal-gap");svg.append(gap);return;}segment.push(`${x(index)},${y(point.value_milli)}`);});flush();for(const [index,point] of points.entries()){if(point.value_milli==null)continue;const dot=document.createElementNS(svg.namespaceURI,"circle");dot.setAttribute("cx",x(index));dot.setAttribute("cy",y(point.value_milli));dot.setAttribute("r","3");dot.dataset.disposition=point.disposition;svg.append(dot);}}
  figure.append(svg,caption);return figure;
}
function renderLearnedProjection(projection){
  const panel=document.createElement("section"),heading=document.createElement("h4"),facts=document.createElement("dl"),detail=projection.detail;panel.className="learned-watch";panel.dataset.projection=projection.projection;heading.textContent=projection.projection;panel.append(heading);term(facts,"Observation sequence",projection.observation_sequence);term(facts,"Presentation bound",`${projection.max_updates_per_second} updates/s · ${projection.dropped_updates} dropped`);
  if(projection.projection==="signal"){term(facts,"Stream",`${detail.role} · ${detail.channel} · ${detail.unit}`);term(facts,"Clock",detail.clock_identity);term(facts,"Alignment",detail.alignment.status==="related"?`related by ${detail.alignment.relation_evidence}`:detail.alignment.status);term(facts,"Continuity",detail.continuity);panel.append(facts,learnedPlot(detail,`${detail.role} ${detail.channel} trajectory`));}
  else if(projection.projection==="tensor"){term(facts,"Tensor",`${detail.dtype} [${detail.shape.join(" × ")}] · ${detail.total_bytes} bytes`);term(facts,"Axes",detail.axes.map(axis=>`${axis.role}:${axis.length}${axis.unit?` ${axis.unit}`:""}`).join(", "));term(facts,"Bounded slice",`${detail.bounded_slice_milli.map(milli).join(", ")}${detail.slice_truncated?" … truncated":""}`);term(facts,"Resource",detail.resource_identity);panel.append(facts);}
  else if(projection.projection==="probabilistic"){term(facts,"Disposition",detail.disposition);term(facts,"Posterior",`mean ${milli(detail.mean_milli)} · σ ${milli(detail.standard_deviation_milli)}`);term(facts,"Samples",`${detail.sample_count} · ${detail.seed_profile} · ${detail.approximation}${detail.truncated?" · truncated":""}`);term(facts,"Alternatives",detail.alternatives.map(item=>`${item.label} ${milli(item.value_milli)} (${(item.weight_millionths/10000).toFixed(1)}%)`).join(", "));panel.append(facts);}
  else if(projection.projection==="state"){term(facts,"Generation",`${detail.generation} · step ${detail.step}`);term(facts,"Transition",detail.transition);term(facts,"Current",detail.value_identity);term(facts,"Candidate",detail.candidate_identity);panel.append(facts);}
  else if(projection.projection==="training"){term(facts,"Training",`${detail.phase} · step ${detail.step} · ${detail.work_units} work units`);term(facts,"Batch",`${detail.split_identity} · ${detail.batch_identity}`);term(facts,"Objectives",`${detail.objectives.map(item=>`${item.name} ${milli(item.value_milli)}`).join(" + ")} = ${milli(detail.total_loss_milli)}`);term(facts,"Checkpoint",detail.checkpoint_event);term(facts,"Pressure",detail.pressure);panel.append(facts);}
  else if(projection.projection==="dynamics"){term(facts,"Interval",`${detail.start_tick}–${detail.end_tick} on ${detail.clock_identity}`);term(facts,"Solver",`${detail.solver_work} work · tolerance ${detail.tolerance_millionths} ppm · error ${detail.estimated_error_millionths} ppm${detail.truncated?" · truncated":""}`);term(facts,"Refusal",detail.refusal);panel.append(facts,learnedPlot(detail,"continuous dynamics trajectory"));}
  return panel;
}
function renderTimeline(subject){
  renderExecutionControl(subject);
  const section=document.querySelector("#debugger-timeline"),gap=section.querySelector(".timeline-gap"),list=section.querySelector(".timeline-events"),timeline=state.snapshot?.timeline,projection=state.snapshot?.timeline_projection;list.replaceChildren();section.hidden=!timeline;if(!timeline||!projection){presentSharedStatus("timeline-status","No retained observations available.");presentActions("timeline-actions",[]);gap.textContent="";return;}
  const controls=[];const control=(label,action,disabled=false)=>controls.push({label,disabled,run:()=>dispatchTimeline(action)});
  control(projection.mode==="live"?"Pause visualization":"Visualization paused","pause",projection.mode!=="live");control("Previous event","previous",projection.cursor==null||projection.cursor===0);control("Next event","next",projection.cursor==null||projection.cursor===timeline.events.length-1);control("Jump live","jump-live",projection.mode==="live");
  const hasSubject=subject&&timeline.events.some(event=>event.subject===subject.identity||event.related_subject===subject.identity);if(hasSubject)controls.push({label:"Focus events for exact subject",run:()=>dispatchTimeline("filter-subject",{subject:subject.identity})});if(timeline.subject_filter)controls.push({label:"Show all events",run:()=>dispatchTimeline("clear-filter")});if(projection.cursor!=null){controls.push({label:"Trace upstream",run:()=>dispatchTimeline("trace-upstream")},{label:"Trace downstream",run:()=>dispatchTimeline("trace-downstream")});}if(timeline.trace)controls.push({label:"Clear causal trace",run:()=>dispatchTimeline("clear-trace")});presentActions("timeline-actions",controls);
  presentSharedStatus("timeline-status",`${projection.mode==="live"?"Following live observations":"Debugger replay paused; execution is not paused"} · ${timeline.events.length} of 128 events · cursor ${projection.cursor_sequence??"none"} · ${timeline.retained_bytes} of 65536 bytes`);
  const traceGap=timeline.trace?.missing_parent_sequences?.length?` Causal history unavailable for parent sequence ${timeline.trace.missing_parent_sequences.join(", ")}; no topology edge was substituted.`:"";const gapText=(timeline.gap?`Replay is incomplete: ${timeline.gap.dropped_records} observations lost before sequence ${timeline.gap.first_retained_sequence}. Exact reconstruction across this gap is unavailable.`:timeline.evicted_events?`${timeline.evicted_events} oldest events evicted; available range begins at sequence ${timeline.events[0]?.sequence}.`:"")+traceGap;gap.textContent=gapText;gap.dataset.evidenceDisposition=timeline.gap?"missing":timeline.evicted_events?"stale":"current";
  const traceSteps=new Map((timeline.trace?.steps??[]).map((step,position)=>[step.event_index,position+1]));timeline.events.forEach((event,index)=>{if(timeline.subject_filter&&event.subject!==timeline.subject_filter&&event.related_subject!==timeline.subject_filter)return;const row=document.createElement("li"),selectEvent=document.createElement("button"),context=document.createElement("div"),traceNumber=traceSteps.get(index);row.setAttribute("aria-current",String(projection.cursor===index));row.dataset.causalTrace=traceNumber?"exact":"unrelated";selectEvent.type="button";selectEvent.textContent=`${traceNumber?`trace ${traceNumber} · `:""}seq ${event.sequence} · ${event.event} · ${event.subject} · ${watchValue(event)}`;selectEvent.onclick=()=>dispatchTimeline("select-event",{index});context.className="timeline-event-context";context.textContent=`Host ${event.host} · Form ${event.form} · Body ${event.execution.body.join("")} · Plan ${event.execution.plan.join("")} · Play ${event.execution.play.join("")}${event.invocation_sequence!=null?` · Invocation ${event.invocation_sequence}`:""}`;row.append(selectEvent,context);list.append(row);});
}
function renderExecutionControl(subject){
  const section=document.querySelector("#debugger-control"),control=state.snapshot?.debugger_control;section.hidden=!control;if(!control){presentSharedStatus("control-status","Execution control unavailable.","missing-evidence");presentActions("control-actions",[]);return;}const selected=subject?.identity,eligible=control.eligible_subjects.includes(selected),actions=[];let status;if(control.state==="suspended"){status=`Execution actually suspended before Gear start at ${control.suspended_subject}. This is runtime suspension, not visualization replay pause.`;actions.push({label:"Resume execution",run:()=>dispatchDebuggerControl("resume")});}else if(control.state==="running"){status="Execution running. Pausing the timeline does not suspend this Play.";if(eligible)actions.push({label:"Break here",run:()=>dispatchDebuggerControl("break-here",selected)});}else status=`Execution control ${control.state}: ${control.reason??"the exact execution context is unavailable"}`;presentSharedStatus("control-status",status,control.state==="running"||control.state==="suspended"?"status":"warning-status");presentActions("control-actions",actions);
}
function renderWatches(subject){
  const section=document.querySelector("#debugger-watches"),list=section.querySelector(".watch-list"),set=state.snapshot?.watches;list.replaceChildren();section.hidden=!set;if(!set){presentSharedStatus("watch-status","No debugger Watches available.","missing-evidence");presentActions("watch-actions",[]);return;}
  const selected=subject?.identity,eligible=set.eligible_subjects.some(([identity])=>identity===selected),current=set.watches.find(watch=>watch.subject===selected);
  const actions=[];
  if(eligible&&selected){
    if(current){actions.push({label:"Remove Watch",run:()=>dispatchWatch("remove",selected)},{label:"Clear Watch history",run:()=>dispatchWatch("clear-history",selected)});}
    else actions.push({label:"Watch",run:()=>dispatchWatch("add",selected)});
  }
  presentActions("watch-actions",actions);presentSharedStatus("watch-status",`${set.watches.length} of 8 Watches · sequence-domain rates only`);
  for(const watch of set.watches){
    const replay=state.snapshot.timeline_projection?.watch_states.find(candidate=>candidate.subject===watch.subject),shown=replay?.historical?{...watch,latest:replay.latest}:watch;
    const item=document.createElement("li"),card=document.createElement("article"),focus=document.createElement("button"),facts=document.createElement("dl"),history=document.createElement("ol");card.className="watch-card";card.setAttribute("aria-current",String(set.focused_subject===watch.subject));focus.type="button";focus.textContent=`Watch ${watch.subject}`;focus.onclick=()=>dispatchWatch("focus",watch.subject).then(()=>select(watch.subject));card.append(focus);term(facts,"State",replay?.historical?"historical replay":watch.lifecycle);term(facts,"Latest",watchValue(shown.latest));term(facts,"Kind",shown.latest?.value?.kind??(shown.latest?.fault_code!=null?"fault":shown.latest?.event));term(facts,"Type",shown.latest?.value?.type_identity);if(shown.latest?.value?.truncated)term(facts,"Preview",`Truncated bounded preview of ${shown.latest.value.total_bytes} bytes`);term(facts,"Updates",watch.update_count);term(facts,"Latest sequence",shown.latest?.sequence);term(facts,"Rate",replay?.historical?"Historical cursor; live rate not shown":watch.rate?`${watch.rate.updates} updates / ${watch.rate.sequence_span} sequence steps`:"Unavailable without an authoritative time basis");card.append(facts);
    if(watch.lifecycle!=="current"){const stale=document.createElement("p");stale.className="watch-stale";stale.textContent=watch.lifecycle==="missing"?"Exact watched subject is no longer present.":"Historical Watch from a replaced execution; not current.";card.append(stale);}
    if(watch.telemetry_gap){const gap=document.createElement("p");gap.className="watch-gap";gap.textContent=`History incomplete: ${watch.telemetry_gap.dropped_records} observations lost before sequence ${watch.telemetry_gap.first_retained_sequence}.`;card.append(gap);}
    for(const projection of watch.learned_projections??[])card.append(renderLearnedProjection(projection));
    history.className="watch-history";history.setAttribute("aria-label",`Recent observations for ${watch.subject}`);for(const entry of watch.history){const row=document.createElement("li"),detail=document.createElement("details"),label=document.createElement("summary"),exact=document.createElement("dl");label.textContent=`seq ${entry.sequence} · ${entry.event} · ${watchValue(entry)}`;term(exact,"Exact subject",watch.subject);term(exact,"Execution",`Body ${watch.execution.body.join("")} · Plan ${watch.execution.plan.join("")} · Play ${watch.execution.play.join("")}`);term(exact,"Sequence",entry.sequence);term(exact,"Event",entry.event);detail.append(label,exact);row.append(detail);history.append(row);}card.append(history);item.append(card);list.append(item);
  }
}

function configurationValue(defaultValue,raw){if(defaultValue.U64!==undefined)return {U64:Number(raw)};if(defaultValue.I64!==undefined)return {I64:Number(raw)};if(defaultValue.Bool!==undefined)return {Bool:raw==="true"};if(defaultValue.Text!==undefined)return {Text:raw};throw new Error("this configuration value is not supported by the common browser editor");}
function renderAuthoringActions(subject){
  const authoring=state.snapshot.authoring,nodes=[{parent:null,component:"stack",key:"authoring",text:"",action:null}],actions=[],callbacks=[];
  const addAction=(event,run)=>{const index=actions.length;actions.push({id:`authoring-${index}`,event});callbacks.push(run);return index;};
  const addButton=(label,run)=>nodes.push({parent:0,component:"button",key:`button-${nodes.length}`,text:label,action:addAction("activate",run)});
  if(!authoring||!subject){present();return;}
  const semantic=semanticIdentity(subject.identity);
  if(subject.role==="Gear"){
    const kind=property(subject.identity,"kind-id")?.value?.Identity,entry=authoring.palette.find(item=>item.kind_id===kind);
    addButton("Duplicate Gear",()=>authoringEdit("duplicate-gear",semantic));addButton("Remove Gear",()=>authoringEdit("remove-gear",semantic));
    for(const [fieldIndex,field] of (entry?.configuration??[]).entries()){
      const value=field.default_value,valueKey=`${authoring.source_revision}:${semantic}:${field.key}`,initial=String(value.U64??value.I64??value.Bool??value.Text??"");if(!state.authoringValues.has(valueKey))state.authoringValues.set(valueKey,initial);
      const form=nodes.length,inputAction=addAction("input",event=>state.authoringValues.set(valueKey,new TextDecoder().decode(event.value)));nodes.push({parent:0,component:"form-field",key:`field-${fieldIndex}`,text:"",action:null},{parent:form,component:"field-label",key:`label-${fieldIndex}`,text:`Configure ${field.key}`,action:null},{parent:form,component:"field-help",key:`help-${fieldIndex}`,text:`Canonical ${field.key} value`,action:null},{parent:form,component:"text-input",key:`input-${fieldIndex}`,text:`Configure ${field.key}`,value:state.authoringValues.get(valueKey),valueCapacity:256,action:inputAction});
      addButton("Apply",()=>authoringEdit("configure-gear",semantic,{key:field.key,value:configurationValue(value,state.authoringValues.get(valueKey))}));
    }
  }else if(subject.role==="Port"){
    const direction=property(subject.identity,"direction")?.value?.Text;
    if(state.rerouteCord)addButton("Reroute armed Cord here",()=>{const cord=state.rerouteCord;state.rerouteCord=null;return authoringEdit("reroute-cord",cord,{secondary:semantic});});
    if(direction==="outgoing")addButton(state.cordSource===semantic?"Cord source selected":"Start Cord here",()=>{state.cordSource=semantic;displaySelection(subject.identity);});
    if(direction==="receiving"&&state.cordSource)addButton("Connect selected output here",()=>{const source=state.cordSource;state.cordSource=null;return authoringEdit("connect-ports",source,{secondary:semantic});});
  }else if(subject.role==="Cord"){
    addButton("Remove Cord",()=>authoringEdit("remove-cord",semantic));addButton("Reroute one endpoint",()=>{state.rerouteCord=semantic;displaySelection(subject.identity);});
  }
  present();
  function present(){const revision=(sharedRevisions.get("authoring-actions")??0)+1;sharedRevisions.set("authoring-actions",revision);applicationPresentation.present("authoring-actions",{revision,actions,nodes},{onEvent(event){applicationPresentation.nextEvent("authoring-actions");callbacks[Number(event.action.slice("authoring-".length))]?.(event);}});}
}
function lensProperty(lens,name){if(lens==="world")return ["seed-id","body-id","part-id","candidate-id","membership-state","membership-proof","current","current-body","this-host","opened","freshness-sequence","source-document-id","checked-form-id","offer-generation","profile-id","capability-count","resource-count","planner-capability-count","capability-id","kind-id","operational-state","availability","freshness","line-id","binding-id","source-host-id","source-boot-id","sink-host-id","sink-boot-id","base","in-plan","playing","activity","evidence-class","candidate-state","lifecycle","auto-run","stage","authority-state","refusal","disposition"].includes(name)||name.startsWith("resource-")||name.startsWith("maximum-");if(lens==="form")return !["plan-id","plan-status","realization-layer","placement-id","host-id","boot-id","implementation-id","artifact-id","execution-profile-id","runtime-name","runtime-version","model-name","model-content-id","quantization","admitted-capacity","active-play-id","play-state","pressure","line-id","line","base","base-instance-id"].includes(name)&&!name.startsWith("resource-")&&!name.startsWith("sign-");if(lens==="plan")return ["plan-status","realization-layer","placement-id","host-id","boot-id","implementation-id","artifact-id","execution-profile-id","runtime-name","runtime-version","model-name","model-content-id","quantization","offer-generation","admitted-capacity","line-id","line","base","base-instance-id"].includes(name)||name.startsWith("resource-")||name.startsWith("maximum-");if(lens==="play")return ["active-play-id","play-state","pressure","activity","disposition","request-id","run-id","stage","authority-state","effect-id"].includes(name);if(lens==="signs")return ["evidence-class","effect-id","request-id"].includes(name)||name.startsWith("sign-");return false;}

async function dispatchInteraction(input,presentationBasis=currentPresentationBasis()){
  const response=await fetch("/api/interaction",{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify({...presentationBasis,...input})});
  if(!response.ok)throw new Error(`interaction delivery HTTP ${response.status}`);
  const next=requireSnapshot(await response.json());render(next);return next;
}
async function dispatchNavigation(operation){
  const navigation=state.snapshot.navigation;if(!navigation)throw new Error("portable navigation is unavailable");
  const response=await fetch("/api/navigation",{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify({presentation_id:state.snapshot.presentation.identity,presentation_revision:state.snapshot.presentation.revision,navigation_id:navigation.navigation.identity,operation})});
  if(!response.ok)throw new Error(`navigation delivery HTTP ${response.status}`);
  const next=requireSnapshot(await response.json());render(next);return next;
}
async function dispatchWatch(action,subject){
  const watches=state.snapshot.watches;if(!watches)throw new Error("debugger Watches are unavailable");const response=await fetch("/api/debugger-watch",{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify({presentation_id:state.snapshot.presentation.identity,presentation_revision:state.snapshot.presentation.revision,watch_revision:watches.revision,action,subject})});if(!response.ok)throw new Error(`debugger Watch HTTP ${response.status}`);const next=requireSnapshot(await response.json());render(next);return next;
}
let timelineDispatch=Promise.resolve();
function dispatchTimeline(action,extra={}){
  const run=timelineDispatch.then(async()=>{const timeline=state.snapshot.timeline;if(!timeline)throw new Error("debugger timeline is unavailable");const response=await fetch("/api/debugger-timeline",{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify({presentation_id:state.snapshot.presentation.identity,presentation_revision:state.snapshot.presentation.revision,timeline_revision:timeline.revision,action,...extra})});if(!response.ok)throw new Error(`debugger timeline HTTP ${response.status}`);const next=requireSnapshot(await response.json());render(next);return next;});timelineDispatch=run.catch(()=>{});return run;
}
async function dispatchDebuggerControl(action,subject){
  const control=state.snapshot.debugger_control;if(!control)throw new Error("runtime debugger control is unavailable");const response=await fetch("/api/debugger-control",{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify({presentation_id:state.snapshot.presentation.identity,presentation_revision:state.snapshot.presentation.revision,control_revision:control.revision,action,...(subject?{subject}: {})})});if(!response.ok)throw new Error(`debugger control HTTP ${response.status}`);const next=requireSnapshot(await response.json());render(next);return next;
}
async function closeSubordinateSurfaces(except){
  const closeTruth=except!=="truth"&&document.body.dataset.truthOpen==="true";
  for(const name of ["palette","parts","truth"]){if(name===except)continue;document.body.dataset[`${name}Open`]="false";document.querySelector(`#toggle-${name}`).setAttribute("aria-expanded","false");}
  if(except!=="inspector"){state.inspectorOpen=false;document.body.dataset.inspectorOpen="false";document.querySelector("#toggle-inspector").setAttribute("aria-expanded","false");}
  if(closeTruth&&state.truthTransition)await state.truthTransition;
  if(closeTruth&&state.snapshot.navigation?.cursor.depth==="Exact")await dispatchNavigation({kind:"back"});
}
async function select(identity){await closeSubordinateSurfaces("inspector");state.inspectorOpen=true;state.inspectorDepth=Boolean(state.snapshot.navigation);return state.snapshot.navigation?dispatchNavigation({kind:"focus",subject:identity,depth:"Detail"}):dispatchInteraction({kind:"select",subject:identity});}
function dispatchSemanticAction(action,presentationBasis=currentPresentationBasis()){return dispatchInteraction({kind:"invoke",action_id:action.identity},presentationBasis);}

async function openSeed(identity){
  const action=semanticActions(identity).find(candidate=>candidate.intent==="conduit.intent/open@1");
  if(!action||!actionAvailability(action).available)throw new Error("Seed OPEN is unavailable");
  await dispatchSemanticAction(action);
  document.body.dataset.paletteOpen="false";document.querySelector("#toggle-palette").setAttribute("aria-expanded","false");
  await select(identity);
}

function renderSeedPalette(){
  const list=document.querySelector("#seed-results"),focusedSeed=document.activeElement?.dataset?.seed,query=state.seedQuery.trim().toLocaleLowerCase(),all=projectedSubjects("Seed");
  const visible=all.filter(subject=>[subject.label,subject.accessibility_name,...texts(subject.identity)].join(" ").toLocaleLowerCase().includes(query));
  list.replaceChildren();
  for(const subject of visible){
    const li=document.createElement("li"),button=document.createElement("button"),name=document.createElement("strong"),summary=document.createElement("span");
    button.type="button";button.dataset.seed=subject.identity;button.dataset.subject=subject.identity;button.setAttribute("aria-label",`Open Seed ${subject.label}`);
    name.textContent=subject.label;summary.textContent=texts(subject.identity)[0]??subject.accessibility_name;button.append(name,summary);button.onclick=()=>openSeed(subject.identity);li.append(button);list.append(li);
  }
  document.querySelector("#seed-results-status").textContent=`${visible.length} of ${all.length} Seeds available`;
  if(focusedSeed)list.querySelector(`[data-seed="${CSS.escape(focusedSeed)}"]`)?.focus();
}

function renderGearPalette(){
  const list=document.querySelector("#gear-results"),focused=document.activeElement?.dataset?.kind,all=state.snapshot.authoring?.palette??[],query=state.gearQuery.trim().toLocaleLowerCase();
  const visible=all.filter(entry=>[entry.name,entry.kind_id,entry.summary,entry.category,...entry.tags,...entry.inputs.map(port=>`${port.identity} ${port.info}`),...entry.outputs.map(port=>`${port.identity} ${port.info}`)].join(" ").toLocaleLowerCase().includes(query));list.replaceChildren();
  for(const entry of visible){const li=document.createElement("li"),button=document.createElement("button"),name=document.createElement("strong"),summary=document.createElement("span"),ports=document.createElement("small");button.type="button";button.dataset.kind=entry.kind_id;button.setAttribute("aria-label",`Place ${entry.name} Gear`);name.textContent=`${entry.name} · ${entry.category}`;summary.textContent=entry.summary;ports.textContent=`${entry.inputs.length} in · ${entry.outputs.length} out · ${entry.kind_id}`;button.append(name,summary,ports);button.onclick=()=>authoringEdit("place-gear",entry.kind_id);li.append(button);list.append(li);}document.querySelector("#gear-results-status").textContent=`${visible.length} of ${all.length} Gears available from the canonical catalog`;if(focused)list.querySelector(`[data-kind="${CSS.escape(focused)}"]`)?.focus();
}

function moveSeedFocus(event){
  if(!["ArrowDown","ArrowUp","Home","End"].includes(event.key))return;
  const buttons=[...document.querySelectorAll("#seed-results button")];if(!buttons.length)return;
  event.preventDefault();const current=buttons.indexOf(document.activeElement),index=event.key==="Home"?0:event.key==="End"?buttons.length-1:event.key==="ArrowDown"?Math.min(buttons.length-1,current+1):Math.max(0,current<0?buttons.length-1:current-1);buttons[index].focus();
}

async function dispatchEntranceTransition(action,subject){
  const response=await fetch("/api/front-door-transition",{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify({presentation_id:state.snapshot.presentation.identity,revision:state.snapshot.revision,action:action==="Birth"?"birth":action.toLowerCase(),subject})});
  if(!response.ok)throw new Error(`front-door transition HTTP ${response.status}`);
  const next=requireSnapshot(await response.json());render(next);return next;
}

async function dispatchFrontDoorAction(action){
  const feedback=document.querySelector("#front-door-feedback"),controls=document.querySelector("#front-door-actions");feedback.textContent=`${action} pending…`;controls.setAttribute("aria-busy","true");
  try{const intent=`conduit.intent/${action.toLowerCase()}@1`,semantic=state.snapshot.presentation.actions.find(candidate=>candidate.intent===intent);if(!semantic)throw new Error(`current Presentation does not expose ${action}`);const next=await dispatchSemanticAction(semantic);feedback.textContent=`${action} ${next.interaction.last_disposition}`;}
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
function partsButton(action,target){const button=document.createElement("button");button.type="button";button.textContent=partsActionLabels[action]??action;button.dataset.partsAction=action;button.dataset.partsTarget=target;button.onclick=()=>dispatchPartsAction(action,target);return button;}
function renderParts(){
  const section=document.querySelector("#parts"),view=state.snapshot.parts;
  section.hidden=!view;if(!view)return;
  const focused=document.activeElement?.dataset?.partsAction?{action:document.activeElement.dataset.partsAction,target:document.activeElement.dataset.partsTarget}:null;
  document.querySelector("#parts-title").textContent=`Parts · ${shortId(view.body_id)}`;document.querySelector("#parts-lifecycle").textContent=view.awake?"AWAKE":"LULLED";
  const notice=document.querySelector("#parts-possibilities");notice.hidden=!view.new_realization_possibilities;notice.textContent=view.new_realization_possibilities?"New realization possibilities are available. The current Plan remains unchanged until Plan again is explicitly requested.":"";
  presentDefinitions("parts-truth",[["AVAILABLE",view.truth_explanation.available],["LINE READY",view.truth_explanation.line_ready],["LINE UNAVAILABLE",view.truth_explanation.line_unavailable],["IN PLAN",view.truth_explanation.in_plan],["PLAYING",view.truth_explanation.playing]]);
  const parts=document.querySelector("#part-rows");parts.replaceChildren();
  for(const row of view.parts){const li=document.createElement("li"),summary=document.createElement("div"),stateText=document.createElement("strong"),badges=document.createElement("div"),actions=document.createElement("div");li.className="parts-row";summary.textContent=row.label;stateText.className="parts-row-state";stateText.textContent=`${row.state.toUpperCase()} · ${row.available?"AVAILABLE":"OFFLINE"}`;badges.className="parts-badges";for(const label of [row.in_plan?"IN PLAN":null,row.playing?"PLAYING":null].filter(Boolean)){const badge=document.createElement("span");badge.className="parts-badge";badge.textContent=label;badges.append(badge);}summary.append(badges);actions.className="parts-row-actions";for(const action of row.actions)actions.append(partsButton(action,row.details.part_id));li.append(summary,stateText,actions);parts.append(li);}
  const candidates=document.querySelector("#candidate-rows");candidates.replaceChildren();
  if(!view.wants_to_join.length){const li=document.createElement("li");li.textContent="No candidates currently want to join.";candidates.append(li);}
  for(const row of view.wants_to_join){const li=document.createElement("li"),summary=document.createElement("div"),stateText=document.createElement("strong"),actions=document.createElement("div");li.className="parts-row";summary.textContent=row.label;stateText.className="parts-row-state";stateText.textContent=`${row.state.replace(/([A-Z])/g," $1").trim().toUpperCase()} · AVAILABLE`;actions.className="parts-row-actions";for(const action of row.actions)actions.append(partsButton(action,row.candidate_id));li.append(summary,stateText,actions);candidates.append(li);}
  const toolbar=document.querySelector("#parts-actions");toolbar.replaceChildren();for(const action of view.actions)toolbar.append(partsButton(action,view.body_id));
  const selectedPart=view.parts.find(row=>row.details.part_id===state.snapshot.interaction.selected_part),selectedCandidate=view.wants_to_join.find(row=>row.candidate_id===state.snapshot.interaction.selected_candidate);let details;
  if(selectedPart){const d=selectedPart.details;details=[["Part",d.part_id],["Host",d.host_id],["Boot",d.boot_id],["Offer generation",d.offer_generation],["Capabilities",d.capabilities.map(item=>`${item.kind_id} (${item.capability_id})`).join(", ")||"none"],["Admission proof",d.proof_reference],["Plan placements",d.planned_placements.join(", ")||"none"],["Authority bindings",d.planned_authority_bindings],["Expected Signs",d.expected_signs]];}
  else if(selectedCandidate)details=[["Candidate",selectedCandidate.candidate_id],["Host",selectedCandidate.host_id],["Boot",selectedCandidate.boot_id],["Offer generation",selectedCandidate.offer_generation],["Capabilities",selectedCandidate.capability_offers.map(item=>`${item.kind_id} (${item.capability_id})`).join(", ")||"none"]];
  else details=[["Selection","Inspect a Part or candidate to disclose exact facts."]];
  presentDefinitions("parts-details",details);
  const feedback=document.querySelector("#parts-feedback");feedback.textContent=state.snapshot.interaction.parts_feedback??"No Parts action requested.";feedback.dataset.disposition=(state.snapshot.interaction.parts_disposition??"").toLowerCase();
  if(focused)document.querySelector(`[data-parts-action="${CSS.escape(focused.action)}"][data-parts-target="${CSS.escape(focused.target)}"]`)?.focus();
}

function shortId(value){return value&&value!=="unsupported"?`${value.slice(0,10)}…`:"not present";}
function renderStructuredNavigator(){
  const list=document.querySelector("#structured-navigator ul"),focused=document.activeElement?.dataset?.subject;list.replaceChildren();
  for(const subject of state.projected.subjects){const li=document.createElement("li"),button=document.createElement("button");button.type="button";button.dataset.subject=subject.identity;button.dataset.role=subject.role;button.setAttribute("aria-pressed",String(subject.identity===state.selected));button.textContent=`${subject.role}: ${subject.accessibility_name}`;button.onclick=()=>select(subject.identity);li.append(button);list.append(li);}
  for(const follow of state.projected.follows.filter(candidate=>candidate.source_subject===state.projected.cursor.focus)){const destination=state.snapshot.presentation.subjects.find(subject=>subject.identity===follow.target_subject);if(!destination)throw new Error("portable FOLLOW destination is absent");const li=document.createElement("li"),button=document.createElement("button");button.type="button";button.dataset.follow=follow.identity;button.textContent=`Follow ${follow.relationship} to ${destination.role}: ${destination.accessibility_name}`;button.onclick=()=>dispatchNavigation({kind:"follow",relationship:follow.identity});li.append(button);list.append(li);}
  if(focused)list.querySelector(`[data-subject="${CSS.escape(focused)}"]`)?.focus();
}
function renderNavigationControls(){
  const bundle=state.snapshot.navigation;if(!bundle)return;
  const cursor=bundle.cursor,places=document.querySelector("#place-controls"),aspects=document.querySelector("#aspect-controls"),current=bundle.navigation.places.find(place=>place.place===cursor.place);places.replaceChildren();aspects.replaceChildren();
  const back=document.createElement("button");back.type="button";back.textContent="Back";back.dataset.navigationBack="true";back.onclick=()=>dispatchNavigation({kind:"back"});places.append(back);
  for(const place of bundle.navigation.places){const button=document.createElement("button");button.type="button";button.textContent=place.label;button.dataset.place=place.place;button.setAttribute("aria-pressed",String(place.place===cursor.place));button.onclick=()=>dispatchNavigation({kind:"enter",place:place.place});places.append(button);}
  for(const aspect of current.aspects){const button=document.createElement("button");button.type="button";button.textContent=aspect.aspect;button.dataset.aspect=aspect.aspect;button.setAttribute("aria-pressed",String(aspect.aspect===cursor.aspect));button.onclick=()=>dispatchNavigation({kind:"show",aspect:aspect.aspect});aspects.append(button);}
}
function selectLens(lens){const aspect=({world:"Structure",form:"Structure",plan:"Plan",play:"Play",signs:"Signs"})[lens];return state.snapshot.navigation?dispatchNavigation({kind:"show",aspect}):Promise.resolve();}

function fillLines(selector,items){const list=document.querySelector(selector);list.replaceChildren();for(const value of items){const li=document.createElement("li");li.textContent=value;list.append(li);}}
function renderTemporalContext(){const section=document.querySelector("#temporal-context"),list=section.querySelector("ol"),facts=state.snapshot.temporal_context;list.replaceChildren();section.hidden=facts.length===0;for(const fact of facts){const subject=state.snapshot.presentation.subjects.find(candidate=>candidate.identity===fact.subject),li=document.createElement("li"),summary=document.createElement("span"),exact=document.createElement("details"),label=document.createElement("summary"),values=document.createElement("dl");summary.textContent=`${fact.relative_time} · ${fact.role.toLowerCase()} · ${subject.accessibility_name}`;label.textContent="Exact time and clock provenance";term(values,"Source",`${fact.source.ticks} ${fact.source.scale}`);term(values,"Source clock",fact.source.clock_basis);term(values,"Reference identity",fact.reference.identity);term(values,"Reference",`${fact.reference.instant.ticks} ${fact.reference.instant.scale}`);term(values,"Reference clock",fact.reference.instant.clock_basis);term(values,"Sign",fact.sign_id);exact.append(label,values);li.append(summary,exact);list.append(li);}}
function renderCards(){const cards=document.querySelector("#route-cards");cards.replaceChildren();for(const route of subjects("Route")){const article=document.createElement("article"),heading=document.createElement("h3");article.className="route-card";heading.textContent=`Route ${route.label}`;article.append(heading);for(const line of texts(route.identity)){const p=document.createElement("p");p.textContent=line;article.append(p);}const children=state.snapshot.presentation.relationships.filter(item=>item.source===route.identity&&item.kind==="Contains").map(item=>item.target);const ul=document.createElement("ul");for(const identity of children){const candidate=state.snapshot.presentation.subjects.find(item=>item.identity===identity);if(!candidate)continue;const li=document.createElement("li");li.textContent=[candidate.label,...properties(identity).map(item=>`${item.name}=${propertyText(item.value)}`)].join(" · ");ul.append(li);}article.append(ul);cards.append(article);}}

function renderBodyWorkbench(workbench){
  const root=document.querySelector("#body-workbench");root.hidden=!workbench;if(!workbench)return;
  const current=workbench.current,history=workbench.history;
  document.querySelector("#body-workbench-title").textContent=current.friendly_name;
  document.querySelector("#body-workbench-status").textContent=current.status_line;
  document.querySelector("#body-workbench-placement").textContent=current.placement_line;
  const action=document.querySelector("#body-workbench-action");action.textContent=current.salient_action;action.dataset.semanticAction=current.salient_action.toLowerCase();
  const facts=document.querySelector("#body-workbench-current dl");facts.replaceChildren();term(facts,"Program",current.program.label);term(facts,"Lifecycle",typeof current.lifecycle==="string"?current.lifecycle:Object.keys(current.lifecycle)[0]);term(facts,"Durable Parts",current.admitted_parts);term(facts,"Current Hosts",current.current_hosts.length);term(facts,"Physical Hosts","Not evidenced");
  const exact=document.querySelector(".body-workbench-exact");exact.replaceChildren();term(exact,"Body",current.body_id);term(exact,"Source document",current.program.source_document_id);term(exact,"Checked Form",current.program.checked_form_id);term(exact,"Evidence revision",current.evidence_revision);
  const entries=document.querySelector("#body-workbench-history>ol"),linear=document.querySelector(".body-workbench-linear");entries.replaceChildren();linear.replaceChildren();
  for(const entry of history.entries){const item=document.createElement("li"),title=document.createElement("strong"),narrative=document.createElement("p"),evidence=document.createElement("details"),summary=document.createElement("summary"),code=document.createElement("code"),line=document.createElement("li");title.textContent=entry.title;narrative.textContent=entry.narrative;summary.textContent="Exact evidence";code.textContent=JSON.stringify(entry.exact);evidence.append(summary,code);item.append(title,narrative,evidence);entries.append(item);line.textContent=entry.linear;linear.append(line);}
}

function render(snapshot){
  const entering=state.snapshot===null;state.snapshot=snapshot;state.projected=projectCurrent(snapshot);const p=snapshot.presentation,b=p.basis,renderer=snapshot.renderer,manifestation=renderer.manifestation,cursor=snapshot.navigation?.cursor;if(entering&&!cursor)state.lens=({World:"world",Intent:"form",Realization:"plan"})[snapshot.entrance.layer]??state.lens;if(cursor)state.lens=lensForCursor(cursor);document.body.dataset.lens=state.lens;document.body.dataset.place=cursor?.place??"Canonical";document.body.dataset.aspect=cursor?.aspect??"Canonical";document.body.dataset.depth=cursor?.depth??"Canonical";
  document.body.dataset.embodied=String(b.body_id!==null);
  const unbodied=b.body_id===null,atEntrance=cursor?.place==="Entrance",authoring=Boolean(snapshot.authoring);document.querySelector("#toggle-palette").textContent=authoring?"Gears":atEntrance?"Seeds":"Navigate";document.querySelector("#palette-title").textContent=authoring?"Gears":atEntrance?"Seeds":"Navigate";document.querySelector("#seed-palette").hidden=!atEntrance||authoring;document.querySelector("#gear-palette").hidden=!authoring;document.querySelector("#structure-title").textContent=cursor?`${cursor.place} ${cursor.aspect}`:(unbodied?"World context":"Program structure");renderNavigationControls();document.querySelector("#lens-label").textContent=cursor?`${cursor.place.toUpperCase()} · ${cursor.aspect.toUpperCase()}`:`${state.lens.toUpperCase()} LENS`;document.querySelector("#canvas-title").textContent=cursor?.place==="Entrance"&&!authoring?"Entrance choices":cursor?.place==="Body"?"Body topology":"Program structure";
  presentStatus(`Presentation revision ${p.revision} · content ${p.identity} · Manifestation ${manifestation.lifecycle} · ${snapshot.authoring?"canonical authoring":"read-only"}`);
  document.querySelector("#run-summary").textContent=`Manifestation ${manifestation.lifecycle} · ${b.plan_id===null?"not planned":"Plan ready"} · ${b.active_play_id===null?"not playing":"Play active"}`;
  document.querySelector("#ordinary-summary").textContent=subjects("Info").flatMap(subject=>texts(subject.identity)).join(" · ");
  document.querySelector("#plan-form").disabled=b.body_id===null||b.plan_id!==null;document.querySelector("#play-plan").disabled=b.body_id===null||b.plan_id===null||b.active_play_id!==null;const lossAction=p.actions.find(action=>action.intent==="conduit.intent/observe-line-loss@1"),lossButton=document.querySelector("#text-lab-loss");lossButton.hidden=!lossAction;lossButton.disabled=lossAction?.availability!=="Available";
  presentDefinitions("form-facts",[["Seed",b.seed_id],["Body",b.body_id],["Wake",b.wake_id],["Source document",b.source_document_id],["Checked Form",b.checked_form_id]]);
  const list=document.querySelector("#subjects"),navigationSubjects=state.projected.subjects;list.replaceChildren();for(const subject of navigationSubjects){const li=document.createElement("li"),button=document.createElement("button");button.type="button";button.dataset.subject=subject.identity;button.dataset.role=subject.role;button.setAttribute("aria-pressed","false");button.textContent=`${subject.role}: ${subject.accessibility_name}`;button.onclick=()=>select(subject.identity);li.append(button);list.append(li);}renderSeedPalette();renderGearPalette();renderParts();renderFlow(snapshot,{onSelect:select,onConnect:(source,sink)=>authoringEdit("connect-ports",semanticIdentity(source),{secondary:semanticIdentity(sink)}),onClear:()=>snapshot.navigation?dispatchNavigation({kind:"focus",subject:snapshot.navigation.navigation.places.find(place=>place.place===cursor.place).root_subject}):dispatchInteraction({kind:"clear"}),lens:state.lens});renderStructuredNavigator();
  const placements=renderer.plan.fragments.flatMap(fragment=>fragment.placements);const placement=placements.find(item=>item.placement_id===manifestation.placement_id);const connections=[...new Map(renderer.plan.fragments.flatMap(fragment=>fragment.connections).map(connection=>[connection.connection_id,connection])).values()];
  presentDefinitions("plan-facts",[["Expanded Form",b.expanded_form_id],["Source Plan",b.plan_id],["Renderer Face",placement?.kind_id],["Renderer Plan",manifestation.plan_id],["Renderer Play",manifestation.active_play_id],["Manifestation",manifestation.manifestation_id],["Lifecycle",manifestation.lifecycle],["Placement",placement?.placement_id],["Host",placement?.host_id],["Boot",placement?.boot_id],["Implementation",placement?.implementation_id],["Artifact",placement?.artifact_id],["Execution profile",placement?.execution_profile_id],["Offer generation",placement?.offer_generation],["Limits",placement?`active=${placement.limits.max_active_instances} queue-items=${placement.limits.max_queue_items} queue-bytes=${placement.limits.max_queue_bytes}`:undefined]]);fillLines("#realizations",[...placements.flatMap(item=>[`${item.gear_id} · host ${item.host_id} · boot ${item.boot_id} · implementation ${item.implementation_id} · artifact ${item.artifact_id}`,...item.inputs.concat(item.outputs).map(port=>`Port ${port.port_id} · ${port.direction} · Info ${port.value_kind} · ${port.temporal}`),...item.resources.map(resource=>`Resource ${resource.pool_id} · class ${resource.class_id} · units ${resource.units}`),...item.host_operations.map(operation=>`Base ${operation.contract_id} · target ${operation.target_kind??"not present"} · in-flight ${operation.maximum_in_flight} · input-bytes ${operation.maximum_input_bytes} · output-bytes ${operation.maximum_output_bytes}`)]),...connections.map(connection=>{const line=connection.selected_line,binding=line?.binding;return `Cord ${connection.connection_id} · ${connection.source_port_id} -> ${connection.sink_port_id} · Info ${connection.value_kind} · Line ${line?.line_id??"not present"} · base ${binding?.base??"not present"} · binding ${binding?.binding_id??"not present"} · base-instance ${binding?.base_instance_id??"not present"}`;})]);
  presentDefinitions("play-facts",[["Active Play",b.active_play_id],["Plan",b.plan_id]]);fillLines("#sign",[...subjects("Sign").map(subject=>subject.label),...manifestation.signs.map(sign=>`Renderer ${sign.sign_id} · ${sign.lifecycle}`)]);
  presentDefinitions("interaction-facts",[["Interaction revision",snapshot.interaction.revision],["Request",snapshot.interaction.last_request_id],["Disposition",snapshot.interaction.last_disposition],["Interaction Plan",snapshot.interaction.interaction_plan_id],["Interaction Play",snapshot.interaction.interaction_play_id]]);
  renderBodyWorkbench(snapshot.body_workbench);renderTemporalContext();const diagnosticLines=subjects("Diagnostic").flatMap(subject=>texts(subject.identity));
  fillLines("#diagnostics ol",diagnosticLines);document.querySelector("#diagnostic-summary").textContent=diagnosticLines.length?`${diagnosticLines.length} checked diagnostic`:"No checked diagnostics";renderCards();fillLines("#topology ul",subjects().filter(subject=>["Seed","Body","Part","Candidate","Host","Capability","Line"].includes(subject.role)).flatMap(subject=>[`${subject.role}: ${subject.accessibility_name}`,...texts(subject.identity)]));fillLines("#linear ol",state.projected.text.map(item=>item.text));displaySelection(cursor?.focus??snapshot.interaction.selected_subject??snapshot.entrance.selected_subject);
}

async function load(){try{const response=await fetch("/api/snapshot",{cache:"no-store"});if(!response.ok)throw new Error(`HTTP ${response.status}`);const snapshot=requireSnapshot(await response.json());if(state.snapshot&&(snapshot.revision<state.snapshot.revision||(snapshot.revision===state.snapshot.revision&&snapshot.interaction.revision<=state.snapshot.interaction.revision)))return;render(snapshot);}catch(error){presentStatus(state.snapshot?`Renderer disconnected; retained revision ${state.snapshot.revision}`:`Snapshot unavailable: ${error.message}`,"failure-status");}}
async function observeTextLabLoss(){const button=document.querySelector("#text-lab-loss"),feedback=document.querySelector("#front-door-feedback");button.disabled=true;feedback.textContent="Running the exact split Text Lab until browser loss…";const {base}=await fetch("/api/text-lab-base",{cache:"no-store"}).then(response=>response.json());if(!admittedRuntimeBytes)throw new Error("admitted browser runtime unavailable");const openLine=()=>new BrowserWebSocketLine({url:base,maximumMessageBytes:1024,maximumBufferedBytes:4096}).open(),forward=await openLine(),runtime=await instantiateTextLabLive(admittedRuntimeBytes,base);let injected=false,failure=null;try{await runTextLabLive(runtime,forward,openLine,async({deliveredValues,returned})=>{if(!injected&&deliveredValues===2){injected=true;void returned.close(4001,"injected-return-line-loss");}});}catch(error){failure=error;}if(!injected||!failure?.message.includes("CND-WS-S4-007"))throw new Error("Text Lab loss did not remain an exact transport failure");feedback.textContent="Browser loss observed; awaiting the native causal receipt.";}
async function joinCurrentBody(){
  const response=await fetch("/api/body-admission",{cache:"no-store"});
  if(response.status===404)return;
  if(!response.ok)throw new Error(`Body admission HTTP ${response.status}`);
  const {url}=await response.json();
  if(!admittedRuntimeBytes)throw new Error("admitted browser runtime unavailable");
  window.__patchbayMembership=await joinBrowserBody({bodyUrl:url,wasmBytes:admittedRuntimeBytes,onState:()=>load()});
}
export async function startApplication(context){admittedRuntimeBytes=context.bytes("runtime");configureFlowStorage(context.storage);
presentStatus("Loading bounded snapshot…");document.body.dataset.lens=state.lens;load().then(()=>joinCurrentBody()).catch(error=>{presentStatus(`Browser Host admission unavailable: ${error.message}`,"failure-status");});window.addEventListener("online",load);window.setInterval(load,250);window.patchbayReload=load;
document.querySelector("#zoom-in").onclick=()=>zoomFlow(1.2);document.querySelector("#zoom-out").onclick=()=>zoomFlow(1/1.2);document.querySelector("#pan-right").onclick=()=>panFlow(40,0);document.querySelector("#arrange").onclick=()=>arrangeFlow();document.querySelector("#theme").onclick=event=>{const active=document.body.classList.toggle("high-contrast");event.currentTarget.setAttribute("aria-pressed",String(active));event.currentTarget.textContent=active?"Standard contrast":"High contrast";};document.querySelector("#toggle-linear").onclick=()=>{const semantic=state.snapshot.presentation.actions.find(candidate=>candidate.intent==="conduit.intent/toggle-linear-view@1");if(semantic)return dispatchSemanticAction(semantic);};
document.querySelector("#fit-flow").onclick=()=>fitFlow();window.patchbayFlowViewport=flowViewport;window.patchbayFlowStorageSettled=flowStorageSettled;
document.querySelector("#center-flow").onclick=()=>focusFlow(state.selected);
document.querySelector("#plan-form").onclick=()=>dispatchFrontDoorAction("Plan");document.querySelector("#play-plan").onclick=()=>dispatchFrontDoorAction("Play");
document.querySelector("#text-lab-loss").onclick=()=>observeTextLabLoss().catch(error=>{document.querySelector("#front-door-feedback").textContent=`Text Lab loss failed: ${error.message}`;});
async function navigateWorkbench(place,aspect){if(state.snapshot.navigation.cursor.place!==place)await dispatchNavigation({kind:"enter",place});if(state.snapshot.navigation.cursor.aspect!==aspect)await dispatchNavigation({kind:"show",aspect});}
for(const button of document.querySelectorAll("[data-workbench-destination]"))button.onclick=async()=>{const destination=button.dataset.workbenchDestination;for(const candidate of document.querySelectorAll("[data-workbench-destination]"))candidate.toggleAttribute("aria-current",candidate===button);document.querySelector("#body-workbench-current").hidden=destination!=="body";document.querySelector("#body-workbench-history").hidden=destination!=="history";if(destination==="program"){await navigateWorkbench("Program","Structure");document.querySelector("#form").scrollIntoView();}else if(destination==="body")await navigateWorkbench("Body","Structure");else await navigateWorkbench("Body","Signs");};
document.querySelector("#body-workbench-action").onclick=()=>dispatchFrontDoorAction(document.querySelector("#body-workbench-action").dataset.semanticAction);
document.querySelector("#clear-selection").onclick=()=>{const navigation=state.snapshot.navigation,current=navigation?.navigation.places.find(place=>place.place===navigation.cursor.place);return navigation?dispatchNavigation({kind:"focus",subject:current.root_subject}):dispatchInteraction({kind:"clear"});};
document.querySelector("#seed-query").oninput=event=>{state.seedQuery=event.currentTarget.value;renderSeedPalette();};document.querySelector("#seed-query").addEventListener("keydown",moveSeedFocus);document.querySelector("#seed-results").addEventListener("keydown",moveSeedFocus);
document.querySelector("#gear-query").oninput=event=>{state.gearQuery=event.currentTarget.value;renderGearPalette();};
const furnitureSurface={palette:"palette",parts:"parts",truth:"deep-inspection",structured:"structured-navigator",inspector:"inspector"};
function focusSurface(name){const surface=document.querySelector(`#${furnitureSurface[name]}`),candidate=[...(surface?.querySelectorAll('button:not([disabled]),a[href],summary,[tabindex="0"]')??[])].find(item=>!item.hidden&&item.getClientRects().length);candidate?.focus();}
async function withFurnitureTransition(name,transition,settle=()=>{}){const surface=document.querySelector(`#${furnitureSurface[name]}`),controls=[...surface.querySelectorAll("button")].filter(control=>control!==document.activeElement);surface.setAttribute("aria-busy","true");for(const control of controls)control.disabled=true;try{return await transition();}finally{surface.removeAttribute("aria-busy");for(const control of controls)control.disabled=false;settle();}}
async function dismissFurnitureSurface(name){
  if(name==="inspector"){if(state.inspectorTransition)await state.inspectorTransition;const returnFromDetail=state.inspectorDepth;state.inspectorOpen=false;state.inspectorDepth=false;displaySelection(state.selected);if(returnFromDetail)await dispatchNavigation({kind:"back"});}
  else if(name==="truth")await withFurnitureTransition(name,async()=>{document.body.dataset.truthOpen="false";if(state.truthTransition)await state.truthTransition;if(state.snapshot.navigation?.cursor.depth==="Exact")await dispatchNavigation({kind:"back"});});
  else document.body.dataset[`${name}Open`]="false";
  const launcher=document.querySelector(`#toggle-${name}`);launcher.setAttribute("aria-expanded","false");launcher.focus();
}
const furniture=installPanelFurniture([
  {name:"palette",slot:"palette-furniture",selector:"#palette",title:"Navigate",dock:"left",onDismiss:()=>dismissFurnitureSurface("palette")},
  {name:"parts",slot:"parts-furniture",selector:"#parts",title:"Parts",dock:"left",onDismiss:()=>dismissFurnitureSurface("parts")},
  {name:"inspector",slot:"inspector-furniture",selector:"#inspector",title:"Inspector",dock:"right",onDismiss:()=>dismissFurnitureSurface("inspector")},
  {name:"truth",slot:"truth-furniture",selector:"#deep-inspection",title:"Exact truth",dock:"right",onDismiss:()=>dismissFurnitureSurface("truth")},
  {name:"structured",slot:"structured-furniture",selector:"#structured-navigator",title:"Subjects",dock:"bottom",onDismiss:()=>dismissFurnitureSurface("structured")},
],applicationPresentation);
async function toggleDrawer(name){const key=`${name}Open`,next=document.body.dataset[key]!=="true";if(next)await closeSubordinateSurfaces(name);document.body.dataset[key]=String(next);const launcher=document.querySelector(`#toggle-${name}`);launcher.setAttribute("aria-expanded",String(next));if(next){furniture.restore(name);focusSurface(name);}else launcher.focus();}
for(const name of ["palette","parts","structured"])document.querySelector(`#toggle-${name}`).onclick=event=>{event.stopPropagation();return toggleDrawer(name);};
document.querySelector("#toggle-truth").onclick=async event=>{event.stopPropagation();const opening=document.body.dataset.truthOpen!=="true";if(opening){await withFurnitureTransition("truth",async()=>{document.body.dataset.truthOpen="true";event.currentTarget.setAttribute("aria-expanded","true");furniture.restore("truth");state.truthTransition=(async()=>{await closeSubordinateSurfaces("truth");if(state.snapshot.navigation)await dispatchNavigation({kind:"disclose",depth:"Exact"});})();await state.truthTransition;state.truthTransition=null;});focusSurface("truth");}else await dismissFurnitureSurface("truth");};
document.querySelector("#toggle-inspector").onclick=async event=>{event.stopPropagation();const opening=document.body.dataset.truthOpen==="true"||document.body.dataset.inspectorOpen!=="true";if(opening){state.inspectorOpen=true;state.inspectorDepth=state.inspectorDepth||Boolean(state.snapshot.navigation?.cursor.focus);displaySelection(state.selected);furniture.restore("inspector");focusSurface("inspector");state.inspectorTransition=withFurnitureTransition("inspector",async()=>{await closeSubordinateSurfaces("inspector");if(state.snapshot.navigation?.cursor.focus&&state.snapshot.navigation.cursor.depth!=="Detail")await dispatchNavigation({kind:"disclose",depth:"Detail"});displaySelection(state.selected);},()=>focusSurface("inspector"));try{await state.inspectorTransition;}finally{state.inspectorTransition=null;}}else await dismissFurnitureSurface("inspector");};
for(const selector of ["#palette","#parts","#inspector","#deep-inspection","#structured-navigator"]){document.querySelector(selector).addEventListener("click",event=>event.stopPropagation());}
document.addEventListener("keydown",event=>{if(event.key!=="Escape")return;const open=["truth","parts","palette","structured"].find(name=>document.body.dataset[`${name}Open`]==="true");if(open){event.preventDefault();if(open==="truth")document.querySelector("#toggle-truth").click();else toggleDrawer(open);return;}if(state.inspectorOpen){event.preventDefault();dismissFurnitureSurface("inspector");}});
document.querySelector("#patchbay-root").removeAttribute("inert");document.body.dataset.applicationReady="true";
}
