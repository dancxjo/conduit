// Workspace orchestration of the existing Crèche lifecycle. Rust owns Body truth.
const encoder = new TextEncoder();
const decoder = new TextDecoder('utf-8', { fatal: true });

export function openWorkspaceSession({ host, storage }) {
  const api = host.runtime;
  let sequence = 0;
  let write = Promise.resolve();
  let workspace = null;
  const request = (action, fields = {}) => {
    const bytes = encoder.encode(JSON.stringify({ action, ...fields }));
    if (bytes.length > api.conduit_workspace_input_capacity()) throw new Error('Workspace input exceeds its bound');
    const pointer = api.conduit_workspace_input_ptr();
    new Uint8Array(api.memory.buffer, pointer, bytes.length).set(bytes);
    const status = api.conduit_workspace_request(bytes.length);
    const length = api.conduit_workspace_output_len();
    if (length < 1 || length > 256 * 1024) throw new Error('Workspace output exceeds its bound');
    const result = JSON.parse(decoder.decode(new Uint8Array(api.memory.buffer, api.conduit_workspace_output_ptr(), length)));
    if (status < 0) throw new Error(result.message ?? 'Workspace refused');
    if (result.schema === 'conduit.workspace/body@1') workspace = result;
    return result;
  };
  const here = { host_id: host.hostId, boot_id: host.bootId };
  const read = () => {
    const length = api.conduit_creche_output_len();
    if (!Number.isSafeInteger(length) || length < 1 || length > 65536) throw new Error('Body output exceeds its bound');
    return JSON.parse(decoder.decode(new Uint8Array(api.memory.buffer, api.conduit_creche_output_ptr(), length)));
  };
  const call = (name, ...args) => {
    const status = api[name](...args);
    if (status < 0) throw new Error(read().message ?? `Body operation refused (${status})`);
    return status === 1 ? null : read();
  };
  const put = bytes => {
    if (bytes.length < 1 || bytes.length > api.conduit_creche_input_capacity()) throw new Error('Body input exceeds its bound');
    const pointer = api.conduit_creche_input_ptr();
    new Uint8Array(api.memory.buffer, pointer, bytes.length).set(bytes);
  };
  const nextSequence = () => {
    if (sequence >= Number.MAX_SAFE_INTEGER - 2) throw new Error('Body event sequence exhausted');
    return ++sequence;
  };
  const save = () => {
    const current = workspace ? request('Current') : null;
    const snapshot = current ? { schema: current.schema, evidence: current.evidence, foreground: current.foreground } : call('conduit_creche_durable_snapshot');
    if (!snapshot) return write;
    write = write.then(() => storage.writeJson('body-session', snapshot));
    return write;
  };
  return Object.freeze({
    nextSequence,
    // First-Host admission records both membership and presence.
    nextMembershipSequence() { const next = nextSequence(); nextSequence(); return next; },
    current() {
      if (!workspace) return call('conduit_creche_current');
      const { evidence, realization } = request('Current');
      const part = evidence.membership.parts.find(part => part.current?.host_id === host.hostId && part.current?.boot_id === host.bootId);
      return { body_id: evidence.body_id, friendly_name: evidence.friendly_name,
        state: evidence.body.state === 'Lulled' ? 'LULLED' : 'AWAKE',
        initial_forms: evidence.body.workset.forms, workload_revision: evidence.body.workload_revision,
        here_part_id: part?.part_id, host_id: part?.current?.host_id, boot_id: part?.current?.boot_id,
        wake_id: realization?.wake.wake_id, plan_id: realization?.plan.plan_id,
        active_play_id: realization?.play?.active_play_id };
    },
    attachHere() {
      const parts = [host.hostId, host.bootId].map(value => encoder.encode(value));
      const bytes = new Uint8Array(parts[0].length + parts[1].length);
      bytes.set(parts[0]); bytes.set(parts[1], parts[0].length); put(bytes);
      const at = nextSequence(); nextSequence();
      return call('conduit_creche_attach_here', parts[0].length, parts[1].length, BigInt(at));
    },
    selectForm(form) { request('SelectForm', { form }); return save(); },
    foreground: () => workspace ? request('Current').foreground : null,
    arrive() { if (!workspace) request('Arrive'); return save(); },
    evidence: () => workspace ? request('Current') : null,
    async propose(source) { const proposal = request('Propose', { ...here, source }); workspace = request('Current'); await save(); return proposal; },
    async started(start) { request('Started', { ...here, play: start.play, wake_at_start: start.wake_at_start }); await save(); },
    async lull(play) { request('Lull', { ...here, terminated_play: play ?? null }); await save(); },
    save,
    settled: () => write,
    async restore() {
      const snapshot = await storage.readJson('body-session');
      if (snapshot === null) return null;
      if (snapshot.schema === 'conduit.workspace/body@1') {
        request('Restore', { evidence: snapshot.evidence, ...here });
        if (snapshot.foreground) request('SelectForm', { form: snapshot.foreground });
        await save();
        return workspace;
      }
      const bytes = encoder.encode(JSON.stringify(snapshot));
      put(bytes);
      const receipt = call('conduit_creche_restore_durable', bytes.length);
      const sequences = [receipt.birth_sequence, ...(snapshot.biography?.records ?? []).map(record => record.sequence)];
      if (sequences.some(value => !Number.isSafeInteger(value) || value < 0)) throw new Error('Retained Body sequence is invalid');
      sequence = Math.max(...sequences);
      if (receipt.here_part_id) {
        const parts = [host.hostId, host.bootId].map(value => encoder.encode(value));
        const input = new Uint8Array(parts[0].length + parts[1].length);
        input.set(parts[0]); input.set(parts[1], parts[0].length); put(input);
        const restored = call('conduit_creche_attach_here', parts[0].length, parts[1].length, BigInt(nextSequence()));
        nextSequence();
        if (restored.host_id !== host.hostId || restored.boot_id !== host.bootId) throw new Error('Body membership did not reconcile to this Host and Boot');
        await save();
        return restored;
      }
      return receipt;
    },
  });
}
