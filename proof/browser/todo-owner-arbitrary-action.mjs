import assert from 'node:assert/strict';
import { writeFile } from 'node:fs/promises';
import path from 'node:path';

export async function captureArbitraryTodoRefusals({ page, face, output, sourceRecord, bodyId, identity }) {
  const beforeAttempt = face();
  const originalView = await page.evaluate(() => globalThis.__conduitOwnerParticipation.face());
  assert.equal(originalView.interactions_admitted, true, 'negative proof requires a currently interactive acknowledged Mask');
  const currentAdd = originalView.actions.find(action => action.identity === 'todo.add');
  assert.ok(currentAdd);
  const results = [];
  for (const [actionId, target] of [['invented.body.action', currentAdd.target], ['todo.add', 'invented/body-port']]) {
    await page.evaluate(({actionId,target,argument}) => {
      globalThis.__todoAdversarialResult = null;
      const button = document.createElement('button');
      button.textContent = 'Attempt undeclared action';
      button.id = 'todo-adversarial-probe';
      button.onclick = async event => {
        try {
          await globalThis.__conduitOwnerParticipation.submitFaceInteraction({
            view: globalThis.__conduitOwnerParticipation.face(), actionId, target,
            arguments: [{name:argument,value:'Must never become a Todo item'}],
          });
          globalThis.__todoAdversarialResult = {trusted:event.isTrusted,refused:false};
        } catch(error) {
          globalThis.__todoAdversarialResult = {trusted:event.isTrusted,refused:true,message:error.message};
        }
      };
      document.querySelector('[data-owner-face]').append(button);
    }, {actionId,target,argument:currentAdd.arguments[0].name});
    await page.getByRole('button',{name:'Attempt undeclared action',exact:true}).click();
    await page.waitForFunction(() => globalThis.__todoAdversarialResult !== null);
    const result = await page.evaluate(() => globalThis.__todoAdversarialResult);
    assert.equal(result.trusted,true);
    assert.equal(result.refused,true);
    assert.ok(result.message === 'Choose one currently available Face action and its declared inputs.' || /^owner Face interaction refused \(-[0-9]+\)$/.test(result.message), 'refusal must come from the production SDK/Mask action gate');
    assert.deepEqual(face().face,beforeAttempt.face,'undeclared action must not change the original Body Face');
    results.push({actionId,target,...result});
    await page.locator('#todo-adversarial-probe').evaluate(element => element.remove());
  }
  await writeFile(path.join(output,'adversarial-receipt.json'),JSON.stringify({
    schema:'conduit.proof/browser-todo-arbitrary-action-refusal@1',...sourceRecord,
    body_id:bodyId,browser_host_id:identity.hostId,browser_boot_id:identity.bootId,
    show_id:originalView.show_id,face_id:originalView.face_id,
    results,unchanged_owner_face:beforeAttempt.face,
    scope:'Actual trusted DOM clicks use the production SDK/Mask action gate; invented action/target refuses without Body mutation',
  },null,2)+'\n');
}
