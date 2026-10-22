import { checkedPatchbayFaceArguments } from "../sdk/browser-sdk-face-actions.mjs";

// Retain no Face/Show or graph truth. Every operation rereads the admitted
// owner; current() fences the captured adapter and started Play.
export function bindWorkspacePatchbay(owner, admitted, current) {
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
}

export function checkedWorkspaceTerminal(receipt, activePlayId) {
  if (receipt?.schema !== 'conduit.tour/manifestation-receipt@3' ||
      receipt.active_play_id !== activePlayId ||
      !['completed', 'cancelled', 'failed'].includes(receipt.disposition) ||
      typeof receipt.terminal_sign_id !== 'string' || !receipt.terminal_sign_id) {
    throw new Error('The host has not supplied the terminal receipt for this play');
  }
  return receipt;
}

export async function cleanupWorkspacePlay({ external, adapter, started, session, error, requireTerminal }) {
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
  return { external, adapter, cleanupError };
}
