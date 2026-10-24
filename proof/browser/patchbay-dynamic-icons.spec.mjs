import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import { expect, test } from '@playwright/test';

function startServer() {
  const child=spawn('target/debug/conduit-browser-patchbay-workbench',['--documentary-fixture'],{stdio:['ignore','pipe','pipe']});
  const errors=[];child.stderr.on('data',bytes=>errors.push(String(bytes)));
  const lines=createInterface({input:child.stdout});
  const url=new Promise((resolve,reject)=>{lines.once('line',line=>resolve(line.replace('PATCHBAY_HTML_URL=','')));child.once('exit',code=>reject(new Error(`Workbench exited ${code}: ${errors.join('')}`)))});
  return {child,lines,url};
}

test('dynamic Patchbay symbols preserve exact node and Port selection',async({page},testInfo)=>{
 const server=startServer();
 try{
  const errors=[];page.on('pageerror',error=>errors.push(error.message));
  await page.goto(await server.url);
  await expect(page.locator('body')).toHaveAttribute('data-application-ready','true');
  await page.getByRole('button',{name:'Plot',exact:true}).click();
  await page.getByRole('button',{name:'Fit',exact:true}).click();
  await expect(page.locator('#lens-label')).toHaveText('PROGRAM · STRUCTURE');
  const nodes=page.locator('.flow-frontplate');
  expect(await nodes.count()).toBeGreaterThan(0);
  await expect.poll(()=>nodes.locator('.faceplate-icon svg').count()).toBe(await nodes.count());
  const ports=page.locator('.faceplate-port');
  expect(await ports.count()).toBeGreaterThan(0);
  await expect(ports.locator('.faceplate-inline-icon svg')).toHaveCount(await ports.count());
  for(const icon of await page.locator('.faceplate-icon svg,.faceplate-inline-icon svg').all()){
   await expect(icon).toHaveAttribute('aria-hidden','true');await expect(icon).toHaveAttribute('focusable','false');
   expect(await icon.evaluate(node=>getComputedStyle(node).pointerEvents)).toBe('none');
  }
  const radio=page.locator('.flow-frontplate.role-gear input[type=radio]').first(),identity=await radio.getAttribute('data-subject');
  await expect(radio).not.toBeChecked();
  await radio.focus();await page.keyboard.press('Space');
  await expect.poll(()=>page.evaluate(async()=>(await(await fetch('/api/snapshot')).json()).navigation.cursor.focus)).toBe(identity);
  await expect(radio).toBeChecked();
  const port=page.locator('.faceplate-port-name').first(),portId=await port.locator('..').getAttribute('data-port-id');
  await port.click();
  await expect.poll(()=>page.evaluate(async()=>(await(await fetch('/api/snapshot')).json()).navigation.cursor.focus)).toBe(portId);
  await expect(page.locator('#interaction-proof')).toContainText('Succeeded');
  const portButtons=page.locator('.faceplate-port-select');
  await expect(portButtons).toHaveCount(await ports.count());
  const otherPort=portButtons.nth(1),otherId=await otherPort.locator('..').getAttribute('data-port-id');
  await radio.focus();await page.keyboard.press('Tab');await expect(portButtons.first()).toBeFocused();
  await otherPort.focus();await expect(otherPort).toBeFocused();
  const beforeEnter=await page.evaluate(async()=>(await(await fetch('/api/snapshot')).json()).interaction.revision);
  await page.keyboard.press('Enter');
  await expect.poll(()=>page.evaluate(async()=>(await(await fetch('/api/snapshot')).json()).navigation.cursor.focus)).toBe(otherId);
  await expect(otherPort).toHaveAttribute('aria-pressed','true');
  expect(await page.evaluate(async()=>(await(await fetch('/api/snapshot')).json()).interaction.revision)).toBe(beforeEnter+1);
  const firstPort=portButtons.first();await firstPort.focus();await page.keyboard.press('Space');
  await expect.poll(()=>page.evaluate(async()=>(await(await fetch('/api/snapshot')).json()).navigation.cursor.focus)).toBe(portId);
  await expect(firstPort).toHaveAttribute('aria-pressed','true');
  await expect(otherPort).toHaveAttribute('aria-pressed','false');
  expect(await page.evaluate(async()=>(await(await fetch('/api/snapshot')).json()).interaction.revision)).toBe(beforeEnter+2);
  expect(await firstPort.evaluate(node=>getComputedStyle(node).minHeight)).toBe('44px');

  await page.getByRole('button',{name:'Inspect',exact:true}).click();
  await page.setViewportSize({width:1440,height:1100});
  await page.getByRole('button',{name:'Fit',exact:true}).click();
  const snapshot=await page.evaluate(async()=>(await(await fetch('/api/snapshot')).json()).face);
  await page.screenshot({path:testInfo.outputPath('dynamic-icons-wide.png'),fullPage:true});
  await page.setViewportSize({width:390,height:1000});
  await page.addStyleTag({content:'html{font-size:32px!important}'});
  await page.emulateMedia({forcedColors:'active',reducedMotion:'reduce'});
  await page.getByRole('button',{name:'Fit',exact:true}).click();
  expect(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth)).toBe(false);
  expect(await page.evaluate(async()=>(await(await fetch('/api/snapshot')).json()).face)).toEqual(snapshot);
  await expect(ports.locator('.faceplate-inline-icon svg')).toHaveCount(await ports.count());
  await page.screenshot({path:testInfo.outputPath('dynamic-icons-narrow-forced.png'),fullPage:true});
  expect(errors).toEqual([]);
 }finally{server.lines.close();server.child.kill('SIGTERM')}
});

test('unknown graph decoration tokens retain text without selecting a guessed icon',async({page})=>{
 const server=startServer();
 try{
  await page.goto(await server.url);await expect(page.locator('body')).toHaveAttribute('data-application-ready','true');
  // Isolated renderer fixture, not an owner acknowledgement or executable action.
  await page.evaluate(async()=>{
   const {FaceplateNode}=await import('/assets/flow-frontplate.js');
   const root=document.createElement('div');root.id='unknown-icon-fixture';document.body.append(root);
   window.ReactDOM.createRoot(root).render(window.React.createElement(FaceplateNode,{data:{role:'Gear',label:'Fixture Gear',subjectIdentity:'fixture/gear',accessibilityName:'Fixture Gear',iconToken:'foreign/unavailable',icon:'original token',iconName:'Unknown symbol',ports:[],onActivate:()=>{}}}));
  });
  const fixture=page.locator('#unknown-icon-fixture');await expect(fixture.locator('.faceplate-icon')).toHaveText('original token');
  await expect(fixture.locator('svg')).toHaveCount(0);await expect(fixture.getByRole('radio',{name:'Select Fixture Gear'})).toBeEnabled();
 }finally{server.lines.close();server.child.kill('SIGTERM')}
});
