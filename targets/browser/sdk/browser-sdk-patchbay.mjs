// Reuse the checked Face realization; this module only owns Show presentation.
import '../handbook/owner-face-component.mjs';

const sequences = new WeakMap();
const visibilityWaits = new WeakMap();
const supportsAction = action => [
  'patchbay/select-plot@1', 'patchbay/edit-subject@1', 'patchbay/change-masks@1',
].includes(action.intent);

export async function renderPatchbayPublication({ observation, root, signal, play }) {
  visibilityWaits.get(root)?.();
  const publication = observation?.publication;
  const current = () => play?.state === 'playing' && !signal.aborted && root.isConnected;
  if (!publication || !current()) throw new Error('Patchbay presentation is no longer current');
  let component = root.querySelector('conduit-owner-face');
  let status = root.querySelector('[data-patchbay-status]');
  if (!component || !status) {
    component = root.ownerDocument.createElement('conduit-owner-face');
    status = root.ownerDocument.createElement('p');
    status.dataset.patchbayStatus = '';
    status.setAttribute('role', 'status');
    root.replaceChildren(component, status);
  }
  const invoke = async ({ view, action, arguments: actionArguments }) => {
    component.disabled = true;
    try {
      const sequence = (sequences.get(play) ?? 0) + 1;
      sequences.set(play, sequence);
      const accepted = await play.submitPatchbayFaceAction({ view, actionId: action.identity,
        target: action.target, arguments: actionArguments, sequence });
      if (!current()) return;
      const effect = accepted.publication.effect;
      const next = play.refreshPatchbayFace({ body_plan_id: accepted.body_plan_id,
        active_play_id: accepted.active_play_id, show_id: effect.show_id,
        face_id: effect.face_id, face_revision: effect.face_revision });
      await renderPatchbayPublication({ observation: next, root, signal, play });
    } catch (error) {
      if (!current()) return;
      status.textContent = `${error.code ?? 'PatchbayRefused'}: ${error.message}`;
      // Re-read authoritative truth; never replay or automatically re-acknowledge.
      const latest = play.patchbayObservation().publication;
      if (latest) component.show(latest.view, { invoke, supportsAction });
      component.disabled = false;
    }
  };
  component.show(publication.view, { invoke, supportsAction });
  if (publication.view.show_state === 'prepared') {
    // A retained hidden resident has content, but no acknowledged presentation.
    if (!component.checkVisibility({ visibilityProperty: true })) {
      if (!current()) return;
      const Observer = root.ownerDocument.defaultView?.ResizeObserver;
      if (Observer) {
        const stop = () => {
          observer.disconnect(); signal.removeEventListener('abort', stop);
          if (visibilityWaits.get(root) === stop) visibilityWaits.delete(root);
        };
        const observer = new Observer(() => {
          if (!current()) { stop(); return; }
          if (!component.checkVisibility({ visibilityProperty: true })) return;
          stop(); // One visibility wake, not an acknowledgement retry loop.
          renderPatchbayPublication({ observation: play.patchbayObservation(), root, signal, play })
            .catch(error => { if (current()) status.textContent = `${error.code ?? 'PatchbayRefused'}: ${error.message}`; });
        });
        visibilityWaits.set(root, stop);
        signal.addEventListener('abort', stop, { once: true });
        observer.observe(component);
      }
      return;
    }
    if (!current() || !component.isConnected) throw new Error('Patchbay DOM was removed before acknowledgement');
    const effect = publication.effect;
    const shown = play.acknowledgePatchbayFace({ show_id: effect.show_id,
      manifestation_id: effect.manifestation_id, mask_plan_id: effect.mask_plan_id,
      active_play_id: effect.mask_play.active_play_id, placement_id: effect.placement_id,
      face_id: effect.face_id, face_revision: effect.face_revision });
    component.show(shown.publication.view, { invoke, supportsAction });
  }
  component.disabled = false;
}
