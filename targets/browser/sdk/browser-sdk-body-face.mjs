// The existing owner component realizes the actual admitted Mask Show.
import '../handbook/owner-face-component.mjs';

async function responseBytes(response) {
  if (!response.ok) throw new Error(`Body owner refused (${response.status}): ${await response.text()}`);
  const bytes = new Uint8Array(await response.arrayBuffer());
  if (bytes.length < 1 || bytes.length > 2 * 1024 * 1024) throw new Error('Body owner response exceeds its bound');
  return bytes;
}
export async function showBodyFace({ coordinator, root, bridge, play, current, transport = fetch }) {
  const base = new URL(coordinator);
  if (!['http:', 'https:'].includes(base.protocol) || base.username || base.password || !root?.isConnected
      || play?.state !== 'playing' || !current()) throw new Error('Current admitted Body and connected Face root required');
  const endpoint = path => new URL(`/api/${path}`, base);
  const post = async (path, bytes) => responseBytes(await transport(endpoint(path), {
    method: 'POST', headers: { 'Content-Type': 'application/json' }, body: bytes,
  }));
  const assertCurrent = () => {
    if (!current() || !root.isConnected || play.state !== 'playing') throw new Error('Body Face owner is no longer current');
  };
  const prepared = bridge.bodyFacePrepare(await responseBytes(await transport(endpoint('body-face-publication'))));
  assertCurrent();
  const component = root.ownerDocument.createElement('conduit-owner-face');
  root.replaceChildren(component);
  component.disabled = true;
  component.show(prepared.value.view);
  // Connection and a rendered layout precede the acknowledgement, as for the
  // same component's resident owner publication. Hidden pages cannot ACK.
  await new Promise(resolve => requestAnimationFrame(resolve));
  assertCurrent();
  if (root.ownerDocument.visibilityState !== 'visible' || !component.checkVisibility({ visibilityProperty: true })) {
    throw new Error('Body Face is not visibly manifested');
  }
  const effect = prepared.value.effect;
  const acknowledged = bridge.bodyFaceAcknowledge({
    show_id: effect.show_id, manifestation_id: effect.manifestation_id,
    mask_plan_id: effect.mask_plan_id, active_play_id: effect.mask_play.active_play_id,
    placement_id: effect.placement_id, face_id: effect.face_id,
    face_revision: prepared.value.view.face_revision,
  });
  await post('body-face-acknowledge', acknowledged.bytes);
  assertCurrent();
  const view = acknowledged.value.view;
  component.show(view, {
    supportsAction: action => action.intent === 'conduit.intent/lull@1',
    invoke: async ({ view: supplied, action, arguments: args }) => {
      assertCurrent(); component.disabled = true;
      if (supplied !== view || args.length) throw new Error('Body Face action basis changed');
      const returned = bridge.bodyFaceInteract({ show_id: view.show_id, face_id: view.face_id,
        face_revision: view.face_revision, action_id: action.identity, target: action.target,
        arguments: [], sequence: 1 });
      const pending = JSON.parse(new TextDecoder().decode(await post('body-face-interact', returned.bytes)));
      if (pending.disposition !== 'awaiting-terminal' || pending.play?.active_play_id !== play.id) {
        throw new Error('Body owner did not retain this cancellation intent');
      }
      // Uncertain cancellation or transport leaves the consumed Show disabled;
      // it never retries, reports success, or invents a Terminal receipt.
      const terminal = await play.terminate();
      await post('body-execution', JSON.stringify({ schema: 'conduit.body/execution-request@1', action: {
        kind: 'Terminal', play: play.identity, disposition: terminal.disposition,
        terminal_sign_id: terminal.terminal_sign_id,
      } }));
      root.dataset.bodyFaceDisposition = 'lulled';
    },
  });
  component.disabled = false;
  return Object.freeze({ view, observation: acknowledged.value, component });
}
