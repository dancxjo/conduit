// Workspace orchestration of the existing Crèche lifecycle. Rust owns Body truth.
const encoder = new TextEncoder();
const decoder = new TextDecoder('utf-8', { fatal: true });

export function openWorkspaceSession({ host, storage }) {
  const api = host.runtime;
  const localAdvertisement = host.membership.advertisement();
  let sequence = 0;
  let write = Promise.resolve();
  let persistenceFailure = null;
  let workspace = null;
  const request = (action, fields = {}, binary = false) => {
    const bytes = encoder.encode(JSON.stringify({ action, ...fields }));
    if (bytes.length > api.conduit_workspace_input_capacity()) throw new Error('Workspace input exceeds its bound');
    const pointer = api.conduit_workspace_input_ptr();
    new Uint8Array(api.memory.buffer, pointer, bytes.length).set(bytes);
    const status = api.conduit_workspace_request(bytes.length);
    const length = api.conduit_workspace_output_len();
    if (length < 1 || length > api.conduit_workspace_output_capacity()) throw new Error('Workspace output exceeds its bound');
    const output = new Uint8Array(api.memory.buffer, api.conduit_workspace_output_ptr(), length).slice();
    if (status >= 0 && binary) return output;
    const result = JSON.parse(decoder.decode(output));
    if (status < 0) throw Object.assign(new Error(result.message ?? 'Workspace refused'), { code: result.code, refusal: result });
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
    const snapshot = workspace ? request('Durable') : call('conduit_creche_durable_snapshot');
    if (!snapshot) return write;
    write = write.then(async () => {
      const archives = snapshot.pending_archives ?? [];
      if (archives.length === 0) return storage.writeJson('body-session', snapshot);
      const digestKey = digest => digest.map(byte => byte.toString(16).padStart(2, '0')).join('');
      await storage.writeJsonBatch([
        ...archives.map(segment => ({ key: `body-history/${segment.ordinal}-${digestKey(segment.digest)}`, value: segment, immutable: true })),
        { key: 'body-session', value: snapshot },
      ]);
      request('AcknowledgeArchives', { head_digest: archives.at(-1).digest });
    }).catch(error => {
      persistenceFailure ??= error;
      throw error;
    });
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
      const state = typeof evidence.body.state === 'object' && evidence.body.state?.Fulfilled
        ? 'FULFILLED' : evidence.body.state === 'Lulled' ? 'LULLED' : 'AWAKE';
      return { body_id: evidence.body_id, friendly_name: evidence.friendly_name,
        state,
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
    libraryView(source, query, revision, joinedLines = []) { return request('LibraryView', { ...here, source, query, revision, joined_lines: joinedLines }, true); },
    tutorialView(revision, playback) { return request('TutorialView', { revision, playback }, true); },
    tutorialPresenterRequest(request_identity, presentation_revision, playback) {
      return request('TutorialPresenterInput', { request_identity, presentation_revision, playback }, true);
    },
    invitationView(fields) { return request('InvitationView', fields, true); },
    invitationQr(transfer_uri) { return request('InvitationQr', { transfer_uri }); },
    async changeWorkset(edit, form, source, expected_revision) {
      if (persistenceFailure) throw persistenceFailure;
      await write;
      request('ChangeWorkset', { ...here, edit, form, source, expected_revision });
      await save();
    },
    foreground: () => workspace ? request('Current').foreground : null,
    arrive() { if (!workspace) request('Arrive', { advertisement: localAdvertisement }); return save(); },
    evidence: () => workspace ? request('Current') : null,
    conversationContext: () => request('ConversationContext'),
    async propose(source, joinedLines = [], browserAudioAuthority = false) {
      if (persistenceFailure) throw persistenceFailure;
      const proposal = request('Propose', { ...here, source, joined_lines: joinedLines, browser_audio_authority: browserAudioAuthority }); workspace = request('Current'); await save(); return proposal;
    },
    async started(start) { request('Started', { ...here, play: start.play, wake_at_start: start.wake_at_start }); await save(); },
    async lull(play) { request('Lull', { ...here, terminated_play: play ?? null }); await save(); },
    async failed(rejections) { request('Failed', { ...here, rejections }); await save(); },
    async fulfill(attribution = `operator/${host.hostId}`) {
      if (persistenceFailure) throw persistenceFailure;
      await write;
      request('Fulfill', { ...here, attribution, authority_grant_id: `grant/${host.hostId}/workspace-fulfill` });
      await save();
    },
    save,
    settled: () => write,
    persistenceFailure: () => persistenceFailure,
    inspectInvitation(claim, now = Date.now()) { return request('InspectInvitation', { claim, now_millis: now }); },
    async createInvitation(secret, nonce, now = Date.now(), expires = now + 10 * 60_000) {
      if (persistenceFailure) throw persistenceFailure;
      const claim = request('CreateInvitation', { ...here, secret: Array.from(secret), nonce: Array.from(nonce), now_millis: now, expires_at_millis: expires });
      await save();
      return claim;
    },
    async prepareBrowserSpore(selection, imageContentDigest, secret, nonce, now = Date.now(), expires = now + 10 * 60_000) {
      if (persistenceFailure) throw persistenceFailure;
      const prepared = request('PrepareBrowserSpore', {
        ...here,
        secret: Array.from(secret),
        nonce: Array.from(nonce),
        now_millis: now,
        expires_at_millis: expires,
        image_content_digest: imageContentDigest,
        selection,
      });
      await save();
      return prepared;
    },
    async admitInvitation(advertisement, proof, now = Date.now()) {
      if (persistenceFailure) throw persistenceFailure;
      const receipt = request('AdmitInvitation', { ...here, advertisement, proof, now_millis: now });
      workspace = receipt.body;
      write = write.then(() => storage.writeJson('body-session', receipt.durable)).catch(error => {
        persistenceFailure ??= error;
        throw error;
      });
      await write;
      return receipt;
    },
    async hostLost(hostId, bootId) {
      if (persistenceFailure) throw persistenceFailure;
      request('HostLost', { ...here, lost_host_id: hostId, lost_boot_id: bootId });
      workspace = request('Current');
      await save();
      return workspace;
    },
    async openAdmitted(durable) {
      if (workspace) throw new Error('Close the current body before joining another body');
      request('OpenAdmitted', { evidence: durable.evidence, admission: durable.admission, advertisement: localAdvertisement, ...here });
      if (durable.foreground) request('SelectForm', { form: durable.foreground });
      await save();
      return workspace;
    },
    async restore() {
      const snapshot = await storage.readJson('body-session');
      if (snapshot === null) return null;
      if (snapshot.schema === 'conduit.workspace/body@1') {
        const resumeWake = snapshot.evidence?.body?.state !== 'Lulled'
          && !(typeof snapshot.evidence?.body?.state === 'object' && snapshot.evidence.body.state?.Fulfilled);
        request('Restore', { evidence: snapshot.evidence, admission: snapshot.admission ?? null, advertisement: localAdvertisement, ...here });
        if (snapshot.foreground) request('SelectForm', { form: snapshot.foreground });
        await save();
        return { body: workspace, resume_wake: resumeWake };
      }
      const bytes = encoder.encode(JSON.stringify(snapshot));
      put(bytes);
      const receipt = call('conduit_creche_restore_durable', bytes.length);
      const sequences = [receipt.birth_sequence, ...(snapshot.biography?.records ?? []).map(record => record.sequence)];
      if (sequences.some(value => !Number.isSafeInteger(value) || value < 0)) throw new Error('Retained body sequence is invalid');
      sequence = Math.max(...sequences);
      if (receipt.here_part_id) {
        const parts = [host.hostId, host.bootId].map(value => encoder.encode(value));
        const input = new Uint8Array(parts[0].length + parts[1].length);
        input.set(parts[0]); input.set(parts[1], parts[0].length); put(input);
        const restored = call('conduit_creche_attach_here', parts[0].length, parts[1].length, BigInt(nextSequence()));
        nextSequence();
        if (restored.host_id !== host.hostId || restored.boot_id !== host.bootId) throw new Error('Body membership did not reconcile to this host and Boot');
        await save();
        return { body: restored, resume_wake: false };
      }
      return { body: receipt, resume_wake: false };
    },
  });
}
