import { initializeBrowserHost } from "../../../targets/browser/host/assets/browser-host-bootstrap.mjs";
import { createBodyBirthRunner, createFirstHostRunner } from "../../creche/browser/creche-lifecycle.mjs";
import { readReviewedFormInventory, openFormSelection, persistedFormSelection } from "../../creche/browser/creche-form-selection.mjs";
import { openWorkspaceSession } from "./workspace-session.mjs";
import { openWorkspacePlay } from "./workspace-play.mjs";
import { openWorkspaceLibrary } from "./workspace-library.mjs";
import { acquireBrowserBodyContinuity } from "../../../targets/browser/host/assets/browser-body-continuity.mjs";

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
  const fail = error => {
    notice.textContent = error instanceof Error ? error.message : String(error);
    notice.dataset.disposition = 'refused';
  };
  try {
    await acquireBrowserBodyContinuity();
    const host = await initializeBrowserHost({ runtimeBytes: application.bytes('runtime') });
    const session = openWorkspaceSession({ host, storage: application.storage });
    const source = application.text('reviewed-form-inventory');
    const inventory = readReviewedFormInventory(host.runtime, source);
    const retainedSelection = await application.storage.readJson('form-selection');
    let selection = openFormSelection(inventory, retainedSelection);
    if (retainedSelection === null) {
      const scratch = inventory.forms.find(form => form.name === 'memory_lantern');
      if (scratch) selection = { selected: [scratch], refusals: [] };
    }
    await session.restore();
    let saving = Promise.resolve();
    let selected = session.foreground()?.checked_form_id;
    let playback = { state: 'Lulled', detail: 'Its Forms can wake here.' };
    let play = null;
    let library = null, editing = false;
    const bodyChanged = () => {
      saving = saving.then(() => session.save()).then(async () => {
        if (!session.current()?.here_part_id) {
          session.attachHere();
          await session.save();
        }
        await session.arrive();
        selected = session.foreground()?.checked_form_id;
        render();
        if (session.current().initial_forms.length) await play.wake();
      }).catch(fail);
    };
    const inspect = kind => {
      library?.hide();
      const body = session.current();
      const form = inventory.forms.find(form => form.checked_form_id === selected);
      const evidence = session.evidence();
      const heading = inspection.querySelector('h2');
      heading.textContent = kind === 'form' ? (form?.title ?? 'This Form') : kind === 'flow' ? 'Inside this Form' : `${body.friendly_name} · ${playback.state}`;
      details.replaceChildren();
      const text = document.createElement('p'); text.className = 'inspection-explanation';
      text.textContent = kind === 'lifecycle' ? playback.detail : kind === 'flow' ? 'The checked source describes this Form’s meaning. Its exact realization appears below when admitted.' : 'An installed Form in this Body. Opening its surface keeps the current Play.';
      details.append(text);
      const pre = document.createElement('pre');
      pre.textContent = kind === 'lifecycle' ? JSON.stringify({ body: evidence?.evidence, realization: evidence?.realization, terminal: playback.terminal, refusal: playback.refusal }, null, 2)
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
      const form = inventory.forms.find(item => item.checked_form_id === selected);
      input.hidden = !form;
      root.querySelector('#surface-title').textContent = form?.title ?? 'No Forms installed';
      root.querySelector('[data-surface-invitation]').textContent = form?.required_kinds.includes('text/submit-lines') ? 'Type a message. Press Enter to send.'
        : form?.required_kinds.includes('input/keyboard') ? 'Type something. Your Form is listening.' : form ? 'Watch this Form take shape.' : 'Your Body is retained without running Forms.';
      root.querySelector('.current-form').textContent = form?.title ?? 'Your Forms';
      root.querySelector('[data-flow-label]').textContent = session.evidence()?.foreground_flow ?? 'Not yet planned';
      input.setAttribute('aria-label', `Interact with ${form?.title ?? 'your Form'}`);
      for (const button of activities.querySelectorAll('[data-checked-form-id]')) button.setAttribute('aria-pressed', String(button.dataset.checkedFormId === selected));
      const partition = session.evidence()?.realization?.plan.forms.find(item => item.form.checked_form_id === selected);
      const visible = new Set(partition?.plan.fragments.flatMap(fragment => fragment.placements.map(placement => placement.placement_id)) ?? []);
      for (const output of root.querySelectorAll('[data-form-output] output')) output.hidden = !visible.has(output.dataset.placementId);
      for (const button of strip.querySelectorAll('[data-inspect="form"], [data-inspect="flow"]')) button.disabled = !form;
    };
    function render() {
      const body = session.current();
      const arriving = !body || !body.here_part_id;
      nursery.hidden = !arriving;
      surface.hidden = arriving || !inspection.hidden || library?.isOpen();
      activities.hidden = arriving;
      strip.hidden = arriving;
      root.querySelector('[data-body-name]').textContent = body?.friendly_name ?? 'Your body starts here';
      root.querySelector('[data-body-state]').textContent = body ? body.state.toLowerCase() : 'Crèche';
      if (arriving) {
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
      if (!body.initial_forms.length) {
        root.querySelector('#surface-guidance').textContent = 'This Body can remain lulled.';
        wakeButton.hidden = true;
        lullButton.hidden = true;
      }
      notice.textContent = `${body.initial_forms.length} Form${body.initial_forms.length === 1 ? '' : 's'} in ${body.friendly_name}.`;
      if (!body.initial_forms.some(form => form.checked_form_id === selected)) selected = body.initial_forms[0]?.checked_form_id;
      activities.replaceChildren(...body.initial_forms.map(form => {
        const button = document.createElement('button'); button.type = 'button';
        button.textContent = inventory.forms.find(item => item.checked_form_id === form.checked_form_id)?.title ?? form.name;
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
      activities.append(browse);
      if (!play) play = openWorkspacePlay({ host, session, source, foregroundForm: () => selected, inputTarget: input, outputRoot: root.querySelector('[data-form-output]'), onState(state) {
        playback = state;
        root.querySelector('[data-play-state]').textContent = state.state;
        root.querySelector('[data-body-state]').textContent = session.current().state.toLowerCase();
        root.querySelector('#surface-guidance').textContent = state.detail;
        wakeButton.disabled = Boolean(session.persistenceFailure()) || !['Lulled', 'Refused'].includes(state.state);
        lullButton.disabled = !['Playing', 'Completed', 'Failed'].includes(state.state);
        input.disabled = state.state !== 'Playing';
        input.inert = input.disabled;
        input.tabIndex = input.disabled ? -1 : 0;
        input.setAttribute('aria-disabled', String(input.disabled));
        wakeButton.hidden = state.state === 'Playing' || state.state === 'Preparing';
        showSelected();
        if (state.state === 'Playing') input.focus();
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
        if (!installed || session.current().state === 'LULLED') await play.wake();
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
    library = openWorkspaceLibrary({ panel: root.querySelector('#workspace-library'), session, source, inventory,
      presentationFor: application.presentationFor, onUse: useForm, onRemove: removeForm, onFailure: fail,
      onClose() { library.hide(); render(); root.querySelector('[data-open-library]')?.focus(); },
    });
    wakeButton.addEventListener('click', () => play?.wake().catch(fail));
    lullButton.addEventListener('click', () => play?.lull().catch(fail));
    if (session.current()?.here_part_id) await session.arrive();
    render();
    if (session.current()?.here_part_id && session.current().initial_forms.length) await play.wake();
    globalThis.addEventListener('pagehide', () => play?.close());
    globalThis.__conduitWorkspace = Object.freeze({ host, current: session.current, evidence: session.evidence, state: () => structuredClone(playback), settled: () => saving.then(session.settled) });
  } catch (error) { fail(error); }
}
