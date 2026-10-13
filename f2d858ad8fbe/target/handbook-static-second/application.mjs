export async function startApplication(application) {
  const root = document.querySelector('[aria-label="Clock output"]');
  const status = document.querySelector('[role="status"]');
  const lull = document.querySelector('[data-lull]');
  const wake = document.querySelector('[data-wake]');
  try {
    const host = await application.browser({ root });
    const specification = JSON.parse(application.text('birth-specification'));
    const checked = await host.plot(application.text('clock-source')).check();
    if (!checked.ok) throw new Error(checked.refusal.message);
    const retained = await host.recover();
    const body = retained ?? await host.birth({ name: specification.name, plots: checked.plots });
    const opening = await body.current();
    let play = await body.wake();
    const snapshot = async () => {
      const value = await body.current();
      return { hostId: host.id, bootId: host.bootId, bodyId: body.id,
        planId: value.realization?.plan?.plan_id, playId: value.realization?.play?.active_play_id,
        lifecycle: value.evidence.body.state, installedPlots: value.evidence.body.workset.plots,
        foreground: value.foreground, evidence: value.evidence };
    };
    globalThis.__conduitApplication = Object.freeze({ snapshot,
      opening: Object.freeze({ recovered: Boolean(retained), birthState: opening.evidence.body.state }) });
    const show = () => { status.textContent = `Clock ${play ? play.state : 'lulled'}.`; lull.disabled = !play; wake.disabled = Boolean(play); };
    lull.addEventListener('click', async () => { lull.disabled = true; try { await body.lull(); play = null; show(); } catch(error) { status.textContent = error.message; } });
    wake.addEventListener('click', async () => { wake.disabled = true; try { play = await body.wake(); show(); } catch(error) { status.textContent = error.message; } });
    show();
  } catch (error) { status.textContent = `${error.code ?? 'Refused'}: ${error.message}`; }
}
