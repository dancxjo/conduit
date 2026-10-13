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
    <section class="owner-face" aria-labelledby="owner-face-title" data-owner-face>
      <header><p class="eyebrow">Same Body · another view</p><h3 id="owner-face-title">What your Body says now</h3>
        <p>The owner supplies the meaning. This browser shows its current Face and sends your available actions back for the owner to check.</p></header>
      <p role="status" data-owner-face-status>Join the Body to see its current Face.</p>
      <p role="status" data-owner-action-result>No Face action submitted.</p>
      <div data-owner-face-document></div>
      <button type="button" data-owner-face-refresh disabled>Refresh this Face</button>
      <details><summary>Face and Show identities</summary><pre data-owner-face-evidence>No Show yet.</pre></details>
    </section>
    <button type="button" data-owner-leave disabled>Leave this window</button>
    <p><a href="?">Return to my local Handbook</a>. Your local Body remains retained; this join mode does not open or change it.</p>`;
  const status = root.querySelector('[data-owner-status]');
  const form = root.querySelector('[data-owner-join]');
  const result = root.querySelector('[data-owner-result]');
  const evidence = root.querySelector('[data-owner-evidence]');
  const leave = root.querySelector('[data-owner-leave]');
  const faceStatus = root.querySelector('[data-owner-face-status]');
  const actionResult = root.querySelector('[data-owner-action-result]');
  const faceDocument = root.querySelector('[data-owner-face-document]');
  const faceEvidence = root.querySelector('[data-owner-face-evidence]');
  const faceRefresh = root.querySelector('[data-owner-face-refresh]');
  let host, participation, expectedBodyId = null, faceView = null, faceBusy = false, actionSequence = 0;
  const faceNode = (tag, text, className) => {
    const node = document.createElement(tag);
    node.textContent = text;
    if (className) node.className = className;
    return node;
  };
  const renderFace = view => {
    const names = new Map(view.subjects.map(subject => [subject.identity, subject.name]));
    const documentNode = faceNode('article', '', 'owner-face-document');
    const overview = faceNode('p', `${view.subjects.length} subjects · ${view.relationships.length} relationships · ${view.actions.length} described actions`, 'owner-face-summary');
    documentNode.append(overview);
    for (const subject of view.subjects) {
      const node = document.createElement(subject.role === 'Body' || subject.role === 'Plot' ? 'article' : 'section');
      node.className = 'owner-face-subject';
      node.dataset.faceRole = subject.role;
      node.append(faceNode('span', subject.role, 'owner-face-role'));
      node.append(faceNode(subject.role === 'Body' ? 'h4' : 'h5', subject.name));
      for (const text of subject.text) node.append(faceNode('p', text));
      if (subject.properties.length) {
        const details = document.createElement('details');
        details.append(faceNode('summary', 'Exact facts'));
        const list = document.createElement('ul');
        for (const property of subject.properties) {
          const item = document.createElement('li'); item.textContent = property; list.append(item);
        }
        details.append(list); node.append(details);
      }
      documentNode.append(node);
    }
    if (view.relationships.length) {
      const relations = document.createElement('details');
      relations.append(faceNode('summary', 'How these parts relate'));
      const list = document.createElement('ul');
      for (const relationship of view.relationships) {
        const item = document.createElement('li');
        item.textContent = `${names.get(relationship.source) ?? relationship.source} ${relationship.kind.toLowerCase()} ${names.get(relationship.target) ?? relationship.target}`;
        list.append(item);
      }
      relations.append(list); documentNode.append(relations);
    }
    if (view.actions.length) {
      const actions = document.createElement('section');
      actions.append(faceNode('h4', 'What you can do'));
      for (const action of view.actions) {
        const control = document.createElement('form');
        control.dataset.ownerAction = action.identity;
        control.append(faceNode('h5', action.name));
        const inputs = [];
        let supported = Array.isArray(action.arguments) && action.arguments.length <= 64;
        for (const argument of action.arguments ?? []) {
          const label = faceNode('label', argument.value_name);
          let input;
          if (Array.isArray(argument.choices) && argument.choices.length) {
            if (argument.choices.length > 64 || argument.choices.includes('')) {
              supported = false;
              continue;
            }
            input = document.createElement('select');
            const prompt = document.createElement('option');
            prompt.value = ''; prompt.textContent = 'Choose an option';
            input.append(prompt);
            for (const choice of argument.choices) {
              const option = document.createElement('option');
              option.value = choice; option.textContent = choice;
              input.append(option);
            }
            input.required = true;
          } else if (Array.isArray(argument.choices) && argument.value_kind === 'value/text'
            && Number.isSafeInteger(argument.maximum_bytes) && argument.maximum_bytes <= 4096) {
            input = document.createElement('input');
            input.type = 'text'; input.maxLength = argument.maximum_bytes;
            input.addEventListener('input', () => input.setCustomValidity(
              encoder.encode(input.value).length > argument.maximum_bytes
                ? `Use at most ${argument.maximum_bytes} UTF-8 bytes.` : ''));
          } else {
            supported = false;
            continue;
          }
          label.append(input);
          control.append(label);
          inputs.push({ name: argument.name, input });
        }
        const button = faceNode('button', action.name);
        button.type = 'submit';
        button.disabled = !view.interactions_admitted || view.show_state !== 'available'
          || !supported || inputs.length !== action.arguments.length
          || action.availability !== 'available';
        control.append(button);
        if (button.disabled) control.append(faceNode('p', action.explanation ??
          (supported ? 'This owner action is unavailable.' : 'This input has no supported browser form.')));
        control.addEventListener('submit', async event => {
          event.preventDefault();
          if (faceBusy || !participation || faceView?.show_id !== view.show_id) return;
          faceBusy = true;
          button.disabled = true;
          try {
            const outcome = await participation.submitOwnerFaceInteraction({
              view: faceView, actionId: action.identity, target: action.target,
              arguments: inputs.map(({ name, input }) => ({ name, value: input.value })),
              sequence: ++actionSequence,
            });
            if (outcome.accepted !== true) throw new Error('owner did not accept the interaction');
            actionResult.textContent = `The owner accepted ${action.name}.`;
            delete actionResult.dataset.refused;
          } catch (error) {
            actionResult.textContent = `${action.name} refused: ${error.message}`;
            actionResult.dataset.refused = 'true';
          } finally {
            faceBusy = false;
            faceView = null;
            await refreshFace();
          }
        });
        actions.append(control);
      }
      documentNode.append(actions);
    }
    faceDocument.replaceChildren(documentNode);
    faceDocument.dataset.faceId = view.face_id;
    faceDocument.dataset.faceRevision = view.face_revision;
    faceDocument.dataset.showId = view.show_id;
  };
  const refreshFace = async () => {
    if (!participation || faceBusy || participation.presenceState() !== 'available') return;
    faceBusy = true; faceRefresh.disabled = true;
    const priorFace = faceView;
    faceView = null;
    delete root.dataset.ownerFaceShown;
    delete root.dataset.ownerShowAcknowledged;
    for (const button of faceDocument.querySelectorAll('[data-owner-action] button')) button.disabled = true;
    try {
      faceStatus.textContent = 'Asking the owner for its current Face…';
      const prepared = await participation.prepareOwnerFaceMask(priorFace ? {
        lastSeenRevision: priorFace.face_revision, lastSeenIdentity: priorFace.face_id,
      } : undefined);
      renderFace(prepared);
      const shown = await participation.acknowledgeOwnerFaceMask(prepared);
      root.dataset.ownerShowAcknowledged = shown.show_id;
      faceView = shown;
      renderFace(shown);
      faceStatus.textContent = 'The browser is showing the owner’s current Face.';
      delete faceStatus.dataset.refused;
      faceEvidence.textContent = JSON.stringify({ body_id: shown.body_id, face_id: shown.face_id,
        face_revision: shown.face_revision, mask_plot_id: shown.mask_plot_id,
        mask_plan_id: shown.mask_plan_id, mask_play_id: shown.mask_play_id,
        show_id: shown.show_id, show_state: shown.show_state,
        interactions_admitted: shown.interactions_admitted }, null, 2);
      root.dataset.ownerFaceShown = 'true';
    } catch (error) {
      faceStatus.textContent = `Owner Face refused: ${error.message}`;
      faceStatus.dataset.refused = 'true';
    } finally {
      faceBusy = false;
      faceRefresh.disabled = participation?.presenceState() !== 'available';
    }
  };
  const showState = state => {
    status.textContent = `Browser participation: ${state}.`;
    status.dataset.state = state;
    if (state === 'offline' || state.startsWith('refused:')) {
      if (faceView) faceStatus.textContent = 'The route to the owner is lost. The last Face is historical.';
      faceView = null;
      faceRefresh.disabled = true;
      for (const button of faceDocument.querySelectorAll('[data-owner-action] button')) button.disabled = true;
      actionResult.textContent = 'The owner window is closed; this browser has no current action return.';
      actionResult.dataset.refused = 'true';
    }
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
      ? `Body ${biography.body_id} admitted Part ${credential.part_id} on this Host and Boot. The Linux owner remains authoritative; this browser can present its Face through an owner-issued Mask Plan.`
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
          if (state === 'offline' || state.startsWith('refused:')) {
            leave.disabled = true;
            faceRefresh.disabled = true;
          } else if (state === 'admitted' && participation?.presenceState() === 'available') {
            faceRefresh.disabled = false;
            if (!faceView) queueMicrotask(refreshFace);
          }
        },
        onBiographyEvidence: biography => queueMicrotask(() => showBiography(biography)),
      });
      leave.disabled = false;
      const received = participation.biographyEvidence();
      if (received) showBiography(received);
      if (participation.presenceState() === 'available') {
        faceRefresh.disabled = false;
        queueMicrotask(refreshFace);
      }
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
  faceRefresh.addEventListener('click', refreshFace);
  globalThis.__conduitOwnerParticipation = Object.freeze({
    host: () => host.current(),
    admissionIdentity: () => host.admissionIdentity(),
    state: () => participation?.state() ?? 'not-joined',
    presence: () => participation?.presenceState() ?? 'unavailable',
    credential: () => participation?.membershipCredential() ?? null,
    biography: () => participation?.biographyEvidence() ?? null,
    face: () => faceView,
  });
}
