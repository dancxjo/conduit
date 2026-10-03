// Optional, explicitly authorized participation in a Body owned by a running
// Linux Host. All admission and biography facts come from the production SDK.
const encoder = new TextEncoder();

export function checkedLoopbackOwnerWindow(value) {
  if (typeof value !== 'string' || value.length > 2048) throw new Error('Enter one bounded owner window URL.');
  let url;
  try { url = new URL(value); }
  catch { throw new Error('Enter the WebSocket URL printed by the Body owner.'); }
  if (url.protocol !== 'ws:' || url.hostname !== '127.0.0.1' || !url.port
    || url.username || url.password || url.search || url.hash || url.pathname !== '/conduit') {
    throw new Error('This Handbook join accepts only an exact ws://127.0.0.1:PORT/conduit owner window on this computer.');
  }
  return url.href;
}

export function checkedExpectedBodyId(value) {
  const bodyId = value?.trim();
  if (typeof bodyId !== 'string' || !bodyId || encoder.encode(bodyId).length > 128) {
    throw new Error('Enter the exact Body ID reported by the running owner.');
  }
  return bodyId;
}

export async function startOwnerParticipation(application, root) {
  document.title = 'Join a running Body · Conduit Handbook';
  document.querySelector('.handbook-introduction h1').textContent = 'Join the Body you already made.';
  document.querySelector('.handbook-introduction .lede').textContent =
    'Keep the Linux Host as owner and admit this browser as another Part. Watch the exact membership arrive, then inspect what remains available.';
  document.querySelector('.handbook-start').textContent = 'Join this Body ↓';
  document.querySelector('.handbook-first-steps').hidden = true;
  document.querySelector('.handbook-next').hidden = true;
  document.querySelector('.handbook-local-note').hidden = true;
  root.innerHTML = `<header><p class="eyebrow">Join a running body</p><h2>Give this Body another place to meet you.</h2>
    <p>This browser can join a Body already owned by a Linux Host on this computer. The owner must explicitly authorize this Host first. Joining does not birth another Body.</p></header>
    <p role="status" data-owner-status>Opening this browser Host…</p>
    <section aria-labelledby="owner-identity-title"><h3 id="owner-identity-title">1 · Give the owner this Host identity</h3>
      <p>On the running owner's Linux Host, use <code>conduit body browser-window</code> with this exact Host ID and copied public key. The Boot changes when this page reloads.</p>
      <dl class="owner-identity"><dt>Host</dt><dd><code data-owner-host></code></dd>
        <dt>Boot</dt><dd><code data-owner-boot></code></dd>
        <dt>Public verifying key (byte array)</dt><dd><code data-owner-key></code>
          <button type="button" data-copy-key>Copy key</button></dd></dl></section>
    <form data-owner-join><h3>2 · Join the authorized window</h3>
      <label>Body ID <input name="body" autocomplete="off" required aria-describedby="owner-body-help"></label>
      <p id="owner-body-help">Copy the Body ID from the owner's current truth.</p>
      <label>Owner window URL <input name="window" type="url" autocomplete="off" required placeholder="ws://127.0.0.1:PORT/conduit"></label>
      <p>The owner prints this short-lived URL only after authorizing the Host above.</p>
      <button type="submit">Join this Body</button></form>
    <section aria-labelledby="owner-result-title"><h3 id="owner-result-title">What the Body admitted</h3>
      <p data-owner-result>No admission has been acknowledged.</p>
      <details><summary>Exact owner biography evidence</summary><pre data-owner-evidence>No biography received.</pre></details></section>
    <button type="button" data-owner-leave disabled>Leave this window</button>
    <p><a href="?">Return to my local Handbook</a>. Your local Body remains retained; this join mode does not open or change it.</p>`;
  const status = root.querySelector('[data-owner-status]');
  const form = root.querySelector('[data-owner-join]');
  const result = root.querySelector('[data-owner-result]');
  const evidence = root.querySelector('[data-owner-evidence]');
  const leave = root.querySelector('[data-owner-leave]');
  let host, participation, expectedBodyId = null;
  const showState = state => {
    status.textContent = `Browser participation: ${state}.`;
    status.dataset.state = state;
  };
  const showBiography = biography => {
    const credential = participation?.membershipCredential();
    const part = biography?.membership?.parts?.find(item => item.part_id === credential?.part_id);
    if (!credential || !part || biography.body_id !== expectedBodyId) {
      result.textContent = 'The owner evidence has no matching admitted Part.';
      return;
    }
    const current = part.current;
    result.textContent = current?.host_id === host.id && current?.boot_id === host.bootId
      ? `Body ${biography.body_id} admitted Part ${credential.part_id} on this Host and Boot. The Linux owner remains authoritative; no Plan or Play was transferred.`
      : `Part ${credential.part_id} is retained, but this browser Boot is no longer current.`;
    evidence.textContent = JSON.stringify(biography, null, 2);
    root.dataset.joinedBodyId = biography.body_id;
    root.dataset.joinedPartId = credential.part_id;
  };
  try {
    host = await application.browser({ root });
    const identity = host.admissionIdentity();
    root.querySelector('[data-owner-host]').textContent = identity.hostId;
    root.querySelector('[data-owner-boot]').textContent = identity.bootId;
    root.querySelector('[data-owner-key]').textContent = JSON.stringify(identity.verifyingKey);
    root.querySelector('[data-copy-key]').addEventListener('click', async () => {
      try {
        await navigator.clipboard.writeText(JSON.stringify(identity.verifyingKey));
        status.textContent = 'Public verifying key copied. Give it to the Body owner with the Host ID above.';
      } catch {
        status.textContent = 'Clipboard unavailable. Select and copy the exact public verifying key shown above.';
      }
    });
    showState('ready for owner authorization');
  } catch (error) {
    status.textContent = `Browser Host refused: ${error.message}`;
    status.dataset.refused = 'true';
    form.querySelector('button').disabled = true;
    return;
  }
  form.addEventListener('submit', async event => {
    event.preventDefault();
    if (participation) return;
    try {
      const bodyId = checkedExpectedBodyId(new FormData(form).get('body'));
      const invitation = checkedLoopbackOwnerWindow(new FormData(form).get('window'));
      expectedBodyId = bodyId;
      form.querySelector('button').disabled = true;
      delete status.dataset.refused;
      showState('connecting');
      const retainedCredential = await application.storage.readJson(`owner-part/${bodyId}`);
      participation = await host.participate({ invitation, expectedBodyId: bodyId,
        retainedCredential: retainedCredential?.host_id === host.id ? retainedCredential : null,
        onCredential: credential => application.storage.writeJson(`owner-part/${bodyId}`, credential),
        onState: state => {
          showState(state);
          if (state === 'offline' || state.startsWith('refused:')) leave.disabled = true;
        },
        onBiographyEvidence: biography => queueMicrotask(() => showBiography(biography)),
      });
      leave.disabled = false;
      const received = participation.biographyEvidence();
      if (received) showBiography(received);
    } catch (error) {
      status.textContent = `Join refused: ${error.message}`;
      status.dataset.refused = 'true';
      form.querySelector('button').disabled = false;
    }
  });
  leave.addEventListener('click', () => {
    if (!participation) return;
    participation.close();
    leave.disabled = true;
    showState('leaving');
  });
  globalThis.__conduitOwnerParticipation = Object.freeze({
    host: () => host.current(),
    admissionIdentity: () => host.admissionIdentity(),
    state: () => participation?.state() ?? 'not-joined',
    presence: () => participation?.presenceState() ?? 'unavailable',
    credential: () => participation?.membershipCredential() ?? null,
    biography: () => participation?.biographyEvidence() ?? null,
  });
}
