// Retain observed Todo mutations for the complete journey producer.
// A record is documentary evidence; it is never a completed journey receipt.
import assert from 'node:assert/strict';
import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';

export async function observedTodoMutation(state, output, label, bodyId, before, after, actionId) {
  const execution = JSON.parse(await readFile(path.join(state, 'body/owner-execution.json'))).last_execution;
  const write = execution?.write;
  assert.equal(execution?.schema, 'conduit.todo/verified-read-receipt@1');
  assert.equal(execution.verified, true);
  assert.equal(execution.body_id, bodyId);
  assert.equal(write?.body_id, bodyId);
  assert.equal(execution.read_terminal, 'Completed');
  assert.equal(write.terminal, 'Completed');
  assert.equal(execution.restored_fore_sha256, write.committed_fore_sha256);
  assert.equal(write.initiating_action.action_id, actionId);
  if (before) {
    assert.equal(write.initiating_action.show_id, before.show_id);
    assert.equal(write.initiating_action.face_id, before.face_id);
  }
  assert.equal(after.presentation.basis.body_id, bodyId);
  assert.equal(write.terminal_sign.active_play_id, write.play.active_play_id);
  assert.equal(execution.read_terminal_sign.active_play_id, execution.read_play.active_play_id);
  assert.notEqual(write.play.active_play_id, execution.read_play.active_play_id);
  const record = {
    schema: 'conduit.proof/todo-observed-mutation@1',
    observed_at_unix_ms: Date.now(), body_id: bodyId, action_id: actionId,
    initiating_face_id: write.initiating_action.face_id,
    initiating_face_revision: before?.face_revision_decimal ?? before?.face_revision ?? null,
    initiating_show_id: write.initiating_action.show_id, interaction_id: write.interaction_id,
    queue_sequence: write.initiating_action.sequence,
    resulting_face_id: after.presentation.identity,
    resulting_face_revision: after.presentation_revision_decimal,
    write_plan_id: write.plan_id, write_play: write.play, write_terminal_sign: write.terminal_sign,
    read_plan_id: execution.read_plan_id, read_play: execution.read_play,
    read_terminal_sign: execution.read_terminal_sign,
    committed_fore_sha256: execution.restored_fore_sha256,
  };
  await writeFile(path.join(output, `${label}-mutation.json`), `${JSON.stringify(record, null, 2)}\n`);
  return record;
}
