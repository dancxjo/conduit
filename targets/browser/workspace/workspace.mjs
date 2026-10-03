import { initializeBrowserHost } from "../../../targets/browser/host/assets/browser-host-bootstrap.mjs";
import { createBodyBirthRunner, createFirstHostRunner } from "./body-bootstrap.mjs";
import { readReviewedPlotInventory, openPlotSelection, persistedPlotSelection } from "./reviewed-plot-selection.mjs";
import { openWorkspaceSession } from "./workspace-session.mjs";
import { openWorkspacePlay } from "./workspace-play.mjs";
import { configureWorkspaceInput } from "./workspace-surface.mjs";
import { openWorkspaceLibrary } from "./workspace-library.mjs";
import { readWorkspaceHandoff, consumeWorkspaceHandoff } from "./workspace-handoff.mjs";
import { acquireBrowserBodyContinuity } from "../../../targets/browser/host/assets/browser-body-continuity.mjs";
import { createBodyInvitationReceiver, openWorkspaceMembership, readBodyInvitation, readSharedBodyInvitation } from "./workspace-membership.mjs";
import { prepareWorkspaceVoicePlay } from "./workspace-voice-play.mjs";
import { prepareWorkspaceTutorialPresenterPlay } from "./workspace-tutorial-presenter-play.mjs";
import { createMemoryReleaseCache, openReleaseCatalog } from "./creche-release-catalog.mjs";
import { browserHostCallLimits, createBrowserHostCalls } from "../../../targets/browser/host/assets/browser-host-calls.mjs";

export async function startApplication(application) {
  const root = document.querySelector('.workspace-shell');
  const nursery = root.querySelector('[data-workspace-creche]');
  const surface = root.querySelector('[data-workspace-surface]');
  const activities = root.querySelector('.activity-switcher');
  const strip = root.querySelector('.truth-strip');
  const notice = root.querySelector('[data-workspace-notice]');
  const input = root.querySelector('#plot-input');
  const inspection = root.querySelector('#workspace-inspection');
  const details = inspection.querySelector('[data-inspection-content]');
  const wakeButton = root.querySelector('[data-wake-body]');
  const lullButton = root.querySelector('[data-lull-body]');
  const fulfillButton = root.querySelector('[data-fulfill-body]');
  const tutorial = root.querySelector('[data-body-tutorial]');
  const tutorialCore = tutorial.querySelector('[data-body-tutorial-core]');
  const tutorialResident = tutorial.querySelector('[data-body-tutorial-resident]');
  const tutorialPresentation = application.presentationFor(tutorial);
  let tutorialRevision = 0;
  const fail = error => {
    notice.textContent = error instanceof Error ? error.message : String(error);
    notice.dataset.disposition = 'refused';
  };
  try {
    const invitation = await readSharedBodyInvitation(globalThis.location) ?? readBodyInvitation(globalThis.location);
    if (!invitation) await acquireBrowserBodyContinuity();
    const initializedHost = await initializeBrowserHost({ runtimeBytes: application.bytes('runtime'), durable: !invitation });
    const releaseCatalogSource = new URL('./artifacts/release-catalog.json', import.meta.url).href;
    const releaseArtifactCache = createMemoryReleaseCache();
    const host = Object.freeze({
      ...initializedHost,
      admitProfileGatedBrowserBoot: application.admitProfileGatedBrowserBoot,
      releaseCatalogSource,
      releaseArtifactCache,
      async resolveReviewedRelease(profile, signal) {
        if (profile?.target_id !== 'browser/wasm32/page') return null;
        const catalog = await openReleaseCatalog({ source: releaseCatalogSource, signal, cache: releaseArtifactCache });
        try {
          return await catalog.resolve(profile, signal);
        } catch (error) {
          if (error?.code === 'UnknownTarget') return null;
          throw error;
        }
      },
    });
    const calls = createBrowserHostCalls({ hostId: host.hostId, bootId: host.bootId,
      applicationId: application.manifest.applicationId, applicationGeneration: 1, authorityGeneration: 1 });
    let artifactSequence = 0;
    const hostCalls = Object.freeze({
      handoffArtifact(artifact) {
        artifactSequence += 1;
        return calls.handoffArtifact({
          contract: browserHostCallLimits.contract,
          kind: 'artifact-handoff',
          callId: `workspace/artifact-${artifactSequence}`,
          hostId: host.hostId,
          bootId: host.bootId,
          applicationId: application.manifest.applicationId,
          applicationGeneration: 1,
          authorityGeneration: 1,
          userActivation: true,
          artifactId: artifact.artifact_id,
          bytes: artifact.payload,
          maximumBytes: artifact.maximum_bytes,
          filename: artifact.filename,
          mediaType: artifact.media_type,
        });
      },
    });
    const session = openWorkspaceSession({ host, storage: application.storage });
    const source = application.text('reviewed-plot-inventory');
    const inventory = readReviewedPlotInventory(host.runtime, source);
    const catalogSource = application.text('reviewed-plot-catalog');
    const catalog = readWorkspaceCatalog(catalogSource);
    const tutorialPlot = catalog.plots.find(plot => plot.entry === 'tour');
    if (!tutorialPlot) throw new Error('The reviewed resident Tutorial Plot is absent');
    let handoff = null, handoffFailure = null;
    try { handoff = readWorkspaceHandoff(globalThis.location, inventory); }
    catch (error) { handoffFailure = error; }
    const retainedSelection = await application.storage.readJson('plot-selection');
    let selection = openPlotSelection(inventory, retainedSelection);
    if (retainedSelection === null) {
      const scratch = inventory.plots.find(plot => plot.name === 'memory_lantern');
      const chime = typeof window.AudioContext === 'function' ? inventory.plots.find(plot => plot.name === 'startup_chime') : null;
      const residentTutorial = inventory.plots.find(plot => plot.name === 'tour');
      selection = { selected: [scratch, residentTutorial, chime].filter(Boolean), refusals: [] };
    }
    const restored = invitation ? null : await session.restore();
    if (handoff && !session.current()) {
      selection = openPlotSelection(inventory, persistedPlotSelection(inventory, selection.selected), handoff);
      await application.storage.writeJson('plot-selection', persistedPlotSelection(inventory, selection.selected));
      await consumeWorkspaceHandoff({ host, applicationId: application.manifest.applicationId });
    }
    let saving = Promise.resolve();
    let selected = session.foreground()?.checked_plot_id;
    let playback = { state: 'Lulled', detail: 'Its plots can wake here.' };
    let play = null, currentMask = null;
    let library = null, membership = null, editing = false;
    const tutorialInstalled = () => session.current()?.initial_plots.some(plot => plot.checked_plot_id === tutorialPlot.checked_plot_id) ?? false;
    const handleTutorialEvent = (event, resident = false) => {
      if ((!resident && event.revision !== tutorialRevision) || event.kind !== 1 || event.value.length !== 0) {
        fail(new Error('This tutorial action is stale'));
      } else if (event.action === 'body.wake') play?.wake(true).catch(fail);
      else if (event.action === 'body.inspect-lifecycle') inspect('lifecycle');
      else if (event.action === 'body.open-library') { surface.hidden = true; inspection.hidden = true; library?.show(); }
      else if (event.action === 'body.invite-host') membership?.show();
      else if (event.action === 'body.use-current') { surface.hidden = false; inspection.hidden = true; library?.hide(); if (!input.disabled) input.focus(); }
      else fail(new Error('Unknown tutorial action'));
    };
    const submitMaskInteraction = event => {
      const mask = currentMask;
      const action = mask?.actions.find(candidate => candidate.identity === event.action);
      if (!action) throw new Error('The Mask did not expose this action');
      if (action.arguments.length !== 1) throw new Error('The tutorial action does not have one exact argument');
      const argument = action.arguments[0];
      return session.interactWithTutorialMask({
        show_id: mask.show_id,
        presentation_id: mask.presentation_id,
        presentation_revision: mask.presentation_revision,
        action_id: event.action,
        target: action.target,
        arguments: [{
          name: argument.name,
          value_kind: argument.contract.value_kind,
          value: Array.from(event.value ?? []),
        }],
        sequence: Number(event.sequence ?? 1),
      });
    };
    const renderTutorial = () => {
      if (!session.current() || !tutorialInstalled()) {
        tutorial.hidden = true;
        return;
      }
      tutorial.hidden = false;
      const resident = ['Playing', 'Idle', 'Completed', 'Failed'].includes(playback.state);
      tutorialCore.hidden = resident;
      tutorialResident.hidden = !resident;
      const revision = ++tutorialRevision;
      // A freshly born or explicitly lulled Body has no current Wake/Body
      // Plan, so it truthfully has no current Mask Show. The semantic
      // lifecycle surface remains usable to request Wake; once a realization
      // exists, every resident presentation and interaction crosses the exact
      // sealed Mask route below.
      const mask = session.current()?.plan_id
        ? session.presentTutorialMask(revision, playback.state)
        : null;
      currentMask = mask;
      const applyMask = () => {
        if (!mask) return null;
        tutorial.dataset.maskShowId = mask.show_id;
        tutorial.dataset.maskManifestationId = mask.manifestation_id;
        tutorial.dataset.maskPlanId = mask.mask_plan_id;
        tutorial.dataset.maskPlayId = mask.mask_play.active_play_id;
        tutorial.dataset.maskPlacementId = mask.placement_id;
        tutorial.dataset.presentationId = mask.presentation_id;
        tutorial.dataset.presentationRevision = String(mask.presentation_revision);
        return session.acknowledgeTutorialMask({
          show_id: mask.show_id,
          manifestation_id: mask.manifestation_id,
          mask_plan_id: mask.mask_plan_id,
          active_play_id: mask.mask_play.active_play_id,
          placement_id: mask.placement_id,
          presentation_id: mask.presentation_id,
          presentation_revision: mask.presentation_revision,
        });
      };
      if (resident) { applyMask(); return; }
      tutorialPresentation.present('body-tutorial', session.tutorialView(revision, playback.state), { onEvent(event) {
        tutorialPresentation.nextEvent('body-tutorial');
        if (mask) {
          try {
            submitMaskInteraction(event);
          } catch (error) {
            fail(error);
            return;
          }
        }
        handleTutorialEvent(event);
      } });
      applyMask();
    };
    const bodyChanged = () => {
      saving = saving.then(() => session.save()).then(async () => {
        if (!session.current()?.here_part_id) {
          session.attachHere();
          await session.save();
        }
        await session.arrive();
        const resident = session.current().initial_plots;
        if (handoff && resident.some(plot => plot.checked_plot_id === handoff.checked_plot_id)) {
          await session.selectPlot({ source_document_id: handoff.source_document_id, checked_plot_id: handoff.checked_plot_id });
        }
        const foreground = catalog.plots.find(plot => plot.checked_plot_id === session.foreground()?.checked_plot_id);
        if (foreground?.required_kinds.includes('sound/startup-chime') || foreground?.checked_plot_id === tutorialPlot.checked_plot_id) {
          const visible = resident.find(plot => catalog.plots.some(candidate => candidate.checked_plot_id === plot.checked_plot_id
            && candidate.checked_plot_id !== tutorialPlot.checked_plot_id
            && candidate.required_kinds.some(kind => kind.startsWith('presentation/'))));
          if (visible) await session.selectPlot(visible);
        }
        selected = session.foreground()?.checked_plot_id;
        render();
      }).catch(fail);
    };
    const inspect = kind => {
      library?.hide();
      membership && (membership.isOpen() ? membership.close() : null);
      const body = session.current();
      const plot = catalog.plots.find(plot => plot.checked_plot_id === selected);
      const evidence = session.evidence();
      const heading = inspection.querySelector('h2');
      heading.textContent = kind === 'plot' ? (plot?.title ?? 'This plot') : kind === 'flow' ? 'Inside this plot' : `${body.friendly_name} · ${playback.state}`;
      details.replaceChildren();
      const text = document.createElement('p'); text.className = 'inspection-explanation';
      text.textContent = kind === 'lifecycle' ? playback.detail : kind === 'flow' ? 'The checked source describes this plot’s meaning. Its exact realization appears below when admitted.' : 'An installed plot in this body. Opening its surface keeps the current play.';
      details.append(text);
      const pre = document.createElement('pre');
      pre.textContent = kind === 'lifecycle' ? JSON.stringify({ body: evidence?.evidence, realization: evidence?.realization, active_observation: play?.evidence(), terminal: playback.terminal, refusal: playback.refusal }, null, 2)
        : kind === 'flow' && evidence?.realization ? JSON.stringify(evidence.realization.plan.plots.find(item => item.plot.checked_plot_id === selected), null, 2) : (plot?.source ?? 'Source unavailable');
      const disclosure = document.createElement('details'), summary = document.createElement('summary');
      summary.textContent = kind === 'lifecycle' ? 'Exact lifecycle evidence' : kind === 'flow' && evidence?.realization ? 'Exact plan' : 'Checked source';
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
      const plot = catalog.plots.find(item => item.checked_plot_id === selected);
      const partition = session.evidence()?.realization?.plan.plots.find(item => item.plot.checked_plot_id === selected);
      root.querySelector('#surface-title').textContent = plot?.title ?? 'No Plots installed';
      root.querySelector('[data-surface-invitation]').textContent = configureWorkspaceInput(input, plot, partition, playback.detail);
      root.querySelector('.current-plot').textContent = plot?.title ?? 'Your plots';
      root.querySelector('[data-flow-label]').textContent = session.evidence()?.foreground_flow ?? 'Not yet planned';
      for (const button of activities.querySelectorAll('[data-checked-plot-id]')) button.setAttribute('aria-pressed', String(button.dataset.checkedPlotId === selected));
      const visible = new Set(partition?.plan.fragments.flatMap(fragment => fragment.placements.map(placement => placement.placement_id)) ?? []);
      for (const output of root.querySelectorAll('[data-plot-output] > output')) output.hidden = !visible.has(output.dataset.placementId);
      for (const button of strip.querySelectorAll('[data-inspect="plot"], [data-inspect="flow"]')) button.disabled = !plot;
    };
    function render() {
      const body = session.current();
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
        document.title = 'Birth your body · Conduit';
        const slot = nursery.querySelector('[data-creche-content]');
        if (body) {
          slot.replaceChildren(createFirstHostRunner({ host, presentationFor: application.presentationFor, nextSequence: session.nextMembershipSequence, onBodyChanged: bodyChanged }));
        } else {
          const birth = createBodyBirthRunner({
            source, sourceKey: 'workspace-creche', listingId: 'workspace-plots', host,
            presentationFor: application.presentationFor, inventory, initialSelection: selection,
            nextSequence: session.nextSequence, onBodyChanged: bodyChanged,
            onSelection(selected) {
              selection = selected === null ? openPlotSelection(inventory) : { selected, refusals: [] };
              saving = saving.then(() => selected === null
                ? application.storage.deleteJson('plot-selection')
                : application.storage.writeJson('plot-selection', persistedPlotSelection(inventory, selected))).catch(fail);
            },
          });
          const receiver = createBodyInvitationReceiver({ location: globalThis.location, onReceive({ fragment }) {
            history.replaceState(null, '', `${location.pathname}${location.search}#${fragment}`);
            location.reload();
          } });
          slot.replaceChildren(birth, receiver);
        }
        return;
      }
      document.title = `${body.friendly_name} · Conduit`;
      root.dataset.bodyId = body.body_id;
      root.querySelector('[data-play-state]').textContent = playback.state;
      root.querySelector('#surface-guidance').textContent = playback.detail;
      lullButton.hidden = !body.initial_plots.length;
      fulfillButton.hidden = body.state === 'FULFILLED';
      fulfillButton.disabled = body.state !== 'LULLED' || Boolean(session.persistenceFailure());
      if (body.state === 'FULFILLED') {
        wakeButton.hidden = true;
        lullButton.hidden = true;
      }
      if (!body.initial_plots.length) {
        root.querySelector('#surface-guidance').textContent = 'This body can remain lulled.';
        wakeButton.hidden = true;
        lullButton.hidden = true;
      }
      notice.textContent = `${body.initial_plots.length} Plot${body.initial_plots.length === 1 ? '' : 's'} in ${body.friendly_name}.`;
      if (!body.initial_plots.some(plot => plot.checked_plot_id === selected) || selected === tutorialPlot.checked_plot_id) {
        selected = body.initial_plots.find(plot => plot.checked_plot_id !== tutorialPlot.checked_plot_id)?.checked_plot_id
          ?? body.initial_plots[0]?.checked_plot_id;
      }
      activities.replaceChildren(...body.initial_plots.map(plot => {
        const button = document.createElement('button'); button.type = 'button';
        button.textContent = catalog.plots.find(item => item.checked_plot_id === plot.checked_plot_id)?.title ?? plot.name;
        button.dataset.checkedPlotId = plot.checked_plot_id;
        button.addEventListener('click', () => {
          if (editing) return;
          selected = plot.checked_plot_id;
          session.selectPlot({ source_document_id: plot.source_document_id, checked_plot_id: plot.checked_plot_id }).catch(fail);
          library?.hide(); inspection.hidden = true; surface.hidden = false;
          showSelected();
          if (!input.disabled) input.focus();
        });
        return button;
      }));
      const browse = document.createElement('button'); browse.type = 'button';
      browse.textContent = '+ Plots'; browse.dataset.openLibrary = '';
      browse.addEventListener('click', () => {
        if (editing) return;
        surface.hidden = true; inspection.hidden = true; library.show();
      });
      if (body.state !== 'FULFILLED') activities.append(browse);
      if (!play) play = openWorkspacePlay({ host, session, source, planningLines: () => membership?.planningLines() ?? [], foregroundPlot: () => selected, inputTarget: input, outputRoot: root.querySelector('[data-plot-output]'),
        presentationRootFor: ({ checkedPlotId }) => checkedPlotId === tutorialPlot.checked_plot_id ? tutorialResident : null,
        onApplicationEvent: ({ checkedPlotId, event }) => {
          if (checkedPlotId === tutorialPlot.checked_plot_id) {
            try { submitMaskInteraction(event); }
            catch (error) { fail(error); return; }
            handleTutorialEvent(event, true);
          }
        },
        onTutorialPresenterRequest: effect => session.tutorialPresenterRequest(
          `tutorial-presenter/${effect.active_play_id}/${effect.request_sequence}`,
          ++tutorialRevision,
          playback.state,
        ),
        async prepareExternal(proposal) {
          const distributed = proposal.plan.plots.filter(plot => plot.plan.fragments.length > 1);
          if (!distributed.length) return null;
          if (distributed.length !== 1) throw new Error('This body Plan exceeds the one external Plot bound');
          const plan = distributed[0].plan;
          const peer = plan.fragments.find(fragment => fragment.host_id !== host.hostId || fragment.boot_id !== host.bootId);
          const joined = peer && membership?.executionLine(peer.host_id, peer.boot_id);
          if (!joined) throw new Error('The planned remote Host Line is no longer current');
          const presenter = plan.fragments.some(fragment => fragment.placements.some(placement =>
            placement.kind_id === 'llm/present'));
          if (presenter) {
            const tutorialPresenter = await prepareWorkspaceTutorialPresenterPlay({ api: host.runtime,
              localAdvertisement: host.membership.advertisement(), joined, plan, outputRoot: tutorialResident,
              request: identity => session.tutorialPresenterRequest(identity, ++tutorialRevision, playback.state) });
            return Object.freeze({ planId: plan.plan_id, identity: tutorialPresenter.identity,
              run: () => tutorialPresenter.run(), close: () => tutorialPresenter.close() });
          }
          await joined.line.installBodyContext(session.conversationContext());
          const voice = await prepareWorkspaceVoicePlay({ api: host.runtime,
            localAdvertisement: host.membership.advertisement(), joined, plan,
            outputRoot: root.querySelector('[data-plot-output]') });
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
        wakeButton.hidden = session.current().initial_plots.length === 0 || ['Playing', 'Idle', 'Preparing', 'Fulfilled'].includes(state.state);
        if (state.state === 'Fulfilled') lullButton.hidden = true;
        showSelected();
        if (state.state === 'Playing' && input.dataset.acceptsInput === 'true') input.focus();
      } });
      showSelected();
    }
    const usePlot = async (plot, expectedRevision) => {
      if (editing) throw new Error('A body transition is already in progress');
      editing = true;
      try {
        const identity = { source_document_id: plot.source_document_id, checked_plot_id: plot.checked_plot_id };
        const installed = session.current().initial_plots.some(item => item.source_document_id === identity.source_document_id && item.checked_plot_id === identity.checked_plot_id);
        if (installed) await session.selectPlot(identity);
        else await play.changeWorkset('Install', identity, expectedRevision);
        selected = session.foreground()?.checked_plot_id;
        library.hide(); inspection.hidden = true;
        render();
        if (!installed || session.current().state === 'LULLED') await play.wake(true);
        if (!input.disabled && !input.hidden) input.focus();
      } finally { editing = false; }
    };
    const removePlot = async (plot, expectedRevision) => {
      if (editing) throw new Error('A body transition is already in progress');
      editing = true;
      try {
        await play.changeWorkset('Remove', { source_document_id: plot.source_document_id, checked_plot_id: plot.checked_plot_id }, expectedRevision);
        selected = session.foreground()?.checked_plot_id;
        render();
        if (session.current().initial_plots.length) await play.wake();
      } finally { editing = false; }
    };
    library = openWorkspaceLibrary({ panel: root.querySelector('#workspace-library'), session, source: catalogSource, inventory: catalog,
      planningLines: () => membership?.planningLines() ?? [],
      presentationFor: application.presentationFor, onUse: usePlot, onRemove: removePlot, onFailure: fail,
      onClose() { library.hide(); render(); root.querySelector('[data-open-library]')?.focus(); },
    });
    globalThis.__conduitWorkspace = Object.freeze({ host, presentationFor: application.presentationFor, current: session.current, evidence: session.evidence,
      maskObservation: session.tutorialMaskObservation,
      maskJourney() {
        session.beginTutorialMaskJourney();
        const realize = prepare => {
          const mask = prepare();
          tutorial.dataset.maskShowId = mask.show_id;
          tutorial.dataset.maskManifestationId = mask.manifestation_id;
          tutorial.dataset.maskPlanId = mask.mask_plan_id;
          tutorial.dataset.maskPlayId = mask.mask_play.active_play_id;
          tutorial.dataset.maskPlacementId = mask.placement_id;
          tutorial.dataset.presentationId = mask.presentation_id;
          tutorial.dataset.presentationRevision = String(mask.presentation_revision);
          session.acknowledgeTutorialMask({ show_id: mask.show_id, manifestation_id: mask.manifestation_id,
            mask_plan_id: mask.mask_plan_id, active_play_id: mask.mask_play.active_play_id,
            placement_id: mask.placement_id, presentation_id: mask.presentation_id,
            presentation_revision: mask.presentation_revision });
        };
        realize(session.prepareTutorialMaskAlternate);
        realize(session.prepareTutorialMaskReplacement);
        realize(session.prepareTutorialMaskRestored);
        return session.tutorialMaskJourney();
      },
      state: () => structuredClone(playback), settled: () => saving.then(session.settled) });
    membership = openWorkspaceMembership({ root, session, host, hostCalls, invitation, presentationFor: application.presentationFor,
      invitationLabel: () => catalog.plots.find(plot => plot.checked_plot_id === selected)?.name === 'firefly-choir'
        ? 'Invite another phone' : 'Invite another host',
      async beforeAdmission() {
        if (['Playing', 'Idle', 'Completed', 'Failed'].includes(playback.state)) await play?.lull();
      },
      onChanged: render,
      onFailure: fail,
    });
    wakeButton.addEventListener('click', () => play?.wake(true).catch(fail));
    lullButton.addEventListener('click', () => play?.lull().catch(fail));
    fulfillButton.addEventListener('click', () => {
      if (!globalThis.confirm('Finish this body permanently? Its biography remains available, but it cannot wake or change again.')) return;
      play?.fulfill().then(render).catch(fail);
    });
    if (session.current()?.here_part_id && session.current().state !== 'FULFILLED') await session.arrive();
    render();
    if (handoff && session.current()?.here_part_id) {
      await consumeWorkspaceHandoff({ host, applicationId: application.manifest.applicationId });
      await usePlot(handoff, session.current().workload_revision);
    } else if (restored?.resume_wake && session.current()?.here_part_id && session.current().initial_plots.length) await play.wake();
    if (handoffFailure) fail(handoffFailure);
    globalThis.addEventListener('pagehide', () => { play?.close(); membership?.dispose(); });
  } catch (error) { fail(error); }
}

function readWorkspaceCatalog(source) {
  const catalog = JSON.parse(source);
  if (catalog?.schema !== 'conduit.workspace/reviewed-plot-catalog@3'
      || !Number.isSafeInteger(catalog.maximum_plots) || catalog.maximum_plots < 1
      || !Array.isArray(catalog.plots) || catalog.plots.length > catalog.maximum_plots) {
    throw new Error('reviewed Workspace Plot catalog is malformed or over capacity');
  }
  const identities = new Set();
  for (const plot of catalog.plots) {
    if (typeof plot?.title !== 'string' || typeof plot.entry !== 'string'
        || typeof plot.source !== 'string' || typeof plot.source_document_id !== 'string'
        || typeof plot.checked_plot_id !== 'string' || !Array.isArray(plot.required_kinds)
        || !Number.isSafeInteger(plot.presentation_profile)
        || plot.presentation_profile < 0 || plot.presentation_profile > 3
        || typeof plot.unavailable_hint !== 'string' || plot.unavailable_hint.length < 1
        || identities.has(plot.checked_plot_id)) {
      throw new Error('reviewed Workspace Plot catalog contains an invalid or duplicate entry');
    }
    plot.name = plot.entry;
    identities.add(plot.checked_plot_id);
  }
  return catalog;
}
