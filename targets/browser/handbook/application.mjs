// Browser controls orchestrate public SDK operations; Rust owns every Body transition.
import { startOwnerParticipation } from "./owner-participation.mjs";

export async function startApplication(application) {
  const root = document.querySelector('[data-handbook-application]') ?? document.createElement('section');
  if (!root.isConnected) document.querySelector('main').append(root);
  root.className = 'handbook-application';
  if (new URLSearchParams(location.search).get('participate') === 'owner') {
    await startOwnerParticipation(application, root);
    return;
  }
  root.innerHTML = `<header><p class="eyebrow">Your local body</p><h2>Read. Try. Look inside.</h2>
    <p>The site supplies the starting material. Your browser creates this instance and keeps its continuity locally.</p></header>
    <p role="status" data-session-status>Opening your Handbook…</p>
    <div class="handbook-controls"><button data-wake>Wake</button><button data-lull>Lull</button><button data-patchbay>Open in Patchbay</button><button data-example>Return to example</button></div>
    <label>Choose an example <select aria-label="Choose an example"></select></label>
    <p data-lesson></p><label for="handbook-source">Plot source</label><textarea id="handbook-source" spellcheck="false" aria-label="Plot source"></textarea>
    <details><summary>Inspect source provenance</summary><pre data-source-provenance></pre></details>
    <p data-check role="status"></p><button data-try>Try in my Handbook</button>
    <p>First predict the result, then try it. Open Patchbay to follow the gears, typed ports, and cords of the installed example. Trying another example stops the previous example; saved edits are kept. Unapplied edits stay in the editor.</p>
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
  const sourceEvidence = root.querySelector('[data-source-provenance]');
  let inspectedSource, evidenceGeneration = 0;
  const updateSourceEvidence = () => {
    if (!inspectedSource) return;
    const generation = ++evidenceGeneration;
    const value = source.value;
    const entry = specification.plots.find(plot => plot.name === inspectedSource.name);
    const resource = application.manifest?.resources?.find(resource => resource.role === entry?.role);
    const evidence = {
      application_id: application.manifest?.applicationId ?? null,
      package_digest: application.manifest?.packageDigest ?? null,
      source_origin: inspectedSource.origin, plot: inspectedSource.name,
      packaged_role: entry?.role ?? null, packaged_source_sha256: resource?.sha256 ?? null,
      local_edit: value !== inspectedSource.source, displayed_source_sha256: null,
    };
    sourceEvidence.textContent = JSON.stringify(evidence, null, 2);
    // Hashing is observational and never blocks editing, checking or startup.
    if (!globalThis.crypto?.subtle) return;
    crypto.subtle.digest('SHA-256', new TextEncoder().encode(value)).then(bytes => {
      if (generation !== evidenceGeneration) return;
      evidence.displayed_source_sha256 = 'sha256:' + [...new Uint8Array(bytes)]
        .map(byte => byte.toString(16).padStart(2, '0')).join('');
      sourceEvidence.textContent = JSON.stringify(evidence, null, 2);
    }).catch(() => {});
  };
  const inspectSource = (origin, name, original) => {
    inspectedSource = { origin, name, source: original };
    updateSourceEvidence();
  };
  source.addEventListener('input', updateSourceEvidence);
  const showCheckRefusal = (result, origin, name) => {
    checkStatus.dataset.refusalCode = result.refusal.code;
    const details = result.diagnostics.map(item => {
      const span = item.span;
      const position = Number.isInteger(span?.line) && Number.isInteger(span?.column)
        ? ` at line ${span.line}, column ${span.column}` : '';
      return `${item.code ?? result.refusal.code}${position}: ${item.message}`;
    }).join('\n') || `${result.refusal.code}: ${result.refusal.message}`;
    checkStatus.textContent = `${origin} source ${name} — checking refused. ${details}`;
  };
  const controls = [...root.querySelectorAll('button,select,textarea')];
  controls.forEach(control => control.disabled = true);
  const specification = JSON.parse(application.text('birth-specification'));
  // A corrected packaged lesson may be retried without changing the signed package
  // or the original source. Overrides are exact-source scoped and independently
  // checked by the Rust checker on the next Boot.
  const rescueEdits = await application.storage.readJson('handbook-source-rescue').catch(() => null);
  const validRescueEdits = Array.isArray(rescueEdits) ? rescueEdits.slice(0, specification.plots.length) : [];
  const packagedPlots = specification.plots.map(plot => {
    const original = application.text(plot.role);
    const rescue = validRescueEdits.find(item => item?.name === plot.name && item.original === original
      && typeof item.source === 'string' && new TextEncoder().encode(item.source).length <= 16_384);
    return { slug: plot.name, entry: plot.name, title: plot.title, source: rescue?.source ?? original };
  });
  // Source inspection is available before any checker, Host or Body starts.
  // Drafts remain unapplied UI input; the original reviewed bundle stays exact.
  const openingDrafts = new Map();
  let openingChoice = null;
  for (const plot of specification.plots.filter(plot => plot.lesson)) {
    const option = document.createElement('option'); option.value = plot.name; option.textContent = plot.title; selector.append(option);
  }
  const inspectOpeningSource = () => {
    const plot = packagedPlots.find(plot => plot.entry === selector.value);
    if (!plot) return;
    source.value = openingDrafts.get(plot.entry) ?? plot.source;
    const entry = specification.plots.find(entry => entry.name === plot.entry);
    inspectSource(plot.source === application.text(entry.role) ? 'packaged' : 'saved correction', plot.entry, plot.source);
    root.querySelector('[data-lesson]').textContent = specification.plots.find(entry => entry.name === plot.entry).lesson;
    checkStatus.textContent = 'Source editing is available. Checking and Body startup are pending.';
  };
  const chooseOpeningSource = () => { openingChoice = selector.value; inspectOpeningSource(); };
  const editOpeningSource = () => {
    openingChoice = selector.value;
    openingDrafts.set(openingChoice, source.value);
    checkStatus.textContent = 'Edited source, not checked or applied.';
  };
  const finishOpeningInspection = () => {
    selector.removeEventListener('change', chooseOpeningSource);
    source.removeEventListener('input', editOpeningSource);
  };
  inspectOpeningSource();
  source.disabled = false; selector.disabled = false;
  selector.addEventListener('change', chooseOpeningSource);
  source.addEventListener('input', editOpeningSource);
  const bundle = JSON.stringify({ schema: 'conduit.creche/reviewed-plot-bundle@2', plots: packagedPlots });
  let host, body, play, checked, reviewed, editor, selected, snapshot;
  let busy = false;
  let openingStage = 'Browser startup';
  let playFailure = null;
  let foreground = null;
  let thermostatSequence = 0;
  let thermostatFace = null;
  const surfaces = new Map();
  const renderThermostat = async face => {
    const active = play;
    await readTruth();
    if (!active || selected?.name !== 'thermostat/main') return;
    let output = surfaces.get(selected.checkedPlotId);
    if (!output) {
      output = document.createElement('div');
      output.dataset.residentPlot = selected.checkedPlotId;
      output.hidden = foreground !== selected.checkedPlotId;
      surface.append(output); surfaces.set(selected.checkedPlotId, output);
    }
    let component = output.querySelector('conduit-owner-face');
    if (!component) {
      const heading = document.createElement('h3'); heading.textContent = 'Thermostat'; output.append(heading);
      component = document.createElement('conduit-owner-face'); output.append(component);
    }
    const invoke = async ({ view, action, arguments: actionArguments }) => {
      component.disabled = true;
      try {
        const receipt = active.submitThermostatInteraction({ view, actionId: action.identity,
          target: action.target, arguments: actionArguments, sequence: ++thermostatSequence });
        await renderThermostat(receipt.face);
        checkStatus.textContent = `Accepted ${action.name}. Settings remain in this Play.`;
        await current();
      } catch (error) {
        checkStatus.textContent = `${error.code ?? 'ThermostatRefused'}: ${error.message}`;
        component.disabled = false;
      }
    };
    const supportsAction = action => action.identity.startsWith('thermostat.');
    component.show(face.view, { invoke, supportsAction });
    // Acknowledge only after the selected browser Mask's real DOM is present.
    const available = active.acknowledgeThermostatFace(face.effect);
    thermostatFace = available;
    component.show(available.view, { invoke, supportsAction });
  };
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
      evidence: snapshot.evidence, realization };
    return truth;
  };
  const current = async () => {
    const truth = await readTruth();
    root.querySelector('[data-identities]').textContent = JSON.stringify(truth, null, 2);
    status.textContent = `Your Handbook is ${truth.lifecycle.toLowerCase()}. Play: ${truth.execution}.`;
    if (playFailure) {
      status.textContent += ` ${playFailure.code ?? 'PlayExecutionFailed'}: ${playFailure.message}`;
      status.dataset.refused = 'true';
    }
    updateControls();
    return truth;
  };
  const wake = async () => {
    if (play) return;
    playFailure = null;
    thermostatFace = null; thermostatSequence = 0;
    surface.replaceChildren(); surfaces.clear();
    play = await body.wake({ root: surface, faceRootFor({ checkedPlotId }) {
      if (!surfaces.has(checkedPlotId)) {
        const output = document.createElement('div');
        output.dataset.residentPlot = checkedPlotId;
        output.hidden = foreground !== checkedPlotId;
        surface.append(output); surfaces.set(checkedPlotId, output);
      }
      return surfaces.get(checkedPlotId);
    } });
    const active = play;
    await readTruth();
    if (selected?.name === 'thermostat/main'
        && foreground === selected.checkedPlotId
        && snapshot?.evidence.body.workset.plots.some(plot => plot.checked_plot_id === selected.checkedPlotId)) {
      await renderThermostat(active.thermostatFace());
    }
    active.dispatch().then(async () => {
      if (play === active && !busy) await current();
    }, async error => {
      if (play !== active) return;
      // The active control operation will render after its transition settles.
      // Preserve a real failure without queuing a stale observer inside it.
      playFailure = error;
      if (!busy) await current();
    }).catch(error => {
      status.textContent = `${error.code ?? 'Refused'}: ${error.message}`;
      status.dataset.refused = 'true';
    });
  };
  const lull = async () => {
    if (play) {
      await body.lull(); play = null; playFailure = null;
      for (const component of surface.querySelectorAll('conduit-owner-face')) component.disabled = true;
    }
  };
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
    // Retained edits take precedence; new packaged lessons remain selectable
    // in Bodies born before this application update. Installation is explicit.
    selected = checked.plots.find(plot => plot.name === entry.name)
      ?? reviewed.plots.find(plot => plot.name === entry.name);
    source.value = openingDrafts.get(entry.name) ?? selected.source;
    inspectSource('Body inventory', entry.name, selected.source);
    openingDrafts.delete(entry.name);
    editor.render();
    root.querySelector('[data-lesson]').textContent = entry.lesson;
    checkStatus.textContent = 'Edit the source, then check and try it in your body.';
    await application.storage.writeJson('selected-example', entry.name);
  };
  try {
    host = await application.browser({ root: surface });
    openingStage = 'Packaged source checking';
    checked = await host.plot(bundle).check();
    if (!checked.ok) throw new Error(checked.refusal.message);
    reviewed = checked;
    openingStage = 'Retained Body recovery';
    body = await host.recover();
    const recovered = Boolean(body);
    if (!body) {
      openingStage = 'Body birth';
      body = await host.birth({ name: specification.name,
        plots: checked.plots.filter(plot => specification.initialPlots.includes(plot.name)) });
    }
    openingStage = 'Retained source checking';
    checked = await body.plots();
    if (!checked.ok) throw new Error(checked.refusal.message);
    const opening = await body.current();
    const birthState = opening.evidence.body.state;
    finishOpeningInspection();
    editor = host.attachEditor(source);
    source.addEventListener('input', () => {
      delete checkStatus.dataset.refusalCode;
      checkStatus.textContent = 'Edited source — not checked or applied yet. Try it to check and apply the change.';
    });
    const retained = openingChoice ?? await application.storage.readJson('selected-example');
    if (specification.plots.some(plot => plot.lesson && plot.name === retained)) selector.value = retained;
    await selectExample();
    const installed = opening.evidence.body.workset.plots;
    const initial = checked.plots.find(plot => plot.name === (!recovered ? specification.initialPlots[0] : null))
      ?? checked.plots.find(plot => plot.checkedPlotId === opening.foreground?.checked_plot_id)
      ?? checked.plots.find(plot => installed.some(entry => entry.checked_plot_id === plot.checkedPlotId));
    openingStage = 'Play admission';
    await body.select(initial); show(initial.checkedPlotId); await wake();
    root.querySelector('[data-durability]').textContent = JSON.stringify(await application.storage.durability());
    globalThis.__conduitApplication = Object.freeze({ snapshot: readTruth, host: Object.freeze({ current: () => host.current() }),
      thermostat: Object.freeze({
        face: () => { if (!play) throw new Error('Thermostat Play is unavailable'); return play.thermostatFace(); },
        interact: async request => {
          if (!play) throw new Error('Thermostat Play is unavailable');
          const receipt = play.submitThermostatInteraction(request);
          await renderThermostat(receipt.face);
          return { ...receipt, face: thermostatFace };
        },
      }),
      opening: Object.freeze({ recovered, birthState, snapshot: opening }) });
    await current();
    controls.forEach(control => control.disabled = false);
    updateControls();
    selector.addEventListener('change', () => operate(selectExample));
    root.querySelector('[data-try]').addEventListener('click', () => operate(async () => {
      const result = await host.plot(source.value).check();
      if (!result.ok) { showCheckRefusal(result, 'edited', selector.value); return; }
      // Reusable transition declarations belong to the selected document;
      // install only its explicitly selected resident entry.
      const next = result.plots.find(plot => plot.name === selector.value);
      if (!next) throw new Error(`Keep the plot name ${selector.value} for this lesson`);
      if (next.name !== selector.value) {
        checkStatus.dataset.refusalCode = 'LessonPlotNameChanged';
        checkStatus.textContent = `Keep the plot name ${selector.value} for this lesson so your saved edits stay with the example.`;
        return;
      }
      delete checkStatus.dataset.refusalCode;
      checkStatus.textContent = `Checked ${next.name}.`;
      await lull();
      const state = await body.current();
      // Keep a single resident lesson. Removing a Plot from the workset leaves
      // its retained source inventory (including checked edits) available.
      for (const candidate of checked.plots) {
        if (candidate.checkedPlotId !== selected.checkedPlotId
          && specification.plots.some(entry => entry.lesson && entry.name === candidate.name)
          && state.evidence.body.workset.plots.some(plot => plot.checked_plot_id === candidate.checkedPlotId)) {
          await body.remove(candidate);
        }
      }
      const previous = state.evidence.body.workset.plots.find(plot => plot.checked_plot_id === selected.checkedPlotId);
      if (previous && previous.checked_plot_id !== next.checkedPlotId) await body.replace(selected, next);
      else if (!state.evidence.body.workset.plots.some(plot => plot.checked_plot_id === next.checkedPlotId)) await body.install(next);
      checked = await body.plots();
      selected = checked.plots.find(plot => plot.checkedPlotId === next.checkedPlotId);
      inspectSource('Body inventory', selected.name, selected.source);
      await body.select(selected); show(selected.checkedPlotId); await wake();
      surface.focus();
    }));
    root.querySelector('[data-patchbay]').addEventListener('click', () => operate(async () => {
      const patchbay = checked.plots.find(plot => plot.name === 'patchbay');
      await body.select(patchbay); show(patchbay.checkedPlotId); await wake();
    }));
    root.querySelector('[data-example]').addEventListener('click', () => operate(async () => {
      await body.select(selected); show(selected.checkedPlotId); await wake(); surface.focus();
      if (selected.name === 'thermostat/main') await renderThermostat(play.thermostatFace());
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
    status.textContent = `${openingStage} refused: ${error.code ?? 'Refused'}: ${error.message}`;
    status.dataset.refused = 'true';
    controls.forEach(control => control.disabled = true);
    source.disabled = false; selector.disabled = false;
    const retry = root.querySelector('[data-retry]');
    retry.hidden = false; retry.disabled = false;
    retry.addEventListener('click', () => location.reload());

    // Recovery must never depend on successful checking, Body admission, or Play.
    // Present exact source, not a fabricated runnable Body or silent syntax migration.
    let retained = null;
    try { retained = await application.storage.readJson('body-continuity'); } catch { /* Show packaged source. */ }
    const originals = [];
    if (typeof retained?.source === 'string') {
      try {
        const record = JSON.parse(retained.source);
        if (record?.schema === 'conduit.creche/reviewed-plot-bundle@2' && Array.isArray(record.plots)) {
          for (const plot of record.plots.slice(0, 32)) {
            if (typeof plot?.source === 'string' && plot.source.length <= 32_768) {
              originals.push({ origin: 'retained', name: plot.entry ?? plot.slug ?? 'retained',
                title: plot.title ?? plot.entry ?? 'Retained Plot', source: plot.source });
            }
          }
        }
      } catch { /* Single historical source, not a JSON bundle. */ }
      if (originals.length === 0) originals.push({ origin: 'retained', name: 'retained',
        title: 'Retained source', source: retained.source });
    }
    const available = [
      ...originals,
      ...packagedPlots.flatMap(plot => {
        const entry = specification.plots.find(entry => entry.name === plot.entry);
        const original = application.text(entry.role);
        const packaged = { origin: 'packaged', name: plot.entry, title: plot.title, source: original };
        // A saved correction is local input, never the original package bytes.
        return plot.source === original ? [packaged] : [
          { ...packaged, origin: 'saved correction', source: plot.source }, packaged,
        ];
      }),
    ];
    let index = 0;
    if (host && !openingChoice) {
      // This probe is diagnostics only. It creates no Body, Plan, or Play.
      for (let i = 0; i < available.length; i++) {
        try {
          const probe = await host.plot(available[i].source).check();
          if (!probe.ok) { index = i; break; }
        } catch { index = i; break; }
      }
    }
    finishOpeningInspection();
    const effectiveSource = plot => plot.origin !== 'retained'
      && plot.source === packagedPlots.find(entry => entry.entry === plot.name)?.source;
    const chosen = available.findIndex(plot => effectiveSource(plot) && plot.name === openingChoice);
    if (chosen >= 0) index = chosen;
    const drafts = available.map(plot => effectiveSource(plot)
      ? openingDrafts.get(plot.name) ?? plot.source : plot.source);
    selector.replaceChildren();
    for (let i = 0; i < available.length; i++) {
      const option = document.createElement('option');
      option.value = String(i);
      option.textContent = available[i].title + ' (' + available[i].origin + ')';
      selector.append(option);
    }
    const showSource = () => {
      source.value = drafts[index];
      inspectSource(available[index].origin, available[index].name, available[index].source);
      root.querySelector('[data-lesson]').textContent = 'Recovery source only. The current Body has not been changed.';
      checkStatus.textContent = 'Edit this source and press Try to check it without executing anything.';
      try { editor ??= host?.attachEditor(source); editor?.render(); }
      catch { /* Plain editable textarea is the source-of-truth fallback. */ }
    };
    selector.value = String(index);
    showSource();
    source.disabled = false;
    selector.disabled = false;
    root.querySelector('[data-try]').disabled = !host;
    selector.addEventListener('change', () => {
      drafts[index] = source.value;
      index = Number(selector.value);
      showSource();
    });
    source.addEventListener('input', () => {
      drafts[index] = source.value;
      checkStatus.textContent = 'Edited source, not yet checked or applied.';
      delete checkStatus.dataset.refusalCode;
    });
    root.querySelector('[data-try]').addEventListener('click', async () => {
      const candidate = available[index];
      const value = source.value;
      if (new TextEncoder().encode(value).length > 16_384) {
        checkStatus.textContent = 'Recovery source exceeds the 16 KiB editor bound.';
        return;
      }
      try {
        const result = await host.plot(value).check();
        if (source.value !== value || available[index] !== candidate) return;
        if (!result.ok) {
          showCheckRefusal(result, candidate.origin, candidate.name);
          return;
        }
        delete checkStatus.dataset.refusalCode;
        if (retained) {
          checkStatus.textContent = 'Source checks. The retained Body has not been changed: '
            + 'its signed source/Plot identities cannot be rewritten by this editor. '
            + 'Export its record before explicitly starting a new Body.';
          return;
        }
        if (!['packaged', 'saved correction'].includes(candidate.origin)) return;
        const original = application.text(specification.plots.find(plot => plot.name === candidate.name).role);
        const updated = validRescueEdits.filter(item => item?.name !== candidate.name);
        updated.push({ name: candidate.name, original, source: value });
        await application.storage.writeJson('handbook-source-rescue', updated);
        if (available[index] === candidate && source.value === value) {
          inspectSource('saved correction', candidate.name, value);
        }
        checkStatus.textContent = 'Source checks. Correction saved for this application; '
          + 'press Try again to reopen the Handbook. No Body has been changed.';
      } catch (refusal) {
        checkStatus.textContent = (refusal.code ?? 'CheckRefused') + ': ' + refusal.message;
      }
    });

    // A backup is explicit and read-only. History may include independently
    // retained archives; this file is the continuity record, not a full export.
    if (retained) {
      const backup = document.createElement('button');
      backup.type = 'button';
      backup.textContent = 'Download original retained Body record';
      backup.addEventListener('click', () => {
        const blob = new Blob([JSON.stringify(retained, null, 2)], { type: 'application/json' });
        const url = URL.createObjectURL(blob);
        const link = document.createElement('a');
        link.href = url; link.download = 'conduit-handbook-retained-body.json';
        link.click();
        setTimeout(() => URL.revokeObjectURL(url), 0);
      });
      const explanation = document.createElement('p');
      explanation.textContent = 'The original retained Body has not been reset or rewritten. '
        + 'The download preserves its continuity record, but may omit separately archived history. '
        + 'Recovering or migrating this Body requires explicit, checked compatibility.';
      const consentLabel = document.createElement('label');
      const consent = document.createElement('input');
      consent.type = 'checkbox';
      consentLabel.append(consent, ' I have saved what I need and understand that a new Body replaces this application’s retained state and separately archived history.');
      const replacement = document.createElement('button');
      replacement.type = 'button';
      replacement.textContent = 'Start a new Handbook Body';
      replacement.disabled = true;
      consent.disabled = !host;
      consent.addEventListener('change', () => {
        replacement.disabled = !host || !consent.checked || busy;
      });
      replacement.addEventListener('click', async () => {
        if (!host || !consent.checked || busy) return;
        busy = true; replacement.disabled = true;
        try { await host.forget(); location.reload(); }
        catch (refusal) {
          status.textContent = (refusal.code ?? 'ReplacementRefused') + ': ' + refusal.message;
          busy = false; replacement.disabled = !consent.checked;
        }
      });
      explanation.textContent += ' Starting a new Body removes this application’s retained state and archived history; it keeps the browser Host identity. Download the original record and save any edits you need before continuing.';
      checkStatus.after(explanation, backup, consentLabel, replacement);
    }
  }
}
