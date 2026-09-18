import { initializeBrowserHost } from "../../../targets/browser/host/assets/browser-host-bootstrap.mjs";
import { createBodyBirthRunner, createFirstHostRunner } from "./body-bootstrap.mjs";
import { readReviewedFormInventory, openFormSelection, persistedFormSelection } from "./reviewed-form-selection.mjs";
import { openWorkspaceSession } from "./workspace-session.mjs";
import { openWorkspacePlay } from "./workspace-play.mjs";
import { configureWorkspaceInput } from "./workspace-surface.mjs";
import { openWorkspaceLibrary } from "./workspace-library.mjs";
import { readWorkspaceHandoff, consumeWorkspaceHandoff } from "./workspace-handoff.mjs";
import { acquireBrowserBodyContinuity } from "../../../targets/browser/host/assets/browser-body-continuity.mjs";
import { openWorkspaceMembership, readBodyInvitation } from "./workspace-membership.mjs";
import { prepareWorkspaceVoicePlay } from "./workspace-voice-play.mjs";

export async function startApplication(application) {
  const root = document.querySelector('.workspace-shell');
  const nursery = root.querySelector('[data-workspace-creche]');
  const surface = root.querySelector('[data-workspace-surface]');
  const activities = root.querySelector('.activity-switcher');
  const strip = root.querySelector('.truth-strip');
  const notice = root.querySelector('[data-workspace-notice]');
  const input = root.querySelector('#form-input');
  const inspection = root.querySelector('#workspace-inspection');
  const details = inspection.querySelector('[data-inspection-content]');
  const wakeButton = root.querySelector('[data-wake-body]');
  const lullButton = root.querySelector('[data-lull-body]');
  const fulfillButton = root.querySelector('[data-fulfill-body]');
  const tutorial = root.querySelector('[data-body-tutorial]');
  const tutorialPresentation = application.presentationFor(tutorial);
  let tutorialRevision = 0;
  const fail = error => {
    notice.textContent = error instanceof Error ? error.message : String(error);
    notice.dataset.disposition = 'refused';
  };
  try {
    const invitation = readBodyInvitation(globalThis.location);
    if (!invitation) await acquireBrowserBodyContinuity();
    const host = await initializeBrowserHost({ runtimeBytes: application.bytes('runtime'), durable: !invitation });
    const session = openWorkspaceSession({ host, storage: application.storage });
    const source = application.text('reviewed-form-inventory');
    const inventory = readReviewedFormInventory(host.runtime, source);
    const catalogSource = application.text('reviewed-form-catalog');
    const catalog = readWorkspaceCatalog(catalogSource);
    let handoff = null, handoffFailure = null;
    try { handoff = readWorkspaceHandoff(globalThis.location, inventory); }
    catch (error) { handoffFailure = error; }
    const retainedSelection = await application.storage.readJson('form-selection');
    let selection = openFormSelection(inventory, retainedSelection);
    if (retainedSelection === null) {
      const scratch = inventory.forms.find(form => form.name === 'memory_lantern');
      const chime = typeof window.AudioContext === 'function' ? inventory.forms.find(form => form.name === 'startup_chime') : null;
      selection = { selected: [scratch, chime].filter(Boolean), refusals: [] };
    }
    const restored = invitation ? null : await session.restore();
    if (handoff && !session.current()) {
      selection = openFormSelection(inventory, persistedFormSelection(inventory, selection.selected), handoff);
      await application.storage.writeJson('form-selection', persistedFormSelection(inventory, selection.selected));
      await consumeWorkspaceHandoff({ host, applicationId: application.manifest.applicationId });
    }
    let saving = Promise.resolve();
    let selected = session.foreground()?.checked_form_id;
    let playback = { state: 'Lulled', detail: 'Its Forms can wake here.' };
    let play = null;
    let library = null, membership = null, editing = false;
    const renderTutorial = () => {
      if (!session.current()) return;
      const revision = ++tutorialRevision;
      tutorialPresentation.present('body-tutorial', session.tutorialView(revision, playback.state), { onEvent(event) {
        tutorialPresentation.nextEvent('body-tutorial');
        if (event.revision !== tutorialRevision || event.kind !== 1 || event.value.length !== 0) {
          fail(new Error('This tutorial action is stale'));
        } else if (event.action === 'body.wake') play?.wake(true).catch(fail);
        else if (event.action === 'body.inspect-lifecycle') inspect('lifecycle');
        else if (event.action === 'body.open-library') { surface.hidden = true; inspection.hidden = true; library?.show(); }
        else if (event.action === 'body.use-current') { surface.hidden = false; inspection.hidden = true; library?.hide(); if (!input.disabled) input.focus(); }
        else fail(new Error('Unknown tutorial action'));
      } });
    };
    const bodyChanged = () => {
      saving = saving.then(() => session.save()).then(async () => {
        if (!session.current()?.here_part_id) {
          session.attachHere();
          await session.save();
        }
        await session.arrive();
        const resident = session.current().initial_forms;
        if (handoff && resident.some(form => form.checked_form_id === handoff.checked_form_id)) {
          await session.selectForm({ source_document_id: handoff.source_document_id, checked_form_id: handoff.checked_form_id });
        }
        const foreground = catalog.forms.find(form => form.checked_form_id === session.foreground()?.checked_form_id);
        if (foreground?.required_kinds.includes('sound/startup-chime')) {
          const visible = resident.find(form => catalog.forms.some(candidate => candidate.checked_form_id === form.checked_form_id && candidate.required_kinds.some(kind => kind.startsWith('presentation/'))));
          if (visible) await session.selectForm(visible);
        }
        selected = session.foreground()?.checked_form_id;
        render();
      }).catch(fail);
    };
    const inspect = kind => {
      library?.hide();
      membership && (membership.isOpen() ? membership.close() : null);
      const body = session.current();
      const form = catalog.forms.find(form => form.checked_form_id === selected);
      const evidence = session.evidence();
      const heading = inspection.querySelector('h2');
      heading.textContent = kind === 'form' ? (form?.title ?? 'This Form') : kind === 'flow' ? 'Inside this Form' : `${body.friendly_name} · ${playback.state}`;
      details.replaceChildren();
      const text = document.createElement('p'); text.className = 'inspection-explanation';
      text.textContent = kind === 'lifecycle' ? playback.detail : kind === 'flow' ? 'The checked source describes this Form’s meaning. Its exact realization appears below when admitted.' : 'An installed Form in this Body. Opening its surface keeps the current Play.';
      details.append(text);
      const pre = document.createElement('pre');
      pre.textContent = kind === 'lifecycle' ? JSON.stringify({ body: evidence?.evidence, realization: evidence?.realization, active_observation: play?.evidence(), terminal: playback.terminal, refusal: playback.refusal }, null, 2)
        : kind === 'flow' && evidence?.realization ? JSON.stringify(evidence.realization.plan.forms.find(item => item.form.checked_form_id === selected), null, 2) : (form?.source ?? 'Source unavailable');
      const disclosure = document.createElement('details'), summary = document.createElement('summary');
      summary.textContent = kind === 'lifecycle' ? 'Exact lifecycle evidence' : kind === 'flow' && evidence?.realization ? 'Exact Plan' : 'Checked source';
      disclosure.append(summary, pre); details.append(disclosure);
      surface.hidden = true; inspection.hidden = false;
      for (const button of strip.querySelectorAll('button')) button.setAttribute('aria-expanded', String(button.dataset.inspect === kind));
      heading.focus();
    };
    for (const button of strip.querySelectorAll('[data-inspect]')) button.addEventListener('click', () => inspect(button.dataset.inspect));
    root.querySelector('[data-close-inspection]').addEventListener('click', () => {
      inspection.hidden = true; surface.hidden = false;
      const button = strip.querySelector('[aria-expanded="true"]'); button?.setAttribute('aria-expanded', 'false'); button?.focus();
    });
    const showSelected = () => {
      const form = catalog.forms.find(item => item.checked_form_id === selected);
      const partition = session.evidence()?.realization?.plan.forms.find(item => item.form.checked_form_id === selected);
      root.querySelector('#surface-title').textContent = form?.title ?? 'No Forms installed';
      root.querySelector('[data-surface-invitation]').textContent = configureWorkspaceInput(input, form, partition);
      root.querySelector('.current-form').textContent = form?.title ?? 'Your Forms';
      root.querySelector('[data-flow-label]').textContent = session.evidence()?.foreground_flow ?? 'Not yet planned';
      for (const button of activities.querySelectorAll('[data-checked-form-id]')) button.setAttribute('aria-pressed', String(button.dataset.checkedFormId === selected));
      const visible = new Set(partition?.plan.fragments.flatMap(fragment => fragment.placements.map(placement => placement.placement_id)) ?? []);
      for (const output of root.querySelectorAll('[data-form-output] > output')) output.hidden = !visible.has(output.dataset.placementId);
      for (const button of strip.querySelectorAll('[data-inspect="form"], [data-inspect="flow"]')) button.disabled = !form;
    };
    function render() {
      const body = session.current();
      tutorial.hidden = !body;
      renderTutorial();
      membership?.render();
      const arriving = !body || (!body.here_part_id && body.state !== 'FULFILLED');
      const joining = membership?.isJoining();
      nursery.hidden = !arriving || joining;
      surface.hidden = arriving || !inspection.hidden || library?.isOpen() || membership?.isOpen();
      activities.hidden = arriving || membership?.isOpen();
      strip.hidden = arriving || membership?.isOpen();
      root.querySelector('[data-body-name]').textContent = body?.friendly_name ?? 'Your body starts here';
      root.querySelector('[data-body-state]').textContent = body ? body.state.toLowerCase() : 'Crèche';
      if (arriving && joining) {
        document.title = 'Body invitation · Conduit';
        return;
      }
      if (arriving && !joining) {
        document.title = 'Birth your Body · Conduit';
        const slot = nursery.querySelector('[data-creche-content]');
        slot.replaceChildren(body
          ? createFirstHostRunner({ host, presentationFor: application.presentationFor, nextSequence: session.nextMembershipSequence, onBodyChanged: bodyChanged })
          : createBodyBirthRunner({
            source, sourceKey: 'workspace-creche', listingId: 'workspace-forms', host,
            presentationFor: application.presentationFor, inventory, initialSelection: selection,
            nextSequence: session.nextSequence, onBodyChanged: bodyChanged,
            onSelection(selected) {
              selection = selected === null ? openFormSelection(inventory) : { selected, refusals: [] };
              saving = saving.then(() => selected === null
                ? application.storage.deleteJson('form-selection')
                : application.storage.writeJson('form-selection', persistedFormSelection(inventory, selected))).catch(fail);
            },
          }));
        return;
      }
      document.title = `${body.friendly_name} · Conduit`;
      root.dataset.bodyId = body.body_id;
      root.querySelector('[data-play-state]').textContent = playback.state;
      root.querySelector('#surface-guidance').textContent = playback.detail;
      lullButton.hidden = !body.initial_forms.length;
      fulfillButton.hidden = body.state === 'FULFILLED';
      fulfillButton.disabled = body.state !== 'LULLED' || Boolean(session.persistenceFailure());
      if (body.state === 'FULFILLED') {
        wakeButton.hidden = true;
        lullButton.hidden = true;
      }
      if (!body.initial_forms.length) {
        root.querySelector('#surface-guidance').textContent = 'This Body can remain lulled.';
        wakeButton.hidden = true;
        lullButton.hidden = true;
      }
      notice.textContent = `${body.initial_forms.length} Form${body.initial_forms.length === 1 ? '' : 's'} in ${body.friendly_name}.`;
      if (!body.initial_forms.some(form => form.checked_form_id === selected)) selected = body.initial_forms[0]?.checked_form_id;
      activities.replaceChildren(...body.initial_forms.map(form => {
        const button = document.createElement('button'); button.type = 'button';
        button.textContent = catalog.forms.find(item => item.checked_form_id === form.checked_form_id)?.title ?? form.name;
        button.dataset.checkedFormId = form.checked_form_id;
        button.addEventListener('click', () => {
          if (editing) return;
          selected = form.checked_form_id;
          session.selectForm({ source_document_id: form.source_document_id, checked_form_id: form.checked_form_id }).catch(fail);
          library?.hide(); inspection.hidden = true; surface.hidden = false;
          showSelected();
          if (!input.disabled) input.focus();
        });
        return button;
      }));
      const browse = document.createElement('button'); browse.type = 'button';
      browse.textContent = '+ Forms'; browse.dataset.openLibrary = '';
      browse.addEventListener('click', () => {
        if (editing) return;
        surface.hidden = true; inspection.hidden = true; library.show();
      });
      if (body.state !== 'FULFILLED') activities.append(browse);
      if (!play) play = openWorkspacePlay({ host, session, source, planningLines: () => membership?.planningLines() ?? [], foregroundForm: () => selected, inputTarget: input, outputRoot: root.querySelector('[data-form-output]'),
        async prepareExternal(proposal) {
          const distributed = proposal.plan.forms.filter(form => form.plan.fragments.length > 1);
          if (!distributed.length) return null;
          if (distributed.length !== 1) throw new Error('This Body Plan exceeds the one external Form bound');
          const plan = distributed[0].plan;
          const peer = plan.fragments.find(fragment => fragment.host_id !== host.hostId || fragment.boot_id !== host.bootId);
          const joined = peer && membership?.executionLine(peer.host_id, peer.boot_id);
          if (!joined) throw new Error('The planned Voice Host Line is no longer current');
          await joined.line.installBodyContext(session.conversationContext());
          const voice = await prepareWorkspaceVoicePlay({ api: host.runtime,
            localAdvertisement: host.membership.advertisement(), joined, plan,
            outputRoot: root.querySelector('[data-form-output]') });
          return Object.freeze({ planId: plan.plan_id, identity: voice.identity,
            updateContext: () => joined.line.installBodyContext(session.conversationContext()),
            run: () => voice.run(), close: () => voice.close() });
        }, onState(state) {
        playback = state;
        renderTutorial();
        root.querySelector('[data-play-state]').textContent = state.state;
        root.querySelector('[data-body-state]').textContent = session.current().state.toLowerCase();
        root.querySelector('#surface-guidance').textContent = state.detail;
        wakeButton.disabled = Boolean(session.persistenceFailure()) || !['Lulled', 'Refused'].includes(state.state);
        lullButton.disabled = !['Playing', 'Idle', 'Completed', 'Failed'].includes(state.state);
        fulfillButton.disabled = !['Lulled', 'Playing', 'Idle', 'Completed', 'Failed'].includes(state.state)
          || Boolean(session.persistenceFailure());
        fulfillButton.hidden = state.state === 'Fulfilled';
        input.disabled = state.state !== 'Playing';
        input.inert = input.disabled;
        input.tabIndex = input.disabled ? -1 : 0;
        input.setAttribute('aria-disabled', String(input.disabled));
        wakeButton.hidden = session.current().initial_forms.length === 0 || ['Playing', 'Idle', 'Preparing', 'Fulfilled'].includes(state.state);
        if (state.state === 'Fulfilled') lullButton.hidden = true;
        showSelected();
        if (state.state === 'Playing' && input.dataset.acceptsInput === 'true') input.focus();
      } });
      showSelected();
    }
    const useForm = async (form, expectedRevision) => {
      if (editing) throw new Error('A Body transition is already in progress');
      editing = true;
      try {
        const identity = { source_document_id: form.source_document_id, checked_form_id: form.checked_form_id };
        const installed = session.current().initial_forms.some(item => item.source_document_id === identity.source_document_id && item.checked_form_id === identity.checked_form_id);
        if (installed) await session.selectForm(identity);
        else await play.changeWorkset('Install', identity, expectedRevision);
        selected = session.foreground()?.checked_form_id;
        library.hide(); inspection.hidden = true;
        render();
        if (!installed || session.current().state === 'LULLED') await play.wake(true);
        if (!input.disabled && !input.hidden) input.focus();
      } finally { editing = false; }
    };
    const removeForm = async (form, expectedRevision) => {
      if (editing) throw new Error('A Body transition is already in progress');
      editing = true;
      try {
        await play.changeWorkset('Remove', { source_document_id: form.source_document_id, checked_form_id: form.checked_form_id }, expectedRevision);
        selected = session.foreground()?.checked_form_id;
        render();
        if (session.current().initial_forms.length) await play.wake();
      } finally { editing = false; }
    };
    library = openWorkspaceLibrary({ panel: root.querySelector('#workspace-library'), session, source: catalogSource, inventory: catalog,
      planningLines: () => membership?.planningLines() ?? [],
      presentationFor: application.presentationFor, onUse: useForm, onRemove: removeForm, onFailure: fail,
      onClose() { library.hide(); render(); root.querySelector('[data-open-library]')?.focus(); },
    });
    globalThis.__conduitWorkspace = Object.freeze({ host, current: session.current, evidence: session.evidence, state: () => structuredClone(playback), settled: () => saving.then(session.settled) });
    membership = openWorkspaceMembership({ root, session, host, invitation, presentationFor: application.presentationFor,
      async beforeAdmission() {
        if (['Playing', 'Idle', 'Completed', 'Failed'].includes(playback.state)) await play?.lull();
      },
      onChanged: render,
      onFailure: fail,
    });
    wakeButton.addEventListener('click', () => play?.wake(true).catch(fail));
    lullButton.addEventListener('click', () => play?.lull().catch(fail));
    fulfillButton.addEventListener('click', () => {
      if (!globalThis.confirm('Finish this Body permanently? Its biography remains available, but it cannot wake or change again.')) return;
      play?.fulfill().then(render).catch(fail);
    });
    if (session.current()?.here_part_id && session.current().state !== 'FULFILLED') await session.arrive();
    render();
    if (handoff && session.current()?.here_part_id) {
      await consumeWorkspaceHandoff({ host, applicationId: application.manifest.applicationId });
      await useForm(handoff, session.current().workload_revision);
    } else if (restored?.resume_wake && session.current()?.here_part_id && session.current().initial_forms.length) await play.wake();
    if (handoffFailure) fail(handoffFailure);
    globalThis.addEventListener('pagehide', () => { play?.close(); membership?.dispose(); });
  } catch (error) { fail(error); }
}

function readWorkspaceCatalog(source) {
  const catalog = JSON.parse(source);
  if (catalog?.schema !== 'conduit.workspace/reviewed-form-catalog@2'
      || !Number.isSafeInteger(catalog.maximum_forms) || catalog.maximum_forms < 1
      || !Array.isArray(catalog.forms) || catalog.forms.length > catalog.maximum_forms) {
    throw new Error('reviewed Workspace Form catalog is malformed or over capacity');
  }
  const identities = new Set();
  for (const form of catalog.forms) {
    if (typeof form?.title !== 'string' || typeof form.entry !== 'string'
        || typeof form.source !== 'string' || typeof form.source_document_id !== 'string'
        || typeof form.checked_form_id !== 'string' || !Array.isArray(form.required_kinds)
        || !Number.isSafeInteger(form.presentation_profile)
        || form.presentation_profile < 0 || form.presentation_profile > 3
        || typeof form.unavailable_hint !== 'string' || form.unavailable_hint.length < 1
        || identities.has(form.checked_form_id)) {
      throw new Error('reviewed Workspace Form catalog contains an invalid or duplicate entry');
    }
    form.name = form.entry;
    identities.add(form.checked_form_id);
  }
  return catalog;
}
