// Inspect the real generic/specialized renderer without replacing its action path.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import path from 'node:path';

export async function inspectTodoRealization({page,view,face,state}) {
  const collection=view.subjects.find(subject=>subject.identity==='todo/list');
  const action=view.actions.find(action=>action.identity==='todo.add');
  assert.ok(collection && action);
  assert.equal(action.target,collection.identity);
  assert.equal(action.availability,'available');
  assert.equal(action.arguments.length,1);
  const realization={subject_id:collection.identity,subject_name:collection.name,
    action_id:action.identity,target:action.target,action_name:action.name,
    argument_name:action.arguments[0].value_name};
  const specialized=await page.locator('.owner-face-collection').count();
  if(specialized) {
    assert.equal(specialized,1);
    assert.equal(await page.locator('.owner-face-collection-count').count(),1);
    assert.equal(await page.locator('[data-owner-face-document] [data-face-role="Status"]').count(),0);
    return {...realization,kind:'specialized-collection'};
  }
  // Extra source facts deliberately keep the native generic subject renderer.
  // Inspect those exact facts rather than demanding a particular collection Look.
  assert.ok(collection.properties.length>0,'generic fallback specimen must retain actual extra source facts');
  const generic=page.locator('[data-owner-face-document] [data-face-role="Collection"]');
  assert.equal(await generic.count(),1);
  assert.equal(await generic.locator('h4').textContent(),collection.name);
  const facts=generic.locator('details[data-owner-disclosure]');
  assert.equal(await facts.count(),1);
  assert.deepEqual(await facts.evaluate(node=>JSON.parse(node.dataset.ownerDisclosure)),
    [view.body_id,'details',collection.identity,'facts']);
  const before=face();
  const executionPath=path.join(state,'body/owner-execution.json');
  const execution=await readFile(executionPath);
  const summary=facts.getByText('Exact facts',{exact:true});
  await summary.focus();await page.keyboard.press('Enter');
  assert.equal(await facts.getAttribute('open'),'');
  for(const property of collection.properties) assert.equal(await facts.getByText(property,{exact:true}).isVisible(),true);
  const status=view.subjects.find(subject=>subject.identity==='todo/status');
  assert.ok(status && status.text.length===1);
  const shownStatus=page.locator('[data-owner-face-document] [data-face-role="Status"]');
  assert.equal(await shownStatus.count(),1);
  assert.equal(await shownStatus.getByText(status.text[0],{exact:true}).isVisible(),true);
  assert.deepEqual(face(),before,'reading generic source facts is Mask-local');
  assert.deepEqual(await readFile(executionPath),execution,'reading facts cannot create an owner execution');
  const form=page.locator('[data-owner-action="todo.add"]');
  assert.equal(await form.getByRole('textbox',{name:realization.argument_name,exact:true}).isVisible(),true);
  assert.equal(await form.getByRole('button',{name:action.name,exact:true}).isEnabled(),true);
  return {...realization,kind:'generic-source-facts',source_properties:collection.properties,
    source_status:status.text[0],source_equivalent_named_action:true,local_reading_owner_unchanged:true};
}

export async function activateTodoAdd({page,form,text,realization,scenario}) {
  const input=form.getByRole('textbox',{name:realization.argument_name,exact:true});
  const button=form.getByRole('button',{name:realization.action_name,exact:true});
  await input.fill(text);
  if(scenario==='first-add' && realization.kind==='generic-source-facts') {
    await input.focus();await page.keyboard.press('Tab');
    assert.equal(await button.evaluate(node=>node.getRootNode().activeElement===node),true);
    await page.keyboard.press('Enter');
    return 'native-keyboard-tab-enter';
  }
  await button.click();return 'native-pointer-click';
}
