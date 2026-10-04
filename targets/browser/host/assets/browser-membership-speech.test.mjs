import assert from "node:assert/strict";
import test from "node:test";
import { checkedSelectedSpeechResponse } from "./browser-membership.js";

const pending = { requestId: "browser-speech/2", kind: "status", operationId: "selected-speech/1" };
const running = { kind: "selected-speech-response", protocol: 1,
  request_id: pending.requestId, outcome: "status", operation_id: pending.operationId,
  code: null, status: { schema: "conduit.body/selected-speech-status@1",
    operation_id: pending.operationId, state: "running" } };

test("selected speech reply requires exact request and operation correlation", () => {
  assert.equal(checkedSelectedSpeechResponse(running, pending, 256).status.state, "running");
  assert.throws(() => checkedSelectedSpeechResponse({ ...running, request_id: "browser-speech/1" }, pending, 256));
  assert.throws(() => checkedSelectedSpeechResponse({ ...running, operation_id: "selected-speech/other" }, pending, 256));
  assert.throws(() => checkedSelectedSpeechResponse({ ...running,
    status: { ...running.status, operation_id: "selected-speech/other" } }, pending, 256));
  assert.throws(() => checkedSelectedSpeechResponse(running, pending, 64 * 1024 + 1));
});

test("selected speech terminal, refusal, and malformed success remain distinct", () => {
  const terminal = { ...running, status: { schema: "conduit.body/selected-speech-terminal@1",
    operation_id: pending.operationId, outcome: "cancelled" } };
  assert.equal(checkedSelectedSpeechResponse(terminal, pending, 256).status.outcome, "cancelled");
  assert.throws(() => checkedSelectedSpeechResponse({ ...running, outcome: "stop-requested" }, pending, 256));
  assert.throws(() => checkedSelectedSpeechResponse({ ...running, outcome: "refused",
    status: null, code: "selected-speech-status-unavailable" }, pending, 256), /refused selected speech/i);
});
