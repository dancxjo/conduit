# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: patchbay-html.spec.mjs >> HTML Patchbay reconstructs one typed state accessibly and survives delivery loss
- Location: proof/browser/patchbay-html.spec.mjs:130:1

# Error details

```
Error: expect(received).toBeTruthy()

Received: ""
```

# Page snapshot

```yaml
- generic [ref=e1]:
  - link "Skip to main content" [ref=e2] [cursor=pointer]:
    - /url: "#main-content"
  - generic [ref=e3]:
    - banner [ref=e5]:
      - navigation "Conduit products" [ref=e6]:
        - link "Conduit home" [ref=e7] [cursor=pointer]:
          - /url: /conduit
          - text: conduit
        - link "Patchbay" [ref=e8] [cursor=pointer]:
          - /url: /conduit/patchbay/
        - link "Source" [ref=e9] [cursor=pointer]:
          - /url: https://github.com/dancxjo/conduit
      - status [ref=e10]: Presentation revision 1 · content e738b713bbf37a9c5a24a7e775352bc9c72d0306bcc2b3f9e03ce8a3f4542bd9 · Manifestation Available · read-only
    - navigation "Patchbay workspace" [ref=e11]:
      - generic "Patchbay surfaces" [ref=e12]:
        - button "Library" [ref=e13] [cursor=pointer]
        - button "Inspect" [expanded] [ref=e14] [cursor=pointer]
        - button "No Body" [ref=e15] [cursor=pointer]
    - main [ref=e16]:
      - heading "Conduit Patchbay" [level=1] [ref=e17]
      - generic [ref=e18]:
        - region [ref=e19]:
          - generic [ref=e20]:
            - generic [ref=e21]:
              - paragraph [ref=e22]: PROGRAM · STRUCTURE
              - heading "Program structure" [level=2] [ref=e23]
              - generic [ref=e24]:
                - navigation "Available Places" [ref=e26]:
                  - button "Back" [ref=e27] [cursor=pointer]
                  - button "Body" [ref=e28] [cursor=pointer]
                  - button "Form" [ref=e29] [cursor=pointer]
                - navigation "Available views" [ref=e31]:
                  - button "Structure" [ref=e32] [cursor=pointer]
                  - button "Realization" [ref=e33] [cursor=pointer]
                  - button "Play" [ref=e34] [cursor=pointer]
                  - button "Debug" [ref=e35] [cursor=pointer]
            - generic "Canvas actions" [ref=e36]:
              - button "Fit" [active] [ref=e37] [cursor=pointer]
              - button "Center selected" [ref=e38] [cursor=pointer]
              - button "Zoom in" [ref=e39] [cursor=pointer]
              - button "Zoom out" [ref=e40] [cursor=pointer]
              - button "Pan right" [ref=e41] [cursor=pointer]
              - button "Arrange" [ref=e42] [cursor=pointer]
              - button "High contrast" [ref=e43] [cursor=pointer]
          - generic "Patchbay spatial workspace" [ref=e44]:
            - generic [ref=e45]:
              - generic [ref=e46]:
                - generic:
                  - img:
                    - generic:
                      - generic [ref=e47]: value/text
                      - generic [ref=e51]: value/text
                  - generic:
                    - article "Current checked and expanded Form" [ref=e55]:
                      - generic [ref=e56]:
                        - radio "Select Current checked and expanded Form" [ref=e57] [cursor=pointer]
                        - generic [ref=e58]: ◇
                        - generic "Current checked and expanded Form" [ref=e59]
                        - generic "Form" [ref=e60]
                      - paragraph [ref=e61]: checked intent
                    - article "Form hello checked 5b3fd5438bc728482b8e0e07c6daa1a54701905634c0e941767514004d2ea8c5" [ref=e63]:
                      - generic [ref=e64]:
                        - radio "Select Form hello checked 5b3fd5438bc728482b8e0e07c6daa1a54701905634c0e941767514004d2ea8c5" [ref=e65] [cursor=pointer]
                        - generic [ref=e66]: ◇
                        - generic "Form hello checked 5b3fd5438bc728482b8e0e07c6daa1a54701905634c0e941767514004d2ea8c5" [ref=e67]
                        - generic "Form" [ref=e68]
                      - paragraph [ref=e69]: checked intent
                    - article "hello/literal-6f8beb2a9ae7-0 Gear, text/literal" [ref=e71]:
                      - generic [ref=e72]:
                        - radio "Select hello/literal-6f8beb2a9ae7-0 Gear, text/literal" [checked] [ref=e73] [cursor=pointer]
                        - generic [ref=e74]: type
                        - generic "hello/literal-6f8beb2a9ae7-0 Gear, text/literal" [ref=e75]: literal
                        - generic "Gear" [ref=e76]
                      - paragraph [ref=e77]: text/literal
                      - group "Exact typed Ports" [ref=e78]:
                        - generic [ref=e79]:
                          - generic "text Output Port carrying value/text"
                          - code [ref=e80]: value/text
                          - img "text Output Port carrying value/text" [ref=e81]
                    - article "hello/show Gear, presentation/text" [ref=e83]:
                      - generic [ref=e84]:
                        - radio "Select hello/show Gear, presentation/text" [ref=e85] [cursor=pointer]
                        - generic [ref=e86]: presentation
                        - generic "hello/show Gear, presentation/text" [ref=e87]: text
                        - generic "Gear" [ref=e88]
                      - paragraph [ref=e89]: presentation/text
                      - group "Exact typed Ports" [ref=e90]:
                        - generic [ref=e91]:
                          - img "text Input Port carrying value/text" [ref=e92]
                          - generic "text Input Port carrying value/text"
                          - code [ref=e93]: value/text
                    - article "hello/upper Gear, text/upper" [ref=e95]:
                      - generic [ref=e96]:
                        - radio "Select hello/upper Gear, text/upper" [ref=e97] [cursor=pointer]
                        - generic [ref=e98]: case-upper
                        - generic "hello/upper Gear, text/upper" [ref=e99]: upper
                        - generic "Gear" [ref=e100]
                      - paragraph [ref=e101]: text/upper
                      - group "Exact typed Ports" [ref=e102]:
                        - generic [ref=e103]:
                          - generic "text Output Port carrying value/text"
                          - code [ref=e104]: value/text
                          - img "text Output Port carrying value/text" [ref=e105]
                        - generic [ref=e106]:
                          - img "text Input Port carrying value/text" [ref=e107]
                          - generic "text Input Port carrying value/text"
                          - code [ref=e108]: value/text
              - generic [ref=e112]:
                - button "zoom in" [ref=e113] [cursor=pointer]
                - button "zoom out" [ref=e116] [cursor=pointer]
                - button "fit view" [ref=e119] [cursor=pointer]
              - link "React Flow" [ref=e123] [cursor=pointer]:
                - /url: https://reactflow.dev
            - paragraph [ref=e124]
        - complementary [ref=e125]:
          - group "Inspect furniture" [ref=e127]:
            - paragraph [ref=e128]: Inspect
            - button "Collapse Inspect" [ref=e129] [cursor=pointer]
            - button "Move Inspect to bottom" [ref=e130] [cursor=pointer]
            - button "Close Inspect" [ref=e131] [cursor=pointer]
          - heading "Inspect" [level=2] [ref=e132]
          - button "Clear selection" [ref=e133] [cursor=pointer]
          - paragraph [ref=e134]: hello/literal-6f8beb2a9ae7-0 Gear, text/literal
          - generic "Exact facts" [ref=e136]:
            - generic [ref=e137]:
              - term [ref=e138]: Meaning
              - definition [ref=e139]: hello/literal-6f8beb2a9ae7-0 Gear, text/literal
            - generic [ref=e140]:
              - term [ref=e141]: Subject
              - definition [ref=e142]: Gear
            - generic [ref=e143]:
              - term [ref=e144]: source-form
              - definition [ref=e145]: hello
            - generic [ref=e146]:
              - term [ref=e147]: form-path
              - definition [ref=e148]: hello
            - generic [ref=e149]:
              - term [ref=e150]: icon-token
              - definition [ref=e151]: type
            - generic [ref=e152]:
              - term [ref=e153]: icon-name
              - definition [ref=e154]: text
            - generic [ref=e155]:
              - term [ref=e156]: Authored configuration
              - definition [ref=e157]: value="Hello, world." · max 256 bytes
            - generic [ref=e158]:
              - term [ref=e159]: interaction-contract-0
              - definition [ref=e160]: interaction-contract/84e…01fdf22e
            - generic [ref=e161]:
              - term [ref=e162]: interaction-state-0
              - definition [ref=e163]: interaction-state/e93e3e…526162fe
            - generic [ref=e164]:
              - term [ref=e165]: interaction-family-0
              - definition [ref=e166]: Text
          - generic:
            - group "Selected subject actions"
          - group [ref=e167]:
            - generic "Related subjects" [ref=e168]
          - group [ref=e169]:
            - generic "Exact details" [ref=e170]
          - group [ref=e171]:
            - generic "Advanced evidence and linear presentation" [ref=e172]
      - region "Current run and diagnostic status" [ref=e173]:
        - strong [ref=e174]: Current truth
        - generic [ref=e175]: Manifestation Available · Plan ready · Play active
        - generic [ref=e176]: No checked diagnostics
      - region [ref=e177]:
        - strong [ref=e178]: Current observations
        - article [ref=e183]:
          - heading "between 4 and 6 seconds ago · observation" [level=3] [ref=e184]
          - group [ref=e185]:
            - generic "Exact time and clock provenance" [ref=e186]
      - region "Current form realization" [ref=e187]:
        - strong [ref=e188]: Run status
        - group "Available actions" [ref=e190]:
          - button "Plan current form" [disabled] [ref=e191]
          - button "Play current plan" [disabled] [ref=e192]
        - status [ref=e194]: World truth is current; planning is explicit.
```

# Test source

```ts
  1   | import { spawn } from "node:child_process";
  2   | import { mkdir } from "node:fs/promises";
  3   | import path from "node:path";
  4   | import { createInterface } from "node:readline";
  5   | import { expect, test } from "@playwright/test";
  6   | import { persistCaptureDeclaration } from "./capture-declarations.mjs";
  7   | 
  8   | async function captureCanonical(page,browser,evidenceRoot,name,snapshot,disposition) {
  9   |   await page.evaluate(()=>scrollTo(0,0));
  10  |   const viewport=page.viewportSize();
  11  |   const relativePath=`${name}.png`;
  12  |   await page.screenshot({path:path.join(evidenceRoot,relativePath),fullPage:true,animations:"disabled",caret:"hide",scale:"css"});
  13  |   await persistCaptureDeclaration(evidenceRoot,{
  14  |     id:`patchbay.${name}`,
  15  |     kind:"screenshot",
  16  |     path:relativePath,
  17  |     media_type:"image/png",
  18  |     required:true,
  19  |     provenance:{
  20  |       scenario_id:`patchbay-html.${name}@1`,
  21  |       step_id:"prove.browser-host.patchbay-html-matrix",
  22  |       browser_engine:"chromium",
  23  |       browser_version:browser.version(),
  24  |       viewport:`${viewport.width}x${viewport.height}`,
  25  |       device_scale_factor:"1",
  26  |       locale:"en-US",
  27  |       timezone:"UTC",
  28  |       presentation_id:snapshot.presentation.identity,
  29  |       presentation_revision:String(snapshot.presentation.revision),
  30  |       plan_id:snapshot.presentation.basis.plan_id,
  31  |       active_play_id:snapshot.presentation.basis.active_play_id,
  32  |       manifestation_id:snapshot.renderer.manifestation.manifestation_id,
  33  |       renderer_id:"patchbay-html/dom-svg@1",
  34  |       asserted_semantic_disposition:disposition,
  35  |     },
  36  |   });
  37  | }
  38  | 
  39  | async function closeDrawer(page,name) {
  40  |   if(await page.locator("body").getAttribute(`data-${name}-open`)==="true") {
  41  |     await page.locator(`#toggle-${name}`).click();
  42  |   }
  43  | }
  44  | 
  45  | async function prepareCanvasEvidence(page,{inspector=false,structured=false}={}) {
  46  |   await closeDrawer(page,"palette");
  47  |   const deep=page.locator("#deep-inspection");
  48  |   if(await deep.getAttribute("open")!==null){if(await deep.isVisible())await deep.locator(":scope > summary").click();else await deep.evaluate(element=>{element.open=false;});}
  49  |   const inspectorOpen=await page.locator("body").getAttribute("data-inspector-open")==="true";
  50  |   if(inspectorOpen!==(inspector||structured))await page.locator("#toggle-inspector").click();
  51  |   await page.locator("#structured-navigator").evaluate((element,open)=>{element.closest("details").open=open;},structured);
  52  |   if(!structured)await page.locator("#fit-flow").click();
  53  | }
  54  | 
  55  | async function expectFlowDominant(page,{inspector=false}={}) {
  56  |   await expect(page.locator("#flow-root .react-flow")).toBeVisible();
  57  |   await expect(page.locator("#flow-root .flow-frontplate").first()).toBeVisible();
  58  |   await expect(page.locator('#flow-root .flow-frontplate input[type="radio"]').first()).toBeVisible();
  59  |   const root=await page.locator("#patchbay-root").boundingBox();
  60  |   const flow=await page.locator("#flow-root").boundingBox();
  61  |   expect(flow.width).toBeGreaterThan(root.width*(inspector ? .55 : .65));
  62  |   expect(flow.height).toBeGreaterThan(root.height*.75);
  63  |   expect(await page.locator("body").getAttribute("data-palette-open")).not.toBe("true");
  64  |   if(inspector)await expect(page.locator("#inspector")).toBeVisible();
  65  | }
  66  | 
  67  | function rectanglesOverlap(left,right) {
  68  |   return left.x<right.x+right.width&&left.x+left.width>right.x&&left.y<right.y+right.height&&left.y+left.height>right.y;
  69  | }
  70  | 
  71  | async function expectFaceplateTextContained(page) {
  72  |   const result=await page.locator("#flow-root .flow-frontplate").evaluateAll(faceplates=>faceplates.map(faceplate=>{
  73  |     const bounds=element=>{
  74  |       const box=element.getBoundingClientRect();
  75  |       return {left:box.left,right:box.right,top:box.top,bottom:box.bottom};
  76  |     };
  77  |     const inside=(inner,outer)=>inner.left>=outer.left-.5&&inner.right<=outer.right+.5&&inner.top>=outer.top-.5&&inner.bottom<=outer.bottom+.5;
  78  |     const regions=[...faceplate.querySelectorAll("header,.faceplate-clue,.faceplate-port")];
  79  |     const fields=[...faceplate.querySelectorAll(".faceplate-icon,.faceplate-title,.faceplate-role,.faceplate-clue,.faceplate-port-name,.faceplate-port code")];
  80  |     return {
  81  |       subject:faceplate.dataset.subject,
  82  |       fields:fields.map(field=>{
  83  |         const owner=field.matches(".faceplate-clue")?field:field.closest("header,.faceplate-port");
  84  |         const style=getComputedStyle(field);
  85  |         return {className:field.className,inside:inside(bounds(field),bounds(owner)),overflow:style.overflow,textOverflow:style.textOverflow,whiteSpace:style.whiteSpace,title:field.title,text:field.textContent};
  86  |       }),
  87  |       regions:regions.map(region=>({inside:inside(bounds(region),bounds(faceplate))})),
  88  |     };
  89  |   }));
  90  |   for(const faceplate of result){
  91  |     expect(faceplate.subject).toBeTruthy();
  92  |     for(const field of faceplate.fields){
  93  |       expect(field.inside,`${faceplate.subject} ${field.className} escaped its owned region`).toBe(true);
  94  |       expect({overflow:field.overflow,textOverflow:field.textOverflow,whiteSpace:field.whiteSpace}).toEqual({overflow:"hidden",textOverflow:"ellipsis",whiteSpace:"nowrap"});
  95  |       expect(field.title).toBeTruthy();
> 96  |       expect(field.text).toBeTruthy();
      |                          ^ Error: expect(received).toBeTruthy()
  97  |     }
  98  |     for(const region of faceplate.regions){
  99  |       expect(region.inside,`${faceplate.subject} region escaped its faceplate`).toBe(true);
  100 |     }
  101 |   }
  102 | }
  103 | 
  104 | async function cordGeometry(page,snapshot) {
  105 |   return page.locator("#flow-root .react-flow__edge.flow-cord").evaluateAll((edges,presentation)=>{
  106 |     const value=property=>property?.value?.Identity??property?.value?.Text;
  107 |     const semanticSubjects=new Map(presentation.properties.filter(property=>property.name==="semantic-id").map(property=>[value(property),property.subject]));
  108 |     const cordPorts=presentation.subjects.filter(subject=>subject.role==="Cord").map(cord=>{
  109 |       const properties=presentation.properties.filter(property=>property.subject===cord.identity);
  110 |       return {id:cord.identity,ports:[semanticSubjects.get(value(properties.find(property=>property.name==="source-port"))),semanticSubjects.get(value(properties.find(property=>property.name==="sink-port")))]};
  111 |     }).filter(cord=>cord.ports.every(Boolean)).sort((left,right)=>left.id.localeCompare(right.id));
  112 |     const center=element=>{const box=element.getBoundingClientRect();return {x:box.x+box.width/2,y:box.y+box.height/2,node:element.closest(".react-flow__node")?.dataset.id};};
  113 |     const screenPoint=(path,offset)=>{const point=path.getPointAtLength(offset),matrix=path.getScreenCTM(),screen=new DOMPoint(point.x,point.y).matrixTransform(matrix);return {x:screen.x,y:screen.y};};
  114 |     return edges.map((edge,index)=>{
  115 |       const {id,ports}=cordPorts[index],path=edge.querySelector(".react-flow__edge-path"),source=center(document.querySelector(`.faceplate-handle[data-port-id="${CSS.escape(ports[0])}"]`)),target=center(document.querySelector(`.faceplate-handle[data-port-id="${CSS.escape(ports[1])}"]`));
  116 |       const length=path.getTotalLength(),start=screenPoint(path,0),end=screenPoint(path,length),direct=Math.hypot(target.x-source.x,target.y-source.y);
  117 |       return {id,d:path.getAttribute("d"),marker:path.getAttribute("marker-end"),source,target,start,end,length,direct,forward:target.x>source.x};
  118 |     });
  119 |   },snapshot.presentation);
  120 | }
  121 | 
  122 | function startServer() {
  123 |   const process = spawn("target/debug/patchbay-html", ["--documentary-fixture"], { stdio:["ignore","pipe","pipe"] });
  124 |   const errors=[]; process.stderr.setEncoding("utf8"); process.stderr.on("data",chunk=>errors.push(chunk));
  125 |   const lines=createInterface({input:process.stdout});
  126 |   const url=new Promise((resolve,reject)=>{lines.once("line",line=>resolve(line.replace("PATCHBAY_HTML_URL=","")));process.once("exit",code=>reject(new Error(`Patchbay HTML exited ${code}: ${errors.join("")}`)));});
  127 |   return { process, lines, url };
  128 | }
  129 | 
  130 | test("HTML Patchbay reconstructs one typed state accessibly and survives delivery loss", async ({page,browser},testInfo) => {
  131 |   const evidenceRoot=process.env.CONDUIT_EVIDENCE_ROOT;
  132 |   const canonical=testInfo.project.name==="chromium"&&Boolean(evidenceRoot);
  133 |   if(canonical)await mkdir(evidenceRoot,{recursive:true});
  134 |   const server=startServer();
  135 |   try {
  136 |     const url=await server.url;
  137 |     const snapshot=await (await fetch(`${url}/api/snapshot`)).json();
  138 |     await page.goto(url);
  139 |     await expect(page.getByRole("navigation",{name:"Conduit products"}).getByRole("link")).toHaveCount(3);
  140 |     await expect(page.getByRole("link",{name:"Conduit home"})).toHaveAttribute("href","/conduit");
  141 |     await expect(page.locator('style[data-application-resource="flow-style"]')).toHaveCount(1);
  142 |     await expect(page.locator("#flow-root .flow-frontplate").first()).toHaveCSS("width", "240px");
  143 |     await expect(page.locator('[data-application-key="product-status"]')).toContainText("Presentation revision 1");
  144 |     await expect(page.locator('[data-application-key="product-status"]')).toContainText("Manifestation Available");
  145 |     await expect(page.locator('[data-application-slot="product-masthead"]')).toHaveAttribute("data-application-revision", /^\d+$/);
  146 |     expect(snapshot.temporal_context).toHaveLength(1);
  147 |     expect(snapshot.temporal_context[0].subject).toMatch(/^part\//);
  148 |     expect(snapshot.temporal_context[0].relative_time).toMatch(/seconds ago$/);
  149 |     await expect(page.locator("#temporal-context")).toContainText(snapshot.temporal_context[0].relative_time);
  150 |     await expect(page.locator("#temporal-context")).toContainText("observation");
  151 |     await expect(page.locator("#temporal-context details")).not.toHaveAttribute("open", "");
  152 |     await page.locator("#temporal-context summary").click();
  153 |     await expect(page.locator("#temporal-context details")).toHaveAttribute("open", "");
  154 |     await expect(page.locator("#temporal-context details")).toContainText(snapshot.temporal_context[0].source.clock_basis);
  155 |     await expect(page.locator("#temporal-context details")).toContainText(snapshot.temporal_context[0].reference.identity);
  156 |     await expect(page.locator("#flow-root")).toHaveAttribute("data-renderer","react-flow");
  157 |     await expect(page.locator("#flow-root .react-flow")).toBeVisible();
  158 |     expect(await page.evaluate(()=>({innerHeight,innerWidth,scrollHeight:document.documentElement.scrollHeight,scrollWidth:document.documentElement.scrollWidth}))).toEqual({innerHeight:768,innerWidth:1366,scrollHeight:768,scrollWidth:1366});
  159 |     await page.locator("#body-summary").click();
  160 |     await page.locator("#toggle-palette").click();
  161 |     await expect(page.getByRole("heading",{name:/Parts/})).toBeVisible();
  162 |     const truthExplanation=page.locator("#parts-truth-explanation");
  163 |     await expect(truthExplanation).toContainText("AVAILABLE means this admitted Part has fresh current host/Boot presence");
  164 |     await expect(truthExplanation).toContainText("LINE READY means this exact Line is currently usable");
  165 |     await expect(truthExplanation).toContainText("LINE UNAVAILABLE means this exact Line cannot carry traffic");
  166 |     await expect(truthExplanation).toContainText("IN PLAN means the immutable Plan selected this exact Part/Host/Boot realization");
  167 |     await expect(truthExplanation).toContainText("PLAYING means an active play bound to the current plan includes this Part");
  168 |     await expect(page.locator('#part-rows [data-application-component="artifact"]')).toHaveCount(3);
  169 |     await expect(page.locator("#part-rows")).toContainText("HERE · AVAILABLE");
  170 |     await expect(page.locator("#part-rows")).toContainText("ATTACHED · AVAILABLE");
  171 |     await expect(page.locator("#part-rows")).toContainText("OFFLINE · OFFLINE");
  172 |     await expect(page.locator("#part-rows")).toContainText("IN PLAN");
  173 |     await expect(page.locator("#part-rows")).toContainText("PLAYING");
  174 |     await expect(page.locator('#candidate-rows [data-application-component="artifact"]')).toHaveCount(1);
  175 |     await expect(page.locator("#candidate-rows")).toContainText("Browser · tab 3");
  176 |     await expect(page.locator("#parts-possibilities")).toContainText("current plan remains unchanged");
  177 |     const partPlan=snapshot.presentation.basis.plan_id;
  178 |     const candidateInspect=page.locator("#candidate-rows").getByRole("button",{name:"Inspect"});
  179 |     await candidateInspect.focus();await candidateInspect.press("Enter");
  180 |     await expect(page.locator("#parts-feedback")).toContainText("without admitting it");
  181 |     await expect(page.locator("#parts-details")).toContainText(snapshot.parts.wants_to_join[0].candidate_id);
  182 |     const inspectedParts=await (await fetch(`${url}/api/snapshot`)).json();
  183 |     expect(inspectedParts.parts.wants_to_join).toHaveLength(1);
  184 |     expect(inspectedParts.presentation.basis.plan_id).toBe(partPlan);
  185 |     await page.locator("#candidate-rows").getByRole("button",{name:"Admit"}).click();
  186 |     await expect(page.locator("#parts-feedback")).toContainText("refused nonfatally");
  187 |     const refusedParts=await (await fetch(`${url}/api/snapshot`)).json();
  188 |     expect(refusedParts.parts.wants_to_join).toHaveLength(1);
  189 |     expect(refusedParts.presentation.basis.plan_id).toBe(partPlan);
  190 |     await expect(page.getByRole("button",{name:"+ Browser Part"})).toBeVisible();
  191 |     await expect(page.getByRole("button",{name:"Plan again"})).toBeVisible();
  192 |     await expect(page.getByRole("heading",{name:"Body topology"})).toBeVisible();
  193 |     expect(snapshot.entrance.layer).toBe("World");
  194 |     expect(snapshot.entrance.selected_subject).toMatch(/^part\//);
  195 |     await page.locator("#body-summary").click();
  196 |     await page.locator("#deep-inspection > summary").click();
```