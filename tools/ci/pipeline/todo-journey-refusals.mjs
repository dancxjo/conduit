// Correlate supplementary refusal observations from the same completed producer.
// This validates documentary relationships, not independent live execution.
const insist = (value, message) => { if (!value) throw new Error(`Todo journey refusal: ${message}`); };
const parse = output => JSON.parse(output.bytesValue.toString('utf8'));
const todo = face => face.presentation.properties.filter(item => item.subject.startsWith('todo/'));

export function validateTodoRefusals(outputs, journey) {
  const kinds = {
    'browser-run': 'machine-readable-manifest',
    'stale-action-visible': 'screenshot',
    'checkpoint-refusal': 'machine-readable-manifest',
    'checkpoint-refusal-visible': 'document',
    'repaired-read': 'machine-readable-manifest',
  };
  if (!Object.keys(kinds).some(id => outputs.has(id))) return null;
  for (const [id, kind] of Object.entries(kinds)) {
    insist(outputs.get(id)?.kind === kind && outputs.get(id).scenario_id === journey.run_id,
      `missing same-run ${id}`);
  }
  const browser = parse(outputs.get('browser-run'));
  insist(browser.schema === 'conduit.proof/todo-owner-browser@1'
    && browser.body_id === journey.body_id && browser.owner_source_commit === journey.source_commit
    && browser.source_relation === 'exact-source', 'browser source or Body drift');
  const cross = browser.cross_mask;
  const stale = cross?.stale;
  insist(stale && typeof stale.refusal === 'string' && stale.refusal.includes('refused:')
    && stale.action_id === cross.action_id && typeof stale.initiating_show_id === 'string'
    && cross.owner_after.presentation.basis.body_id === journey.body_id
    && stale.owner_after.presentation.basis.body_id === journey.body_id
    && JSON.stringify(todo(cross.owner_after)) === JSON.stringify(todo(stale.owner_after)),
  'stale action did not retain an unchanged same-Body list');
  const failed = parse(outputs.get('checkpoint-refusal')).last_execution;
  const repaired = parse(outputs.get('repaired-read')).last_execution;
  const recover = parse(outputs.get('recover-source'));
  insist(failed?.schema === 'conduit.todo/verified-read-receipt@1'
    && failed.body_id === journey.body_id && failed.verified === false
    && failed.refusal === 'todo-committed-corrupt' && failed.restored_fore_sha256 === null
    && failed.read_kernel_failure?.detail === 2, 'checkpoint did not refuse without verified state');
  insist(repaired?.schema === 'conduit.todo/verified-read-receipt@1'
    && repaired.body_id === journey.body_id && repaired.verified === true
    && repaired.read_terminal === 'Completed'
    && repaired.restored_fore_sha256 === `sha256:${recover.recovered_state_sha256}`,
  'repair did not verify the same committed state');
  const visible = outputs.get('checkpoint-refusal-visible');
  insist(visible.media_type === 'text/plain; charset=utf-8'
    && visible.bytesValue.toString('utf8').includes(failed.refusal), 'checkpoint refusal is not visible');
  insist(outputs.get('stale-action-visible').media_type === 'image/png', 'stale refusal has no screenshot');
  return { staleText: stale.refusal, checkpointText: visible.bytesValue.toString('utf8'),
    screenshot: outputs.get('stale-action-visible').path,
    browserReceipt: outputs.get('browser-run').path,
    checkpointReceipt: outputs.get('checkpoint-refusal').path,
    checkpointTranscript: visible.path, repairReceipt: outputs.get('repaired-read').path };
}
