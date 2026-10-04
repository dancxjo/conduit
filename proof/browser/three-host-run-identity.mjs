import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';

export function captureRunId(bodyId, hostId, bootId) {
  for (const value of [bodyId, hostId, bootId]) {
    assert.ok(typeof value === 'string' && value.length > 0,
      'run identity requires current Body, Host, and Boot');
  }
  const identity = JSON.stringify([bodyId, hostId, bootId]);
  return `three-host-${createHash('sha256').update(identity).digest('hex').slice(0, 24)}`;
}
