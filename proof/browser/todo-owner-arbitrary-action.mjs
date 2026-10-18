import assert from 'node:assert/strict';
import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';

// Every attempt starts from the same actual owner snapshot and current Show.
// The probe is adversarial DOM, never an alternate owner or action executor.
export async function captureArbitraryTodoRefusals({ page, face, output, sourceRecord, bodyId, identity, retiredView, state }) {
  const beforeAttempt = face();
  const executionPath = path.join(state,'body/owner-execution.json');
  const beforeExecution = await readFile(executionPath);
  const unchangedExecution = async () => assert.deepEqual(await readFile(executionPath),beforeExecution,
    'refused/local events must not create a new owner Plan/Play or committed execution receipt');
  const originalView = await page.evaluate(() => globalThis.__conduitOwnerParticipation.face());
  assert.equal(originalView.interactions_admitted, true);
  const currentAdd = originalView.actions.find(action => action.identity === 'todo.add');
  assert.ok(currentAdd);
  assert.equal(currentAdd.arguments.length, 1);
  const declaration = currentAdd.arguments[0];
  const argument = {name:declaration.name,value:'Must never become a Todo item'};
  const cases = [
    {name:'unknown-action',actionId:'invented.body.action',code:'UnknownAction'},
    {name:'wrong-target',target:'invented/body-port',code:'WrongTarget'},
    {name:'missing-argument',arguments:[],code:'MissingArgument'},
    {name:'unknown-argument',arguments:[{...argument,name:'invented/input'}],code:'UnknownArgument'},
    {name:'duplicate-argument',arguments:[argument,argument],code:'DuplicateArgument'},
    {name:'wrong-kind',arguments:[{...argument,value:42}],code:'WrongValueKind'},
    {name:'oversize',arguments:[{...argument,value:'x'.repeat(declaration.maximum_bytes+1)}],code:'OversizeValue'},
    {name:'malformed-unicode',arguments:[{...argument,value:'\ud800'}],code:'MalformedEncoding'},
    {name:'retired-show',view:retiredView,code:'StaleShow'},
  ];
  assert.ok(retiredView && retiredView.show_id !== originalView.show_id,
    'stale probe must retain the genuine pre-mutation Show');
  const results = [];
  const retain = async (complete = false) => writeFile(path.join(output,'adversarial-receipt.json'),JSON.stringify({
    schema:'conduit.proof/browser-todo-arbitrary-action-refusal@2',...sourceRecord,
    body_id:bodyId,browser_host_id:identity.hostId,browser_boot_id:identity.bootId,
    show_id:originalView.show_id,face_id:originalView.face_id,
    unchanged_owner_execution_receipt:JSON.parse(beforeExecution).last_execution,
    retired_show_id:retiredView.show_id,complete,results,unchanged_owner_face:beforeAttempt.face,
    scope:'Trusted DOM clicks reach production SDK; each typed refusal precedes WASM/owner dispatch and preserves the actual owner snapshot.',
  },null,2)+'\n');
  await retain();
  for (const specimen of cases) {
    assert.deepEqual(face(),beforeAttempt,'each rejection begins at the same owner state');
    await page.evaluate(({specimen,currentAdd,originalView,argument}) => {
      globalThis.__todoAdversarialResult = null;
      const button = document.createElement('button');
      button.textContent = 'Attempt undeclared action';
      button.id = 'todo-adversarial-probe';
      button.onclick = async event => {
        try {
          await globalThis.__conduitOwnerParticipation.submitFaceInteraction({
            view:specimen.view ?? originalView,
            actionId:specimen.actionId ?? currentAdd.identity,target:specimen.target ?? currentAdd.target,
            arguments:specimen.arguments ?? [argument],
          });
          globalThis.__todoAdversarialResult = {trusted:event.isTrusted,refused:false};
        } catch(error) {
          globalThis.__todoAdversarialResult = {trusted:event.isTrusted,refused:true,message:error.message,code:error.code ?? null};
        }
      };
      document.querySelector('[data-owner-face]').append(button);
    }, {specimen,currentAdd,originalView,argument});
    await page.getByRole('button',{name:'Attempt undeclared action',exact:true}).click();
    await page.waitForFunction(() => globalThis.__todoAdversarialResult !== null);
    const result = await page.evaluate(() => globalThis.__todoAdversarialResult);
    assert.equal(result.trusted,true);
    assert.equal(result.refused,true);
    assert.equal(result.code,specimen.code);
    const after = face();
    await unchangedExecution();
    assert.deepEqual(after,beforeAttempt,'refusal preserves canonical owner Face, revision and Host/Boot');
    results.push({case:specimen.name,...result,unchanged_owner_face:after.face,
      accepted_domain_mutations:0,gate:'production-sdk-before-wasm'});
    await retain();
    await page.locator('#todo-adversarial-probe').evaluate(element => element.remove());
  }
  // The installed generic renderer exposes native names/keyboard agency. Local
  // focus and scrolling do not cross the owner admission boundary.
  const form = page.locator('[data-owner-action="todo.add"]');
  const input = form.getByRole('textbox', {name:'Item text'});
  const add = form.getByRole('button', {name:'add an item',exact:true});
  assert.equal(await input.isVisible(),true);
  assert.equal(await add.isEnabled(),true);
  await input.focus();
  await page.keyboard.press('Tab');
  assert.equal(await add.evaluate(node => node.getRootNode().activeElement === node),true);
  await page.evaluate(() => window.scrollBy(0,1));
  assert.deepEqual(face(),beforeAttempt,'focus and scroll are renderer-local');
  await unchangedExecution();
  results.push({case:'focus-scroll',disposition:'MaskLocal',accepted_domain_mutations:0});
  await retain();
  // Hostile appearance must fail preparation before an acknowledged usable Show.
  await page.locator('conduit-owner-face').evaluate(element => {element.hidden=true;});
  await page.getByRole('button',{name:'Refresh this Face',exact:true}).click();
  await page.waitForFunction(() => document.querySelector('[data-owner-face-status]')?.dataset.refused === 'true');
  assert.equal(await page.locator('[data-owner-show-acknowledged]').count(),0);
  assert.match(await page.locator('[data-owner-face-status]').textContent(),/No reachable named browser control/);
  assert.deepEqual(face(),beforeAttempt,'inaccessible rendering cannot mutate owner truth');
  await unchangedExecution();
  results.push({case:'hidden-mandatory-action',disposition:'RendererActionInaccessible',
    show_acknowledged:false,accepted_domain_mutations:0});
  await retain();
  await page.locator('conduit-owner-face').evaluate(element => {element.hidden=false;});
  await page.getByRole('button',{name:'Refresh this Face',exact:true}).click();
  await page.waitForFunction(() => document.querySelector('[data-owner-show-acknowledged]'));
  assert.equal(await add.isEnabled(),true);
  assert.deepEqual(face(),beforeAttempt,'restoring accessible generic renderer does not replay a rejected event');
  await unchangedExecution();
  results.push({case:'restored-accessible-renderer',accepted_domain_mutations:0});
  await retain(true);
}
