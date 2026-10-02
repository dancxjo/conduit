// Browser controls orchestrate public SDK operations; Rust owns every Body transition.
export async function startApplication(application) {
  const navigation = document.querySelector('[data-site-navigation]');
  if (navigation) navigation.innerHTML = application.text('site-navigation');
  const root = document.querySelector('[data-handbook-application]') ?? document.createElement('section');
  if (!root.isConnected) document.querySelector('main').append(root);
  root.className = 'handbook-application';
  root.innerHTML = `<header><p class="eyebrow">Your local body</p><h2>Read. Try. Look inside.</h2>
    <p>The site supplies the starting material. Your browser creates this instance and keeps its continuity locally.</p></header>
    <p role="status" data-session-status>Opening your Handbook…</p>
    <div class="handbook-controls"><button data-wake>Wake</button><button data-lull>Lull</button><button data-patchbay>Open in Patchbay</button><button data-example>Return to example</button></div>
    <label>Choose an example <select aria-label="Choose an example"></select></label>
    <p data-lesson></p><label for="handbook-source">Plot source</label><textarea id="handbook-source" spellcheck="false" aria-label="Plot source"></textarea>
    <p data-check role="status"></p><button data-try>Try in my Handbook</button>
    <p>First predict the result, then try it. Open Patchbay to follow the gears, typed ports, and cords that produced the result.</p>
    <section class="handbook-show" tabindex="0" aria-label="Running example"></section>
    <details><summary>Your body and browser</summary><p data-durability></p><pre data-identities></pre>
      <button data-release>Release this tab</button><button data-reset>Start my Handbook over</button>
      <p>Starting over removes this application's local body and examples. It keeps your browser Host identity.</p></details>
    <button data-retry hidden>Try again</button>`;
  const status = root.querySelector('[data-session-status]');
  const source = root.querySelector('textarea');
  const selector = root.querySelector('select');
  const surface = root.querySelector('.handbook-show');
  const checkStatus = root.querySelector('[data-check]');
  const controls = [...root.querySelectorAll('button,select,textarea')];
  controls.forEach(control => control.disabled = true);
  const specification = JSON.parse(application.text('birth-specification'));
  const bundle = JSON.stringify({ schema: 'conduit.creche/reviewed-plot-bundle@2', plots: specification.plots.map(plot => ({
    slug: plot.name, entry: plot.name, title: plot.title, source: application.text(plot.role),
  })) });
  let host, body, play, checked, editor, selected, snapshot;
  let busy = false;
  let foreground = null;
  const surfaces = new Map();
  const updateControls = () => {
    root.querySelector('[data-wake]').disabled = Boolean(play);
    root.querySelector('[data-lull]').disabled = !play;
    root.querySelector('[data-example]').disabled = !snapshot?.evidence.body.workset.plots
      .some(plot => plot.checked_plot_id === selected?.checkedPlotId);
  };
  const show = identity => {
    foreground = identity;
    for (const [id, output] of surfaces) output.hidden = id !== identity;
  };
  const readTruth = async () => {
    snapshot = await body.current();
    const realization = snapshot.realization;
    const state = snapshot.evidence.body.state;
    const truth = { hostId: host.id, bootId: host.bootId, bodyId: body.id,
      planId: realization?.plan?.plan_id ?? null, playId: realization?.play?.active_play_id ?? null,
      execution: play?.state ?? 'none',
      lifecycle: typeof state === 'string' ? state : Object.keys(state)[0],
      selectedPlot: selected?.checkedPlotId ?? null,
      installedPlots: snapshot.evidence.body.workset.plots, foreground: snapshot.foreground,
      evidence: snapshot.evidence };
    return truth;
  };
  const current = async () => {
    const truth = await readTruth();
    root.querySelector('[data-identities]').textContent = JSON.stringify(truth, null, 2);
    status.textContent = `Your Handbook is ${truth.lifecycle.toLowerCase()}. Play: ${truth.execution}.`;
    updateControls();
    return truth;
  };
  const wake = async () => {
    if (play) return;
    surface.replaceChildren(); surfaces.clear();
    play = await body.wake({ root: surface, presentationRootFor({ checkedPlotId }) {
      if (!surfaces.has(checkedPlotId)) {
        const output = document.createElement('div');
        output.dataset.residentPlot = checkedPlotId;
        output.hidden = foreground !== checkedPlotId;
        surface.append(output); surfaces.set(checkedPlotId, output);
      }
      return surfaces.get(checkedPlotId);
    } });
    const active = play;
    active.dispatch().then(async () => {
      if (play === active && !busy) await current();
    }, async error => {
      if (play !== active) return;
      await current();
      status.textContent += ` ${error.code ?? 'PlayExecutionFailed'}: ${error.message}`;
      status.dataset.refused = 'true';
    }).catch(error => {
      status.textContent = `${error.code ?? 'Refused'}: ${error.message}`;
      status.dataset.refused = 'true';
    });
  };
  const lull = async () => { if (play) { await body.lull(); play = null; } };
  const operate = async action => {
    if (busy) return;
    busy = true;
    controls.forEach(control => control.disabled = true);
    try { delete status.dataset.refused; await action(); await current(); }
    catch (error) { status.textContent = `${error.code ?? 'Refused'}: ${error.message}`; status.dataset.refused = 'true'; }
    finally {
      busy = false;
      controls.forEach(control => control.disabled = false);
      updateControls();
    }
  };
  const selectExample = async () => {
    const entry = specification.plots.find(plot => plot.name === selector.value);
    selected = checked.plots.find(plot => plot.name === entry.name);
    source.value = selected.source;
    editor.render();
    root.querySelector('[data-lesson]').textContent = entry.lesson;
    checkStatus.textContent = 'Edit the source, then check and try it in your body.';
    await application.storage.writeJson('selected-example', entry.name);
  };
  try {
    host = await application.browser({ root: surface });
    checked = await host.plot(bundle).check();
    if (!checked.ok) throw new Error(checked.refusal.message);
    body = await host.recover();
    const recovered = Boolean(body);
    if (!body) body = await host.birth({ name: specification.name,
      plots: checked.plots.filter(plot => specification.initialPlots.includes(plot.name)) });
    checked = await body.plots();
    if (!checked.ok) throw new Error(checked.refusal.message);
    const opening = await body.current();
    const birthState = opening.evidence.body.state;
    editor = host.attachEditor(source);
    for (const plot of specification.plots.filter(plot => plot.lesson)) {
      const option = document.createElement('option'); option.value = plot.name; option.textContent = plot.title; selector.append(option);
    }
    const retained = await application.storage.readJson('selected-example');
    if (specification.plots.some(plot => plot.lesson && plot.name === retained)) selector.value = retained;
    await selectExample();
    const installed = opening.evidence.body.workset.plots;
    const initial = checked.plots.find(plot => plot.name === (!recovered ? specification.initialPlots[0] : null))
      ?? checked.plots.find(plot => plot.checkedPlotId === opening.foreground?.checked_plot_id)
      ?? checked.plots.find(plot => installed.some(entry => entry.checked_plot_id === plot.checkedPlotId));
    await body.select(initial); show(initial.checkedPlotId); await wake();
    root.querySelector('[data-durability]').textContent = JSON.stringify(await application.storage.durability());
    globalThis.__conduitApplication = Object.freeze({ snapshot: readTruth, host: Object.freeze({ current: () => host.current() }),
      opening: Object.freeze({ recovered, birthState, snapshot: opening }) });
    await current();
    controls.forEach(control => control.disabled = false);
    updateControls();
    selector.addEventListener('change', () => operate(selectExample));
    root.querySelector('[data-try]').addEventListener('click', () => operate(async () => {
      const result = await host.plot(source.value).check();
      if (!result.ok) { checkStatus.dataset.refusalCode = result.refusal.code; checkStatus.textContent = `${result.refusal.code}: ${result.diagnostics.map(item => item.message).join('\n') || result.refusal.message}`; return; }
      if (result.plots.length !== 1) throw new Error('Choose one plot for this lesson');
      const next = result.plots[0];
      if (next.name !== selector.value) {
        checkStatus.dataset.refusalCode = 'LessonPlotNameChanged';
        checkStatus.textContent = `Keep the plot name ${selector.value} for this lesson so your saved edits stay with the example.`;
        return;
      }
      delete checkStatus.dataset.refusalCode;
      checkStatus.textContent = `Checked ${next.name}.`;
      await lull();
      const state = await body.current();
      const previous = state.evidence.body.workset.plots.find(plot => plot.checked_plot_id === selected.checkedPlotId);
      if (previous && previous.checked_plot_id !== next.checkedPlotId) await body.replace(selected, next);
      else if (!state.evidence.body.workset.plots.some(plot => plot.checked_plot_id === next.checkedPlotId)) await body.install(next);
      checked = await body.plots();
      selected = checked.plots.find(plot => plot.checkedPlotId === next.checkedPlotId);
      await body.select(selected); show(selected.checkedPlotId); await wake();
      surface.focus();
    }));
    root.querySelector('[data-patchbay]').addEventListener('click', () => operate(async () => {
      const patchbay = checked.plots.find(plot => plot.name === 'patchbay');
      await body.select(patchbay); show(patchbay.checkedPlotId); await wake();
    }));
    root.querySelector('[data-example]').addEventListener('click', () => operate(async () => {
      await body.select(selected); show(selected.checkedPlotId); await wake(); surface.focus();
    }));
    root.querySelector('[data-wake]').addEventListener('click', () => operate(wake));
    root.querySelector('[data-lull]').addEventListener('click', () => operate(lull));
    root.querySelector('[data-reset]').addEventListener('click', async () => {
      if (busy) return; busy = true;
      try { await host.forget(); location.reload(); }
      catch (error) { status.textContent = error.message; busy = false; }
    });
    root.querySelector('[data-release]').addEventListener('click', async () => {
      if (busy) return; busy = true;
      try { await host.close(); editor.destroy(); controls.forEach(control => control.disabled = true); status.textContent = 'This tab released your Handbook. Another tab can now recover it.'; }
      catch (error) { status.textContent = error.message; busy = false; }
    });
  } catch (error) {
    status.textContent = `${error.code ?? 'Refused'}: ${error.message}`;
    status.dataset.refused = 'true';
    controls.forEach(control => control.disabled = true);
    const retry = root.querySelector('[data-retry]'); retry.hidden = false; retry.disabled = false;
    retry.addEventListener('click', () => location.reload());
  }
}
