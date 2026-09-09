import { acquireBrowserBodyHost } from "../../../targets/browser/host/assets/browser-body-host.mjs";

export function openWorkspacePlay({ host, session, source, inputTarget, outputRoot, foregroundForm, onState }) {
  let adapter = null, started = null, terminal = null, transition = false;
  const publish = (state, detail = '', error = null) => onState({ state, detail, play: started?.play, terminal,
    refusal: error ? { code: typeof error.code === 'string' ? error.code : error.name, message: error.message } : null });
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
          if (receipt?.schema === 'conduit.browser/pending-effects@1' && receipt.disposition === 'quiescent_awaiting_input' && receipt.active_play_id === started.play.active_play_id && receipt.pending_effects === 0) {
            publish('Idle', 'Its Forms are awake. Their current work has finished.');
            return;
          }
          requireTerminal(receipt);
          publish(receipt.disposition === 'completed' ? 'Completed' : receipt.disposition === 'cancelled' ? 'Cancelled' : 'Failed');
        }).catch(error => { if (!terminal) publish('Failed', error.message, error); });
      } catch (error) {
        let cleanupError = null;
        try {
          const closed = adapter?.close();
          if (started) {
            requireTerminal(closed?.receipt);
            adapter = null;
            await session.lull(started.play);
          } else if (session.evidence()?.realization && (!closed || closed.startOutcome === 'refused-before-play' || closed.startOutcome === 'not-attempted')) {
            adapter = null;
            await session.lull(null);
          }
        } catch (failure) { cleanupError = failure; }
        if (session.persistenceFailure()) {
          publish(terminal ? 'Stopped' : 'Refused', `Your Body could not be saved. ${terminal ? 'Its Forms have stopped. ' : ''}Reopen to recover the last saved state. ${error.message}`, error);
        } else {
          publish(adapter ? 'Failed' : 'Refused', [error.message, cleanupError?.message].filter(Boolean).join(' · '), error);
        }
      } finally { transition = false; }
    },
    async lull() {
      if (!adapter || transition) return;
      transition = true;
      try {
        const closed = adapter.close();
        if (!terminal) requireTerminal(closed?.receipt);
        adapter = null;
        await session.lull(started.play);
        publish('Lulled', 'Your Body is retained. Its Forms can wake again.');
      } catch (error) {
        publish(terminal ? 'Stopped' : 'Failed', terminal
          ? `Its Forms have stopped, but your Body could not be saved. Reopen to recover the last saved state. ${error.message}`
          : error.message, error);
      } finally { transition = false; }
    },
    evidence() { return adapter?.evidence() ?? terminal?.kernel_signs ?? null; },
    close() { return adapter?.close(); },
  });
}
