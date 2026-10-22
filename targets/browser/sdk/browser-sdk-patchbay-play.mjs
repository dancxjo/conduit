// Checked Patchbay operations over one actually started adapter. Face/Show truth
// is always reread from the runtime; this facade retains only the Play identity.
import { checkedPatchbayFaceArguments } from "./browser-sdk-face-actions.mjs";
const encoder = new TextEncoder();

export function createPatchbayPlay({ adapter, identity, isCurrent }) {
  if (!adapter || typeof isCurrent !== 'function'
      || !['body_id', 'plan_id', 'active_play_id'].every(key => typeof identity?.[key] === 'string' && identity[key])) {
    throw new TypeError('Patchbay presentation requires an actual started Play and lifecycle guard');
  }
  const play = Object.freeze({ ...identity });
  const requireCurrent = () => {
    if (isCurrent() !== true) throw new Error('Patchbay Play is unavailable');
  };
  const checkedObservation = observation => {
    if (observation?.schema !== 'conduit.browser/patchbay-observation@1'
        || observation.mask_show !== null || observation.interaction_admitted !== false
        || observation.active_play_id !== play.active_play_id
        || observation.body_plan_id !== play.plan_id
        || observation.face?.basis?.body_id !== play.body_id) {
      throw new Error('Patchbay observation differs from this Body Play');
    }
    return observation;
  };
  const facade = Object.freeze({
    get state() { return isCurrent() === true ? 'playing' : 'terminal'; },
    patchbayObservation() {
      requireCurrent();
      return checkedObservation(adapter.patchbayObservation());
    },
    acknowledgePatchbayFace(message) {
      requireCurrent();
      return checkedObservation(adapter.acknowledgePatchbayFace(message));
    },
    async submitPatchbayFaceAction({ view, actionId, target, arguments: actionArguments = [], sequence }) {
      requireCurrent();
      const current = facade.patchbayObservation().publication?.view;
      if (!current || current.interactions_admitted !== true || current.show_state !== 'available'
          || current.body_id !== play.body_id
          || view?.show_id !== current.show_id || view?.face_id !== current.face_id
          || view?.face_revision !== current.face_revision) throw new Error('Current acknowledged Patchbay Show is required');
      if (!Number.isSafeInteger(sequence) || sequence < 1) throw new RangeError('Face action sequence is invalid');
      const action = current.actions.find(candidate => candidate.identity === actionId && candidate.target === target);
      if (!action || action.availability !== 'available') throw new Error('Patchbay action is unavailable');
      const checked = checkedPatchbayFaceArguments(current, view, actionId, target, actionArguments);
      return facade.submitPatchbayInteraction({ show_id: current.show_id, face_id: current.face_id,
        face_revision: Number(current.face_revision), action_id: actionId, target, arguments: checked, sequence });
    },
    async submitPatchbayInteraction(message) {
      requireCurrent();
      return checkedObservation(await adapter.submitPatchbayInteraction(message));
    },
    refreshPatchbayFace(message) {
      requireCurrent();
      return checkedObservation(adapter.refreshPatchbayFace(message));
    },
    reportPatchbayRequestDisposition({ observation, outcome, evidenceId }) {
      const current = facade.patchbayObservation();
      if (!['Completed', 'Rejected', 'Cancelled', 'Uncertain'].includes(outcome)
          || typeof evidenceId !== 'string' || evidenceId.trim().length < 1
          || encoder.encode(evidenceId).length > 256) throw new TypeError('Invalid Patchbay request disposition');
      if (!current.pending_owner_request || !observation?.pending_owner_request
          || observation.body_plan_id !== current.body_plan_id
          || observation.active_play_id !== current.active_play_id
          || observation.owner_checked_plot_id !== current.owner_checked_plot_id
          || observation.pending_owner_request.application_revision !== current.pending_owner_request.application_revision
          || JSON.stringify(observation.pending_owner_request.request) !== JSON.stringify(current.pending_owner_request.request)) {
        throw new Error('Current exact pending Patchbay owner request is required');
      }
      return checkedObservation(adapter.patchbayRequestDisposition({
        body_id: play.body_id, body_plan_id: current.body_plan_id,
        active_play_id: current.active_play_id, owner_checked_plot_id: current.owner_checked_plot_id,
        application_revision: current.pending_owner_request.application_revision, outcome, evidence_id: evidenceId,
      }));
    },
  });
  return facade;
}
