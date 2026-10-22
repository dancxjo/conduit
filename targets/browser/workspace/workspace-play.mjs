import { acquireBrowserBodyHost } from "../../../targets/browser/host/assets/browser-body-host.mjs";
import { renderPatchbayPublication } from "../sdk/browser-sdk-patchbay.mjs";
import { checkedPatchbayFaceArguments } from "../sdk/browser-sdk-face-actions.mjs";

export function openWorkspacePlay({ host, session, storage, source, planningLines, inputTarget, outputRoot, foregroundPlot, onState,
  faceRootFor, onApplicationEvent, onPatchbayOwnerRequest,
  onPatchbayPublication = renderPatchbayPublication,
  onTutorialPresenterRequest,
  prepareExternal = async () => null, acquireBody = acquireBrowserBodyHost }) {
  let adapter = null, external = null, proposal = null, started = null, terminal = null, transition = false;
  let playbackState = null, rendererRetired = false;
  const publish = (state, detail = '', error = null) => {
    playbackState = state;
    onState({ state, detail, play: started?.play, terminal,
      proposal, refusal: error ? { code: typeof error.code === 'string' ? error.code : error.name, message: error.message } : null });
  };
  // This adapter retains no Face/Show or graph truth. Every operation rereads
  // the admitted owner; the captured adapter/start only fence an old Play.
  const bindPatchbay = (owner, admitted) => {
    const current = () => adapter === owner && started === admitted && !terminal && !rendererRetired
      && ['Playing', 'Idle'].includes(playbackState);
    const requireCurrent = () => {
      if (!current()) throw new Error('Patchbay Play is unavailable');
    };
    const checked = observation => {
      requireCurrent();
      if (observation?.schema !== 'conduit.browser/patchbay-observation@1'
        || observation.mask_show !== null || observation.interaction_admitted !== false
        || observation.active_play_id !== admitted.play.active_play_id
        || observation.body_plan_id !== admitted.play.plan_id
        || observation.face?.basis?.body_id !== admitted.play.body_id
        || observation.publication?.view?.body_id !== admitted.play.body_id) {
        throw new Error('Patchbay observation differs from this Body Play');
      }
      return observation;
    };
    const observation = () => { requireCurrent(); return checked(owner.patchbayObservation()); };
    return Object.freeze({
      get state() { return current() ? 'playing' : 'retired'; },
      patchbayObservation: observation,
      acknowledgePatchbayFace(message) {
        requireCurrent();
        return checked(owner.acknowledgePatchbayFace(message));
      },
      refreshPatchbayFace(message) {
        requireCurrent();
        return checked(owner.refreshPatchbayFace(message));
      },
      async submitPatchbayFaceAction({ view, actionId, target, arguments: arguments_ = [], sequence }) {
        const available = observation().publication?.view;
        if (!Number.isSafeInteger(sequence) || sequence < 1) throw new RangeError('Face action sequence is invalid');
        const argumentsChecked = checkedPatchbayFaceArguments(available, view, actionId, target, arguments_);
        const revision = Number(available.face_revision);
        if (!Number.isSafeInteger(revision) || String(revision) !== String(available.face_revision)) {
          throw new Error('Patchbay Face revision cannot be represented exactly');
        }
        return checked(await owner.submitPatchbayInteraction({ show_id: available.show_id,
          face_id: available.face_id, face_revision: revision, action_id: actionId, target,
          arguments: argumentsChecked, sequence }));
      },
    });
  };
  const requireTerminal = receipt => {
    if (receipt?.schema !== 'conduit.tour/manifestation-receipt@3' ||
        receipt.active_play_id !== started?.play.active_play_id ||
        !['completed', 'cancelled', 'failed'].includes(receipt.disposition) ||
        typeof receipt.terminal_sign_id !== 'string' || !receipt.terminal_sign_id) {
      throw new Error('The host has not supplied the terminal receipt for this play');
    }
    terminal = receipt;
  };
  const stop = async () => {
    await external?.close();
    external = null;
    const closed = adapter.close();
    if (!terminal) requireTerminal(closed?.receipt);
    adapter = null;
    await session.lull(started.play);
  };
  if (session.current()?.state === 'FULFILLED') {
    publish('Fulfilled', 'This body is complete. Its biography remains available for inspection.');
  } else {
    publish('Lulled', 'This body is retained. Wake it to start its plots.');
  }
  return Object.freeze({
    async wake(authorizeAudio = false) {
      if (adapter || transition) return;
      transition = true; terminal = null; started = null; rendererRetired = false;
      publish('Preparing', 'Checking the installed plots');
      try {
        if (typeof onPatchbayPublication !== 'function') throw new Error('canonical Patchbay requires a selected browser Face renderer');
        proposal = await session.propose(source, planningLines(), authorizeAudio);
        publish('Preparing', 'Acquiring the required capabilities');
        external = await prepareExternal(proposal);
        let patchbay = null;
        adapter = acquireBody({ api: host.runtime, hostId: host.hostId, bootId: host.bootId, proposal,
          inputTarget, outputRoot, foregroundPlot, faceRootFor, onApplicationEvent, onPatchbayOwnerRequest,
          onPatchbayPublication: request => {
            if (!patchbay || patchbay.state !== 'playing') throw new Error('Patchbay Play is unavailable');
            return onPatchbayPublication({ ...request, play: patchbay });
          },
          onTutorialPresenterRequest, storage,
          externallyManagedPlanIds: external ? [external.planId] : [] });
        started = adapter.start(1);
        patchbay = bindPatchbay(adapter, started);
        await session.started(started);
        await external?.updateContext?.();
        publish('Playing', 'Plots are awake');
        const running = adapter;
        const runningPlay = started.play;
        const localRun = adapter.run();
        const externalRun = external?.run();
        Promise.all([localRun, externalRun]).then(([receipt, externalReceipt]) => {
          if (adapter !== running || terminal) return;
          if (external && (externalReceipt?.disposition !== 'completed' ||
              externalReceipt.active_play_id !== external.identity.active_play_id)) {
            throw new Error('The external Plot did not supply its exact terminal outcome');
          }
          if (receipt?.schema === 'conduit.browser/pending-effects@1' && receipt.disposition === 'quiescent_awaiting_input' && receipt.active_play_id === started.play.active_play_id && receipt.pending_effects === 0) {
            publish('Idle', 'Its plots are awake. Their current work has finished.');
            return;
          }
          requireTerminal(receipt);
          publish(receipt.disposition === 'completed' ? 'Completed' : receipt.disposition === 'cancelled' ? 'Cancelled' : 'Failed');
        }).catch(async error => {
          if (adapter !== running || terminal) return;
          if (error?.code !== 'FocusLost') {
            publish('Failed', error.message, error);
            return;
          }
          try {
            const closed = running.close();
            requireTerminal(closed?.receipt);
            adapter = null;
            await session.lull(runningPlay);
            publish('Lulled', 'Its input paused when this browser window lost focus. Choose Wake Body to continue.');
          } catch (cleanupError) {
            publish('Failed', `${error.message} · ${cleanupError.message}`, cleanupError);
          }
        });
      } catch (error) {
        let cleanupError = null;
        try {
          await external?.close();
          external = null;
          const closed = adapter?.close();
          if (started) {
            requireTerminal(closed?.receipt);
            adapter = null;
            await session.lull(started.play);
          } else if (session.evidence()?.realization && error?.refusal?.rejections?.length) {
            adapter = null;
            await session.failed(error.refusal.rejections);
          } else if (session.evidence()?.realization && (!closed || closed.startOutcome === 'refused-before-play' || closed.startOutcome === 'not-attempted')) {
            adapter = null;
            await session.lull(null);
          }
        } catch (failure) { cleanupError = failure; }
        if (session.persistenceFailure()) {
          publish(terminal ? 'Stopped' : 'Refused', `Your body could not be saved. ${terminal ? 'Its plots have stopped. ' : ''}Reopen to recover the last saved state. ${error.message}`, error);
        } else {
          publish(adapter ? 'Failed' : 'Refused', [error.message, cleanupError?.message].filter(Boolean).join(' · '), error);
        }
      } finally { transition = false; }
    },
    async lull() {
      if (!adapter || transition) return;
      transition = true;
      try {
        await stop();
        publish('Lulled', 'Your body is retained. Its plots can wake again.');
      } catch (error) {
        publish(terminal ? 'Stopped' : 'Failed', terminal
          ? `Its plots have stopped, but your body could not be saved. Reopen to recover the last saved state. ${error.message}`
          : error.message, error);
      } finally { transition = false; }
    },
    async fulfill() {
      if (transition) return;
      transition = true;
      try {
        if (adapter) await stop();
        if (session.current().state !== 'LULLED') throw new Error('The current play has not been retired');
        await session.fulfill();
        publish('Fulfilled', 'This body is complete. Its biography remains available for inspection.');
      } catch (error) {
        publish(session.persistenceFailure() ? 'Stopped' : 'Refused', error.message, error);
        throw error;
      } finally { transition = false; }
    },
    async changeWorkset(edit, plot, expectedRevision) {
      if (transition) throw new Error('A body transition is already in progress');
      if (session.persistenceFailure()) throw session.persistenceFailure();
      transition = true;
      try {
        // The view's revision is checked again at the authoritative mutation.
        if (session.current().workload_revision !== expectedRevision) throw new Error('The installed plots changed. Reopen the chooser.');
        if (adapter) await stop();
        if (session.current().state !== 'LULLED') throw new Error('The current play has not been retired');
        await session.changeWorkset(edit, plot, source, expectedRevision);
        publish('Lulled', 'Your installed plots have been saved.');
      } catch (error) {
        publish(session.persistenceFailure() ? 'Stopped' : session.current().state === 'LULLED' ? 'Refused' : 'Failed', error.message, error);
        throw error;
      } finally { transition = false; }
    },
    evidence() { return adapter?.evidence() ?? terminal?.kernel_signs ?? null; },
    close() { rendererRetired = true; void external?.close(); external = null; return adapter?.close(); },
  });
}
