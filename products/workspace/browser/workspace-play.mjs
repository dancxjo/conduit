import { acquireBrowserBodyHost } from "../../../targets/browser/host/assets/browser-body-host.mjs";

export function openWorkspacePlay({ host, session, source, inputTarget, outputRoot, foregroundForm, onState }) {
  let adapter = null, started = null, terminal = null, transition = false;
  const publish = (state, detail = '') => onState({ state, detail, play: started?.play, terminal });
  const requireTerminal = receipt => {
    if (receipt?.schema !== 'conduit.tour/manifestation-receipt@3' ||
        receipt.active_play_id !== started?.play.active_play_id ||
        !['completed', 'cancelled', 'failed'].includes(receipt.disposition) ||
        typeof receipt.terminal_sign_id !== 'string' || !receipt.terminal_sign_id) {
      throw new Error('The Host has not supplied the terminal receipt for this Play');
    }
    terminal = receipt;
  };
  return Object.freeze({
    async wake() {
      if (adapter || transition) return;
      transition = true; terminal = null; started = null;
      publish('Preparing', 'Checking the installed Forms');
      try {
        const proposal = await session.propose(source);
        publish('Preparing', 'Acquiring the required capabilities');
        adapter = acquireBrowserBodyHost({ api: host.runtime, hostId: host.hostId, bootId: host.bootId, proposal, inputTarget, outputRoot, foregroundForm });
        started = adapter.start(1);
        await session.started(started);
        publish('Playing', 'Forms are awake');
        adapter.run().then(receipt => {
          if (terminal) return;
          requireTerminal(receipt);
          publish(receipt.disposition === 'completed' ? 'Completed' : receipt.disposition === 'cancelled' ? 'Cancelled' : 'Failed');
        }).catch(error => { if (!terminal) publish('Failed', error.message); });
      } catch (error) {
        const closed = adapter?.close(); adapter = null;
        if (started) {
          requireTerminal(closed?.receipt);
          await session.lull(started.play);
        } else if (session.evidence()?.realization && (!closed || closed.startOutcome === 'refused-before-play' || closed.startOutcome === 'not-attempted')) {
          await session.lull(null);
        }
        publish('Refused', error.message);
      } finally { transition = false; }
    },
    async lull() {
      if (!adapter || transition) return;
      transition = true;
      try {
        const closed = adapter.close();
        if (!terminal) requireTerminal(closed?.receipt);
        await session.lull(started.play);
        adapter = null;
        publish('Lulled', 'Your Body is retained. Its Forms can wake again.');
      } finally { transition = false; }
    },
    close() { return adapter?.close(); },
  });
}
