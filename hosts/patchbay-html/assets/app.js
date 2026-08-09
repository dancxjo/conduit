const schema = "conduit.patchbay.portable-presentation";
const providers = new Map([
  ["Local", "local"], ["InMemory", "in-memory"],
  ["FixtureFrame", "fixture frame"], ["FixtureDatagram", "fixture datagram"],
  ["WebSocket", "WebSocket"], ["UsbCdc", "USB CDC"],
]);
const state = { snapshot:null, selected:null, zoom:1, panX:0, reversed:false, graphHeight:360 };

function requireSnapshot(value) {
  const presentation=value?.presentation;
  if (!value || value.schema!==schema || !Number.isSafeInteger(value.revision) || value.revision!==presentation?.revision) throw new Error("unsupported snapshot schema");
  if (!presentation.basis || typeof presentation.identity!=="string" || !Array.isArray(presentation.subjects) || !Array.isArray(presentation.relationships) || !Array.isArray(presentation.properties) || !Array.isArray(presentation.text)) throw new Error("malformed portable Presentation");
  for (const property of presentation.properties) {
    const provider=property.value?.ConnectionProvider;
    if (provider!==undefined && !providers.has(provider)) throw new Error("unsupported connection provider");
  }
  return value;
}

function term(dl,name,value){const dt=document.createElement("dt"),dd=document.createElement("dd");dt.textContent=name;dd.textContent=value??"not present";dl.append(dt,dd);}
function subjects(role){return state.snapshot.presentation.subjects.filter(subject=>!role||subject.role===role);}
function texts(identity){return state.snapshot.presentation.text.filter(item=>item.subject===identity).map(item=>item.text);}
function properties(identity){return state.snapshot.presentation.properties.filter(item=>item.subject===identity);}
function propertyText(value){
  if(value.Identity!==undefined)return value.Identity;
  if(value.Text!==undefined)return value.Text;
  if(value.Count!==undefined)return String(value.Count);
  if(value.Flag!==undefined)return String(value.Flag);
  if(value.ConnectionProvider!==undefined)return providers.get(value.ConnectionProvider);
  return "unsupported";
}

function select(identity){
  state.selected=identity;
  document.querySelectorAll("[data-subject]").forEach(item=>{const selected=item.dataset.subject===identity;item.classList.toggle("selected",selected);if(item.tagName==="BUTTON")item.setAttribute("aria-pressed",String(selected));});
  const subject=state.snapshot.presentation.subjects.find(item=>item.identity===identity);
  const dl=document.querySelector("#inspector dl");dl.replaceChildren();term(dl,"Semantic subject",identity);term(dl,"Role",subject?.role);term(dl,"Meaning",subject?.label);
  for(const property of properties(identity))term(dl,property.name,propertyText(property.value));
  term(dl,"Body",state.snapshot.presentation.basis.body_id);term(dl,"Wake",state.snapshot.presentation.basis.wake_id);term(dl,"Plan",state.snapshot.presentation.basis.plan_id);term(dl,"Active Play",state.snapshot.presentation.basis.active_play_id);
}

function applyViewport(){const width=740/state.zoom,height=state.graphHeight/state.zoom;document.querySelector("#graph").setAttribute("viewBox",`${state.panX} 0 ${width} ${height}`);}
function renderGraph(){
  const svg=document.querySelector("#graph");svg.replaceChildren(svg.querySelector("title"));
  const nodes=[...state.snapshot.presentation.subjects];if(state.reversed)nodes.reverse();
  const positions=new Map();nodes.forEach((node,index)=>positions.set(node.identity,{x:40+(index%3)*230,y:40+Math.floor(index/3)*90}));
  for(const relation of state.snapshot.presentation.relationships){const from=positions.get(relation.source),to=positions.get(relation.target);if(!from||!to)continue;const line=document.createElementNS("http://www.w3.org/2000/svg","line");line.setAttribute("x1",from.x+80);line.setAttribute("y1",from.y+25);line.setAttribute("x2",to.x+80);line.setAttribute("y2",to.y+25);line.setAttribute("stroke","#8ee3b8");line.setAttribute("stroke-width","2");line.dataset.relationship=relation.kind;svg.append(line);}
  for(const node of nodes){const position=positions.get(node.identity),group=document.createElementNS("http://www.w3.org/2000/svg","g");group.dataset.subject=node.identity;group.setAttribute("role","button");group.setAttribute("tabindex","0");group.setAttribute("aria-label",node.accessibility_name);const rect=document.createElementNS("http://www.w3.org/2000/svg","rect");rect.setAttribute("x",position.x);rect.setAttribute("y",position.y);rect.setAttribute("width","160");rect.setAttribute("height","50");rect.setAttribute("rx","8");rect.setAttribute("class","node");const label=document.createElementNS("http://www.w3.org/2000/svg","text");label.setAttribute("x",position.x+12);label.setAttribute("y",position.y+30);label.setAttribute("class","node-label");label.textContent=`${node.role}: ${node.label}`.slice(0,22);group.append(rect,label);group.onclick=()=>select(node.identity);group.onkeydown=event=>{if(event.key==="Enter"||event.key===" "){event.preventDefault();select(node.identity);}};svg.append(group);}
  state.graphHeight=Math.max(360,100+Math.ceil(nodes.length/3)*90);applyViewport();
}

function fillLines(selector,items){const list=document.querySelector(selector);list.replaceChildren();for(const value of items){const li=document.createElement("li");li.textContent=value;list.append(li);}}
function renderCards(){const cards=document.querySelector("#route-cards");cards.replaceChildren();for(const route of subjects("Route")){const article=document.createElement("article"),heading=document.createElement("h3");article.className="route-card";heading.textContent=`Route ${route.label}`;article.append(heading);for(const line of texts(route.identity)){const p=document.createElement("p");p.textContent=line;article.append(p);}const children=state.snapshot.presentation.relationships.filter(item=>item.source===route.identity&&item.kind==="Contains").map(item=>item.target);const ul=document.createElement("ul");for(const identity of children){const candidate=state.snapshot.presentation.subjects.find(item=>item.identity===identity);if(!candidate)continue;const li=document.createElement("li");li.textContent=[candidate.label,...properties(identity).map(item=>`${item.name}=${propertyText(item.value)}`)].join(" · ");ul.append(li);}article.append(ul);cards.append(article);}}

function render(snapshot){
  state.snapshot=snapshot;const p=snapshot.presentation,b=p.basis;
  document.querySelector("#status").textContent=`Presentation revision ${p.revision} · content ${p.identity} · read-only`;
  const facts=document.querySelector("#form-facts");facts.replaceChildren();term(facts,"Seed",b.seed_id);term(facts,"Body",b.body_id);term(facts,"Wake",b.wake_id);term(facts,"Source document",b.source_document_id);term(facts,"Checked Form",b.checked_form_id);
  const list=document.querySelector("#subjects");list.replaceChildren();for(const subject of p.subjects){const li=document.createElement("li"),button=document.createElement("button");button.type="button";button.dataset.subject=subject.identity;button.setAttribute("aria-pressed","false");button.textContent=`${subject.role}: ${subject.label}`;button.onclick=()=>select(subject.identity);li.append(button);list.append(li);}renderGraph();
  const plan=document.querySelector("#plan dl");plan.replaceChildren();term(plan,"Expanded Form",b.expanded_form_id);term(plan,"Plan",b.plan_id);fillLines("#realizations",subjects("Plan").flatMap(subject=>texts(subject.identity)));
  const play=document.querySelector("#play dl");play.replaceChildren();term(play,"Active Play",b.active_play_id);term(play,"Plan",b.plan_id);fillLines("#evidence",subjects("Evidence").map(subject=>subject.label));
  fillLines("#diagnostics ol",subjects("Diagnostic").flatMap(subject=>texts(subject.identity)));renderCards();fillLines("#topology ul",subjects("Host").flatMap(subject=>[subject.accessibility_name,...texts(subject.identity)]));fillLines("#linear ol",p.text.map(item=>item.text));select(p.subjects[0]?.identity);
}

async function load(){try{const response=await fetch("/api/snapshot",{cache:"no-store"});if(!response.ok)throw new Error(`HTTP ${response.status}`);render(requireSnapshot(await response.json()));}catch(error){document.querySelector("#status").textContent=state.snapshot?`Renderer disconnected; retained revision ${state.snapshot.revision}`:`Snapshot unavailable: ${error.message}`;}}
load();window.addEventListener("online",load);window.patchbayReload=load;
document.querySelector("#zoom-in").onclick=()=>{state.zoom=Math.min(2,state.zoom+.2);applyViewport();};document.querySelector("#zoom-out").onclick=()=>{state.zoom=Math.max(.5,state.zoom-.2);applyViewport();};document.querySelector("#pan-right").onclick=()=>{state.panX=Math.min(300,state.panX+40);applyViewport();};document.querySelector("#arrange").onclick=event=>{state.reversed=!state.reversed;event.currentTarget.setAttribute("aria-pressed",String(state.reversed));renderGraph();select(state.selected);};document.querySelector("#theme").onclick=event=>{const active=document.body.classList.toggle("light");event.currentTarget.setAttribute("aria-pressed",String(active));event.currentTarget.textContent=active?"Dark theme":"Light theme";};
