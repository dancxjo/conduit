import { spawn } from "node:child_process";
import { createInterface } from "node:readline";
import { expect, test } from "@playwright/test";

function startServer() {
  const process=spawn("target/debug/patchbay-html",["--documentary-fixture"],{stdio:["ignore","pipe","pipe"]});
  const errors=[];process.stderr.setEncoding("utf8");process.stderr.on("data",chunk=>errors.push(chunk));
  const lines=createInterface({input:process.stdout});
  const url=new Promise((resolve,reject)=>{lines.once("line",line=>resolve(line.replace("PATCHBAY_HTML_URL=","")));process.once("exit",code=>reject(new Error(`Patchbay HTML exited ${code}: ${errors.join("")}`)));});
  return {process,lines,url};
}

async function renderCycleCordFixture(page,snapshot) {
  return page.evaluate(async current=>{
    const fixture=structuredClone(current),presentation=fixture.presentation;
    fixture.navigation=null;
    fixture.interaction.revision+=1;
    const value=property=>property?.value?.Identity??property?.value?.Text;
    const ports=presentation.subjects.filter(subject=>subject.role==="Port").map(subject=>{
      const properties=presentation.properties.filter(property=>property.subject===subject.identity);
      return {subject,semantic:value(properties.find(property=>property.name==="semantic-id")),direction:value(properties.find(property=>property.name==="direction"))};
    });
    const owner=port=>presentation.relationships.find(relationship=>relationship.kind==="Contains"&&relationship.target===port.subject.identity)?.source;
    const output=ports.find(port=>port.direction==="outgoing"&&ports.some(candidate=>candidate.direction==="receiving"&&owner(candidate)===owner(port)));
    if(!output)throw new Error("documentary fixture lacks one Gear with input and output Ports");
    const input=ports.find(port=>port.direction==="receiving"&&owner(port)===owner(output));
    const gear=owner(output),form=presentation.relationships.find(relationship=>relationship.kind==="Contains"&&relationship.target===gear)?.source;
    if(!input||!gear||!form)throw new Error("documentary fixture cycle ownership is incomplete");
    const semanticIdentity="cord/zz-cycle-proof",subjectIdentity=`cord/${semanticIdentity}`;
    presentation.subjects.push({identity:subjectIdentity,role:"Cord",label:"Cycle proof Cord",accessibility_name:"Cycle proof Cord returning to the same Gear"});
    presentation.relationships.push({source:form,target:subjectIdentity,kind:"Contains"},{source:subjectIdentity,target:output.subject.identity,kind:"Connects"},{source:subjectIdentity,target:input.subject.identity,kind:"Connects"});
    presentation.properties.push({subject:subjectIdentity,name:"semantic-id",value:{Identity:semanticIdentity}},{subject:subjectIdentity,name:"source-port",value:{Identity:output.semantic}},{subject:subjectIdentity,name:"sink-port",value:{Identity:input.semantic}},{subject:subjectIdentity,name:"value-kind",value:{Text:"value/text@1"}});
    const {renderFlow}=await import("/assets/flow.js");renderFlow(fixture,{onSelect:()=>{},onClear:()=>{},lens:"world"});
    return fixture;
  },snapshot);
}

async function cordGeometry(page,presentation) {
  return page.locator("#flow-root .react-flow__edge.flow-cord").evaluateAll((edges,current)=>{
    const value=property=>property?.value?.Identity??property?.value?.Text;
    const semanticSubjects=new Map(current.properties.filter(property=>property.name==="semantic-id").map(property=>[value(property),property.subject]));
    const cordPorts=current.subjects.filter(subject=>subject.role==="Cord").map(cord=>{
      const properties=current.properties.filter(property=>property.subject===cord.identity);
      return {id:cord.identity,ports:[semanticSubjects.get(value(properties.find(property=>property.name==="source-port"))),semanticSubjects.get(value(properties.find(property=>property.name==="sink-port")))]};
    }).filter(cord=>cord.ports.every(Boolean)).sort((left,right)=>left.id.localeCompare(right.id));
    const center=element=>{const box=element.getBoundingClientRect();return {x:box.x+box.width/2,y:box.y+box.height/2,node:element.closest(".react-flow__node")?.dataset.id};};
    const screenPoint=(path,offset)=>{const point=path.getPointAtLength(offset),matrix=path.getScreenCTM(),screen=new DOMPoint(point.x,point.y).matrixTransform(matrix);return {x:screen.x,y:screen.y};};
    return edges.map((edge,index)=>{
      const {id,ports}=cordPorts[index],path=edge.querySelector(".react-flow__edge-path"),source=center(document.querySelector(`.faceplate-handle[data-port-id="${CSS.escape(ports[0])}"]`)),target=center(document.querySelector(`.faceplate-handle[data-port-id="${CSS.escape(ports[1])}"]`));
      const length=path.getTotalLength(),start=screenPoint(path,0),end=screenPoint(path,length),direct=Math.hypot(target.x-source.x,target.y-source.y);
      return {id,d:path.getAttribute("d"),marker:path.getAttribute("marker-end"),source,target,start,end,length,direct,forward:target.x>source.x};
    });
  },presentation);
}

test("reverse cycle Cord geometry is non-vacuous and stable",async({page})=>{
  const server=startServer();
  try {
    const url=await server.url;await page.goto(url);const snapshot=await(await fetch(`${url}/api/snapshot`)).json();
    const fixture=await renderCycleCordFixture(page,snapshot);
    await page.route("**/api/snapshot",route=>route.fulfill({status:200,contentType:"application/json",body:JSON.stringify(fixture)}));
    await renderCycleCordFixture(page,snapshot);
    await expect(page.locator("#flow-root .react-flow__edge.flow-cord")).toHaveCount(fixture.presentation.properties.filter(item=>item.name==="source-port").length);
    const routes=await cordGeometry(page,fixture.presentation),cycle=routes.find(route=>route.id==="cord/cord/zz-cycle-proof");
    expect(cycle).toBeDefined();expect(cycle.source.node).toBe(cycle.target.node);expect(cycle.forward).toBe(false);
    expect(cycle.direct).toBeGreaterThan(4);expect(cycle.length/cycle.direct).toBeGreaterThan(1);expect(cycle.length/cycle.direct).toBeLessThan(5);
    expect(cycle.marker?.startsWith("url(")).toBe(true);expect(routes.filter(route=>!route.forward).length).toBeGreaterThan(0);
    const refreshed=structuredClone(fixture);refreshed.interaction.revision+=1;
    await page.evaluate(async current=>{const {renderFlow}=await import("/assets/flow.js");renderFlow(current,{onSelect:()=>{},onClear:()=>{},lens:"world"});},refreshed);
    await expect(page.locator("#flow-root .react-flow__edge.flow-cord")).toHaveCount(routes.length);
    const repeated=(await cordGeometry(page,refreshed.presentation)).find(route=>route.id===cycle.id);
    expect(repeated.d).toBe(cycle.d);
  }finally{server.lines.close();if(server.process.exitCode===null)server.process.kill("SIGTERM");}
});
