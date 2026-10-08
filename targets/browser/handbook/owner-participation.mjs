// Optional, explicitly authorized participation in a Body owned by a running
// Linux Host. All admission and biography facts come from the production SDK.
const encoder = new TextEncoder();

// A Mask-local layout choice over the current Face, never a second Todo store.
export function projectFaceCollections(view) {
  const subjects = new Map((view.subjects ?? []).map(subject => [subject.identity, subject]));
  const collections = [];
  const placed = new Set();
  for (const collection of view.subjects ?? []) {
    if (collection.role !== 'Collection' || !['Primary', 'Context'].includes(collection.disclosure)) continue;
    const items = (view.relationships ?? [])
      .filter(relation => relation.kind === 'Contains' && relation.source === collection.identity)
      .map(relation => subjects.get(relation.target))
      .filter(item => item?.role === 'Item');
    if (items.some(item => {
      const complete = item.flags?.filter(flag => flag.name === 'complete');
      return complete?.length !== 1 || typeof complete[0].value !== 'boolean'
        || item.disclosure !== (complete[0].value ? 'SelectedDetail' : 'Primary');
    })) continue;
    const open = items.filter(item => !item.flags.find(flag => flag.name === 'complete').value);
    const completed = items.filter(item => item.flags.find(flag => flag.name === 'complete').value);
    const count = `${open.length} ${open.length === 1 ? 'thing' : 'things'} left · ${completed.length} completed`;
    // The collection count is a Mask-local arrangement of the same flags.
    // Only fold away a Status subject when its exact Face wording agrees.
    const matchingStatus = (view.relationships ?? [])
      .filter(relation => relation.kind === 'Contains' && relation.source === collection.identity)
      .map(relation => subjects.get(relation.target))
      .find(subject => subject?.role === 'Status' && subject.text?.length === 1
        && subject.text[0] === count);
    collections.push({ collection, open, completed, count });
    placed.add(collection.identity);
    for (const item of items) placed.add(item.identity);
    if (matchingStatus) placed.add(matchingStatus.identity);
  }
  return { collections, placed };
}

export function faceActionReady(view, action) {
  return view.interactions_admitted === true && view.show_state === 'available'
    && action.availability === 'available';
}

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

export function checkedOwnerRouteEvidence(view, hostId, bootId) {
  const route = view?.route;
  if (view?.show_state !== 'available' || !route ||
      route.mask_host_id !== hostId || route.mask_boot_id !== bootId ||
      [view.body_id, view.face_id, view.face_revision, view.mask_plot_id, view.mask_plan_id,
        view.mask_play_id, view.show_id, route.plan_id,
        route.owner_host_id, route.owner_boot_id].some(value => typeof value !== 'string' || !value)) {
    throw new Error('The owner Show has no current sealed browser route to inspect.');
  }
  return {
    body_id: view.body_id,
    face_id: view.face_id,
    face_revision: view.face_revision,
    selected_mask_plot_id: view.mask_plot_id,
    route_plan_id: route.plan_id,
    owner_host_id: route.owner_host_id,
    owner_boot_id: route.owner_boot_id,
    mask_host_id: route.mask_host_id,
    mask_boot_id: route.mask_boot_id,
    mask_plan_id: view.mask_plan_id,
    mask_play_id: view.mask_play_id,
    show_id: view.show_id,
    show_state: view.show_state,
    interactions_admitted: view.interactions_admitted,
  };
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
      <details><summary>Inspect the current Face, route, Plan, and Show</summary><pre data-owner-face-evidence>No Show yet.</pre></details>
    </section>
    <section class="owner-wardrobe" aria-labelledby="owner-wardrobe-title">
      <header><p class="eyebrow">Owner policy · at rest</p><h3 id="owner-wardrobe-title">Choose how this Body can meet you</h3>
        <p>Inspect the owner's eligible Mask routes, what is worn, and its preference order. Wear and doff change that Body policy; Prefer replaces the preference list with one Mask. These changes work only while the Body is lulled. The owner may refuse while a Plot is running; refresh after Lull. A selected route needs a fresh Show before interaction.</p></header>
      <button type="button" data-owner-wardrobe-refresh disabled>Inspect current wardrobe</button>
      <p role="status" data-owner-wardrobe-status>Join the Body to inspect its wardrobe.</p>
      <div data-owner-wardrobe-rows></div>
      <details><summary>Exact owner wardrobe report</summary><pre data-owner-wardrobe-evidence>No owner report yet.</pre></details>
    </section>
    <section aria-labelledby="owner-speech-title"><h3 id="owner-speech-title">Hear this view</h3>
      <p>Ask the installed Linux owner to read the current Face through its selected speaker. Playback happens on that Host; this browser shows its reported outcome.</p>
      <button type="button" data-owner-speech-start disabled>Read this view aloud</button>
      <button type="button" data-owner-speech-status disabled>Check reading</button>
      <button type="button" data-owner-speech-stop disabled>Stop reading</button>
      <p role="status" data-owner-speech-result>Join and show the Face first.</p></section>
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
  const wardrobeRefresh = root.querySelector('[data-owner-wardrobe-refresh]');
  const wardrobeStatus = root.querySelector('[data-owner-wardrobe-status]');
  const wardrobeRows = root.querySelector('[data-owner-wardrobe-rows]');
  const wardrobeEvidence = root.querySelector('[data-owner-wardrobe-evidence]');
  const speechStart = root.querySelector('[data-owner-speech-start]');
  const speechStatus = root.querySelector('[data-owner-speech-status]');
  const speechStop = root.querySelector('[data-owner-speech-stop]');
  const speechResult = root.querySelector('[data-owner-speech-result]');
  let host, participation, expectedBodyId = null, faceView = null, faceBusy = false, actionSequence = 0;
  let speechOperationId = null, speechBusy = false;
  let wardrobeReport = null, wardrobeBusy = false;
  const renderWardrobe = current => {
    const list = document.createElement('ul');
    list.className = 'owner-wardrobe-list';
    for (const route of current.rows) {
      const item = document.createElement('li');
      const name = document.createElement('h4'); name.textContent = route.maskName; item.append(name);
      const facts = document.createElement('p');
      facts.textContent = `Host ${route.hostId} · ${route.available ? 'available' : 'unavailable'} · ${route.worn ? 'worn' : 'not worn'} · ${route.preferenceRank ? `preference ${route.preferenceRank}` : 'not preferred'}${route.selected ? ' · selected Show route' : ''}`;
      item.append(facts);
      const controls = document.createElement('div'); controls.className = 'owner-wardrobe-controls';
      for (const [verb, label, enabled] of [['wear', 'Wear', !route.worn], ['doff', 'Doff', route.worn], ['prefer', 'Prefer only', route.worn && (route.preferenceRank !== 1 || current.preferenceCount !== 1)]]) {
        const button = document.createElement('button'); button.type = 'button';
        button.textContent = `${label} ${route.maskName}`;
        button.disabled = !enabled;
        button.addEventListener('click', () => changeWardrobe(current.raw, route.routeId, verb));
        controls.append(button);
      }
      item.append(controls); list.append(item);
    }
    if (!current.rows.length) {
      const empty = document.createElement('p'); empty.textContent = 'The owner reports no admitted Mask routes.';
      wardrobeRows.replaceChildren(empty);
    } else wardrobeRows.replaceChildren(list);
    wardrobeEvidence.textContent = JSON.stringify(current.raw, null, 2);
    wardrobeStatus.textContent = `Owner wardrobe revision ${current.revision}. ${current.rows.length} admitted routes. ${current.rows.filter(row => row.available).length} currently available. ${current.freshShowRequired ? 'A fresh Show is required.' : current.selectedShowId ? 'The owner reports a selected Show.' : 'No selected Show is reported.'}`;
    delete wardrobeStatus.dataset.refused;
  };
  const refreshWardrobe = async () => {
    if (!participation || wardrobeBusy || participation.presenceState() !== 'available') return;
    wardrobeBusy = true; wardrobeRefresh.disabled = true;
    for (const button of wardrobeRows.querySelectorAll('button')) button.disabled = true;
    wardrobeStatus.textContent = 'Inspecting the current owner wardrobe…';
    try {
      const current = await participation.inspectOwnerWardrobe();
      wardrobeReport = current.raw;
      if (faceView && (current.freshShowRequired || current.selectedShowId !== faceView.show_id)) {
        faceView = null;
        faceDocument.replaceChildren();
        faceStatus.textContent = 'The owner selected another Show. Refresh this Face before interaction.';
        delete root.dataset.ownerFaceShown;
        delete root.dataset.ownerShowAcknowledged;
        speechControls();
      }
      renderWardrobe(current);
    } catch (error) {
      wardrobeReport = null;
      wardrobeRows.replaceChildren();
      wardrobeStatus.textContent = `Owner wardrobe unavailable: ${error.message}`;
      wardrobeStatus.dataset.refused = 'true';
      wardrobeEvidence.textContent = 'Previous wardrobe evidence is historical.';
    } finally { wardrobeBusy = false; wardrobeRefresh.disabled = participation?.presenceState() !== 'available'; }
  };
  const changeWardrobe = async (report, routeId, verb) => {
    if (!participation || wardrobeBusy || report !== wardrobeReport) return;
    wardrobeBusy = true; wardrobeRefresh.disabled = true;
    for (const button of wardrobeRows.querySelectorAll('button')) button.disabled = true;
    wardrobeStatus.textContent = `Asking the owner to ${verb} this Mask while the Body is at rest…`;
    try {
      const current = await participation.changeOwnerWardrobe(report, routeId, verb);
      wardrobeReport = current.raw;
      faceView = null;
      faceDocument.replaceChildren();
      delete root.dataset.ownerFaceShown;
      delete root.dataset.ownerShowAcknowledged;
      faceStatus.textContent = 'Wardrobe changed. Ask the owner for a fresh Face and Show.';
      faceRefresh.disabled = false;
      speechControls();
      renderWardrobe(current);
    } catch (error) {
      wardrobeReport = null;
      faceView = null;
      faceDocument.replaceChildren();
      delete root.dataset.ownerFaceShown;
      delete root.dataset.ownerShowAcknowledged;
      faceStatus.textContent = 'Wardrobe action refused or uncertain. Ask the owner for a fresh Face and Show.';
      speechControls();
      wardrobeRows.replaceChildren();
      wardrobeStatus.textContent = `Owner refused ${verb}: ${error.message}. Inspect again before another change.`;
      wardrobeStatus.dataset.refused = 'true';
      wardrobeEvidence.textContent = 'The last wardrobe report is historical.';
    } finally { wardrobeBusy = false; wardrobeRefresh.disabled = participation?.presenceState() !== 'available'; }
  };
  const speechControls = () => {
    const current = participation?.presenceState() === 'available';
    speechStart.disabled = !current || !faceView || faceBusy || speechBusy || Boolean(speechOperationId);
    speechStatus.disabled = !current || !speechOperationId || speechBusy;
    speechStop.disabled = !current || !speechOperationId || speechBusy;
    if (speechOperationId) {
      faceRefresh.disabled = true;
      for (const button of faceDocument.querySelectorAll('[data-owner-action] button')) button.disabled = true;
    } else if (current && !faceBusy) {
      faceRefresh.disabled = false;
    }
  };
  const faceNode = (tag, text, className) => {
    const node = document.createElement(tag);
    node.textContent = text;
    if (className) node.className = className;
    return node;
  };
  const renderFace = view => {
    const names = new Map(view.subjects.map(subject => [subject.identity, subject.name]));
    const documentNode = faceNode('article', '', 'owner-face-document');
    const { collections, placed } = projectFaceCollections(view);
    const actionReady = action => faceActionReady(view, action);
    const actionControl = (action, itemName) => {
      const control = document.createElement('form');
      control.dataset.ownerAction = action.identity;
      if (!itemName) control.append(faceNode('h5', action.name));
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
      const button = faceNode('button', itemName ? `${action.name} ${itemName}` : action.name);
      button.type = 'submit';
      button.disabled = !actionReady(action) || !supported || inputs.length !== action.arguments.length;
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
      return control;
    };
    const renderSubject = subject => {
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
      return node;
    };
    for (const { collection, open, completed, count } of collections) {
      const section = document.createElement('section');
      section.className = 'owner-face-collection';
      section.append(faceNode('h4', collection.name));
      for (const text of collection.text) section.append(faceNode('p', text));
      section.append(faceNode('p', count, 'owner-face-collection-count'));
      const openList = document.createElement('ol');
      openList.className = 'owner-face-items';
      openList.setAttribute('aria-label', `Things left in ${collection.name}`);
      for (const item of open) {
        const row = document.createElement('li');
        row.append(faceNode('span', item.name, 'owner-face-item-name'));
        for (const action of view.actions.filter(action => action.target === item.identity
          && actionReady(action) && action.disclosure === 'CurrentAction')) {
          row.append(actionControl(action, item.name));
        }
        openList.append(row);
      }
      if (open.length) section.append(openList);
      else section.append(faceNode('p', 'Nothing left to do.', 'owner-face-empty'));
      if (completed.length) {
        const details = document.createElement('details');
        details.className = 'owner-face-completed';
        details.append(faceNode('summary', `Show ${completed.length} completed ${completed.length === 1 ? 'item' : 'items'}`));
        const completedList = document.createElement('ol');
        completedList.className = 'owner-face-items';
        completedList.setAttribute('aria-label', `Completed items in ${collection.name}`);
        for (const item of completed) {
          const row = document.createElement('li');
          row.append(faceNode('span', item.name, 'owner-face-item-name'));
          for (const action of view.actions.filter(action => action.target === item.identity
            && actionReady(action) && action.disclosure === 'CurrentAction')) {
            row.append(actionControl(action, item.name));
          }
          completedList.append(row);
        }
        details.append(completedList); section.append(details);
      }
      for (const action of view.actions.filter(action => action.target === collection.identity
        && actionReady(action) && action.disclosure === 'CurrentAction')) {
        section.append(actionControl(action));
      }
      documentNode.append(section);
    }
    const inspection = document.createElement('details');
    inspection.append(faceNode('summary', 'Inspect other Face subjects'));
    for (const subject of view.subjects) {
      if (placed.has(subject.identity)) continue;
      const main = ['Primary', 'Context'].includes(subject.disclosure)
        && (!collections.length || subject.role === 'Status');
      (main ? documentNode : inspection).append(renderSubject(subject));
    }
    if (inspection.children.length > 1) documentNode.append(inspection);
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
      const secondary = document.createElement('details');
      secondary.append(faceNode('summary', 'More actions and unavailable controls'));
      for (const action of view.actions) {
        if (placed.has(action.target) && actionReady(action)
          && action.disclosure === 'CurrentAction') continue;
        const container = actionReady(action)
          && action.disclosure === 'CurrentAction' ? actions : secondary;
        container.append(actionControl(action));
      }
      if (actions.children.length > 1) documentNode.append(actions);
      if (secondary.children.length > 1) documentNode.append(secondary);
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
    faceEvidence.textContent = 'Checking the current owner route and Show…';
    for (const button of faceDocument.querySelectorAll('[data-owner-action] button')) button.disabled = true;
    try {
      faceStatus.textContent = 'Asking the owner for its current Face…';
      const prepared = await participation.prepareOwnerFaceMask(priorFace ? {
        lastSeenRevision: priorFace.face_revision, lastSeenIdentity: priorFace.face_id,
      } : undefined);
      renderFace(prepared);
      const shown = await participation.acknowledgeOwnerFaceMask(prepared);
      const inspectedRoute = checkedOwnerRouteEvidence(shown, participation.hostId, participation.bootId);
      root.dataset.ownerShowAcknowledged = shown.show_id;
      faceView = shown;
      renderFace(shown);
      speechControls();
      faceStatus.textContent = 'The browser is showing the owner’s current Face.';
      delete faceStatus.dataset.refused;
      faceEvidence.textContent = JSON.stringify(inspectedRoute, null, 2);
      root.dataset.ownerFaceShown = 'true';
    } catch (error) {
      faceStatus.textContent = `Owner Face refused: ${error.message}`;
      faceStatus.dataset.refused = 'true';
      faceEvidence.textContent = 'No current sealed browser Show. Previous route evidence is historical.';
    } finally {
      faceBusy = false;
      faceRefresh.disabled = participation?.presenceState() !== 'available';
      speechControls();
      if (!wardrobeReport) queueMicrotask(refreshWardrobe);
    }
  };
  const showState = state => {
    status.textContent = `Browser participation: ${state}.`;
    status.dataset.state = state;
    if (state === 'offline' || state.startsWith('refused:')) {
      wardrobeReport = null;
      wardrobeRefresh.disabled = true;
      for (const button of wardrobeRows.querySelectorAll('button')) button.disabled = true;
      wardrobeStatus.textContent = 'The owner window is closed. Wardrobe evidence is historical.';
      wardrobeStatus.dataset.refused = 'true';
      wardrobeEvidence.textContent = 'Previous wardrobe evidence is historical.';
      if (faceView) faceStatus.textContent = 'The route to the owner is lost. The last Face is historical.';
      faceView = null;
      faceRefresh.disabled = true;
      faceEvidence.textContent = 'No current sealed browser Show. Previous route evidence is historical.';
      for (const button of faceDocument.querySelectorAll('[data-owner-action] button')) button.disabled = true;
      actionResult.textContent = 'The owner window is closed; this browser has no current action return.';
      actionResult.dataset.refused = 'true';
      speechOperationId = null;
      speechResult.textContent = 'The owner route was lost. The owner requests cancellation; return for a new current Show.';
      speechControls();
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
            wardrobeRefresh.disabled = true;
          } else if (state === 'admitted' && participation?.presenceState() === 'available') {
            faceRefresh.disabled = false;
            wardrobeRefresh.disabled = false;
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
        wardrobeRefresh.disabled = false;
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
    showState('offline');
  });
  faceRefresh.addEventListener('click', refreshFace);
  wardrobeRefresh.addEventListener('click', refreshWardrobe);
  speechStart.addEventListener('click', async () => {
    if (!participation || !faceView || speechBusy || speechOperationId) return;
    speechBusy = true; speechControls();
    try {
      const reply = await participation.selectedDirectSpeechStart(faceView);
      if (reply.outcome !== 'started' || !reply.operation_id) throw new Error('owner did not report a started reading');
      speechOperationId = reply.operation_id;
      speechResult.textContent = 'The owner started reading this Show. Check its outcome or stop it.';
      delete speechResult.dataset.refused;
    } catch (error) {
      speechResult.textContent = `Reading refused or uncertain: ${error.message}`;
      speechResult.dataset.refused = 'true';
    } finally { speechBusy = false; speechControls(); }
  });
  speechStatus.addEventListener('click', async () => {
    if (!participation || !speechOperationId || speechBusy) return;
    speechBusy = true; speechControls();
    try {
      const reply = await participation.selectedDirectSpeechStatus(speechOperationId);
      if (reply.outcome !== 'status' || reply.operation_id !== speechOperationId) throw new Error('mismatched owner reading');
      const status = reply.status;
      speechResult.textContent = status?.state === 'running' ? 'The owner is still reading.'
        : `Owner reading ended: ${status?.outcome ?? 'unknown'}.` +
          (status?.source_show_still_current === false ? ' That source Show is now historical.' : '');
      if (status?.state !== 'running') speechOperationId = null;
    } catch (error) { speechResult.textContent = `Reading status unavailable: ${error.message}`; speechResult.dataset.refused = 'true'; }
    finally { speechBusy = false; speechControls(); if (!speechOperationId) queueMicrotask(refreshFace); }
  });
  speechStop.addEventListener('click', async () => {
    if (!participation || !speechOperationId || speechBusy) return;
    speechBusy = true; speechControls();
    try {
      const reply = await participation.selectedDirectSpeechStop(speechOperationId);
      if (reply.outcome !== 'stop-requested' || reply.operation_id !== speechOperationId) throw new Error('mismatched owner reading');
      speechResult.textContent = 'Stop requested. Check reading for the terminal outcome.';
    } catch (error) { speechResult.textContent = `Stop refused or uncertain: ${error.message}`; speechResult.dataset.refused = 'true'; }
    finally { speechBusy = false; speechControls(); }
  });
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
