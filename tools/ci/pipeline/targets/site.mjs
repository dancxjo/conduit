import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { acquire, command, digest, xtask } from './common.mjs';
import { retainedOneBodyEvidence } from '../one-body-evidence.mjs';
import { TODO_EVIDENCE_ROOT, renderTodoJourney } from '../todo-journey.mjs';
import { retainedThreeHostDevelopmentEvidence, THREE_HOST_DEVELOPMENT_PROOF,
  THREE_HOST_DEVELOPMENT_SUITE } from '../three-host-development-evidence.mjs';
import { retainedDirectSpokenDevelopmentEvidence, DIRECT_SPOKEN_DEVELOPMENT_PROOF,
  DIRECT_SPOKEN_DEVELOPMENT_SUITE } from '../direct-spoken-development-evidence.mjs';
import { retainedTodoBrowserDevelopmentEvidence } from '../todo-browser-development-evidence.mjs';

// Retain the last documentary publication as history, never as new execution.
const HISTORY = 'd9b79319bd78c72e4a6b48ef524e269300a82bdf';
const HISTORY_DIGEST = '0218a50406761390aba1f00f84ccd178018782dfa89527f293d8ac66b9aa76c2';
const RECORDED_THREE_BODIES = '8c6f4a8bea743b733fa9de46aaeb6c6f119e1faa';
const escape = value => String(value).replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;').replaceAll('"', '&quot;');
const styles = () => readFileSync('targets/browser/host/assets/conduit.css', 'utf8') + '\n' + readFileSync('site/chrome.css', 'utf8');
const navigation = () => readFileSync('site/navigation.html', 'utf8');

export function assembleSite(directory, sourceCommit, workspace = 'target/workspace-product') {
  if (!/^[a-f0-9]{40}$/.test(sourceCommit)) throw new Error('Website requires an exact source commit');
  if (existsSync(directory)) throw new Error(`Website output already exists: ${directory}`);
  xtask('make', 'pages-root', directory);
  rmSync(path.join(directory, 'handbook'), { recursive: true });
  cpSync('target/handbook-static', path.join(directory, 'handbook'), { recursive: true, errorOnExist: true });
  cpSync('target/handbook-static-second', path.join(directory, 'examples/clock-lab'), { recursive: true, errorOnExist: true });
  cpSync(workspace, path.join(directory, 'workspace'), { recursive: true, errorOnExist: true });
  const temporary = mkdtempSync(path.join(tmpdir(), 'conduit-documentary-'));
  try {
    const archive = path.join(temporary, 'history.tar.gz');
    acquire('curl', ['--fail', '--location', '--max-time', '90',
      `https://codeload.github.com/dancxjo/conduit/tar.gz/${HISTORY}`, '--output', archive]);
    if (digest(archive) !== HISTORY_DIGEST) throw new Error('Recorded website archive digest changed');
    command('tar', ['-xzf', archive, '-C', temporary]);
    cpSync(path.join(temporary, `conduit-${HISTORY}`, 'journeys'), path.join(directory, 'journeys'), { recursive: true });
  } finally { rmSync(temporary, { recursive: true, force: true }); }
  renderFieldStation('target/journeys/handbook', path.join(directory, 'journeys/verticals/handbook'), sourceCommit, true);
  // An environment-dependent, multi-host capture cannot be recreated by browser CI
  // runner. Carry its complete raw evidence in source, then render it only
  // after preflight checks source ancestry and the strict renderer checks media.
  const oneBodyEvidence = retainedOneBodyEvidence();
  const threeHostDevelopment = oneBodyEvidence ? null : retainedThreeHostDevelopmentEvidence();
  if (oneBodyEvidence) {
    xtask('prove', 'render-one-body-journey',
      '--evidence-root', oneBodyEvidence.root,
      '--output', path.join(directory, 'journeys/current/one-body-five-masks'),
      '--commit', oneBodyEvidence.sourceCommit);
  }
  if (threeHostDevelopment && !oneBodyEvidence) {
    xtask('prove', 'verify', '--root', threeHostDevelopment.root,
      '--commit', threeHostDevelopment.sourceCommit, '--result', 'diagnostic-incomplete',
      '--proof', THREE_HOST_DEVELOPMENT_PROOF, '--suite', THREE_HOST_DEVELOPMENT_SUITE);
    const destination = path.join(directory, 'journeys/current/one-body-five-masks');
    if (existsSync(destination)) throw new Error('Retained One Body route already exists');
    cpSync(threeHostDevelopment.root, destination, { recursive: true, errorOnExist: true });
  }
  const directSpokenDevelopment = retainedDirectSpokenDevelopmentEvidence();
  if (directSpokenDevelopment) {
    xtask('prove', 'verify', '--root', directSpokenDevelopment.root,
      '--commit', directSpokenDevelopment.sourceCommit, '--result', 'diagnostic-incomplete',
      '--proof', DIRECT_SPOKEN_DEVELOPMENT_PROOF, '--suite', DIRECT_SPOKEN_DEVELOPMENT_SUITE);
    const destination = path.join(directory, 'journeys/current/direct-spoken-development');
    if (existsSync(destination)) throw new Error('Retained direct spoken route already exists');
    cpSync(directSpokenDevelopment.root, destination, { recursive: true, errorOnExist: true });
  }
  if (existsSync(TODO_EVIDENCE_ROOT) && !existsSync(path.join(TODO_EVIDENCE_ROOT, 'manifest.json'))) {
    throw new Error('Todo evidence directory exists without a complete manifest');
  }
  const todoJourney = existsSync(path.join(TODO_EVIDENCE_ROOT, 'manifest.json'))
    ? renderTodoJourney(TODO_EVIDENCE_ROOT, path.join(directory, 'journeys/current/todo'),
      sourceCommit, styles(), navigation())
    : null;
  const todoBrowserDevelopment = retainedTodoBrowserDevelopmentEvidence(
    'site/evidence/todo-browser-development', sourceCommit);
  mkdirSync(path.join(directory, 'journeys/development'), { recursive: true });
  cpSync(todoBrowserDevelopment.root,
    path.join(directory, 'journeys/development/todo-browser'),
    { recursive: true, errorOnExist: true });
  const todoBrowserPage = path.join(directory, 'journeys/development/todo-browser/index.html');
  writeFileSync(todoBrowserPage,
    readFileSync(todoBrowserPage, 'utf8').replace('</head>', `<style>${styles()}</style></head>`));
  xtask('prove', 'refresh-gallery', path.join(directory, 'journeys'));
  {
    const landing = path.join(directory, 'journeys/index.html');
    const html = readFileSync(landing, 'utf8');
    const anchor = '<!-- conduit-three-body-flagship@2 -->';
    if (!html.includes(anchor)) throw new Error('Journeys catalogue has no Todo development card seam');
    const card = '<article class="journey-card"><p class="eyebrow">Live development steps</p>'
      + '<h2>Grow a Todo Body across Masks</h2><p>Watch a browser Add an item, then see the same'
      + ' Body after terminal actions leave three items open and seventeen complete.</p>'
      + '<a href="development/todo-browser/">See the captured steps</a></article>';
    writeFileSync(landing, html.replace(anchor, `${card}${anchor}`));
  }
  if (directSpokenDevelopment) {
    const landing = path.join(directory, 'journeys/index.html');
    const html = readFileSync(landing, 'utf8');
    const anchor = '<!-- conduit-three-body-flagship@2 -->';
    if (!html.includes(anchor)) throw new Error('Journeys catalogue has no direct speech card seam');
    const card = '<article class="journey-card"><p class="eyebrow">Separate live speech proof</p>'
      + '<h2>Hear the whole Face</h2><p>Listen to 11 bounded parts of one installed Linux Host reading.'
      + ' The WAVs came from the Plays delivered to its selected speaker; this is a separate development run,'
      + ' not a chapter of the three-host journey or a claim of human hearing.</p>'
      + '<a href="current/direct-spoken-development/">Listen to the direct reading</a></article>';
    writeFileSync(landing, html.replace(anchor, `${card}${anchor}`));
  }
  // The catalogue refresh writes the historical Field Station introduction.
  // Replace it with this build's complete captured walkthrough afterwards.
  renderFieldStation('target/journeys/field-station-clock', path.join(directory, 'journeys/verticals/field-station-clock'), sourceCommit);
  xtask('prove', 'render-recorded-three-body', path.join(directory, 'journeys/current/three-bodies'));
  // The retained recording was authored two levels below Journeys. Its
  // commit-addressed copy is three levels below, so fix its two back links.
  const retainedThreeBodies = path.join(directory, 'journeys/commits', RECORDED_THREE_BODIES, 'three-bodies/index.html');
  const retainedHtml = readFileSync(retainedThreeBodies, 'utf8');
  if (retainedHtml.split('href="../../"').length !== 3) {
    throw new Error('Retained Three Bodies back-link shape changed');
  }
  writeFileSync(retainedThreeBodies, retainedHtml.replaceAll('href="../../"', 'href="../../../"'));
  // Keep technical examples in the same shell without changing their evidence.
  const littleLife = path.join(directory, 'journeys/current/little-life/index.html');
  if (existsSync(littleLife)) {
    let html = readFileSync(littleLife, 'utf8');
    html = html.replaceAll('Little Life Form', 'Little Life plot')
      .replaceAll('Form · Plan · Play · Presentation', 'Plot · Plan · Play · scalar-field presentation')
      .replace('<h1>Little Life</h1>', `<h1>Little Life</h1><p class="recording-source">Retained technical recording from source <code>${RECORDED_THREE_BODIES}</code>. This publication updates the explanation; it does not record a new execution.</p>`);
    html = html.replace('</head>', `<style>${styles()}
      body { max-width: none; padding: 0; }
      .little-life-content { max-width: 96rem; margin: auto; padding: clamp(1rem,4vw,4rem); }
      body { --ink: var(--conduit-text-primary); --muted: var(--conduit-text-secondary);
        --green: var(--conduit-emphasis); --paper: var(--conduit-background);
        --card: var(--conduit-surface); --line: var(--conduit-structure-secondary); }
      </style></head>`)
      .replace('<body>', `<body data-application-theme="conduit.presentation/phosphor@1">${navigation()}<main class="little-life-content">`)
      .replace('</body>', '</main></body>');
    writeFileSync(littleLife, html);
  }
  writeFileSync(path.join(directory, '.nojekyll'), '');
  writeFileSync(path.join(directory, 'site-publication.json'), JSON.stringify({
    schema: 'conduit.site-publication/v1', sourceCommit,
    recordedHistoryCommit: HISTORY,
    handbook: { sourceCommit, environment: 'Chromium', path: 'journeys/verticals/handbook/' },
    fieldStation: { sourceCommit, environment: 'Chromium', path: 'journeys/verticals/field-station-clock/' },
    threeBodies: { sourceCommit: RECORDED_THREE_BODIES, path: 'journeys/current/three-bodies/', refreshedExecution: false },
    ...(oneBodyEvidence && { oneBody: { captureSourceCommit: oneBodyEvidence.sourceCommit, publicationSourceCommit: sourceCommit,
      path: 'journeys/current/one-body-five-masks/', proof: 'producer-owned-complete-evidence' } }),
    ...(threeHostDevelopment && !oneBodyEvidence && { oneBodyDevelopment: { captureSourceCommit: threeHostDevelopment.sourceCommit,
      publicationSourceCommit: sourceCommit, path: 'journeys/current/one-body-five-masks/',
      proof: 'retained-local-development-evidence' } }),
    ...(directSpokenDevelopment && { directSpokenDevelopment: {
      captureSourceCommit: directSpokenDevelopment.sourceCommit, publicationSourceCommit: sourceCommit,
      path: 'journeys/current/direct-spoken-development/', proof: 'separate-retained-local-selected-alsa-evidence',
      humanListeningObserved: false,
    } }),
    ...(todoJourney && { todoJourney: {
      captureSourceCommit: todoJourney.sourceCommit, publicationSourceCommit: sourceCommit,
      path: 'journeys/current/todo/', proof: 'producer-correlated-complete-evidence',
      humanListeningObserved: false,
    } }),
    todoBrowserDevelopment: {
      captureSourceCommit: todoBrowserDevelopment.sourceCommit,
      browserRuntimeSourceCommit: todoBrowserDevelopment.browserRuntimeCommit,
      longListActionsSourceCommit: todoBrowserDevelopment.longListActionsCommit,
      longListBrowserSourceCommit: todoBrowserDevelopment.longListBrowserCommit,
      publicationSourceCommit: sourceCommit, bodyId: todoBrowserDevelopment.bodyId,
      path: 'journeys/development/todo-browser/', proof: 'retained-cross-source-browser-add',
    },
  }, null, 2));
}

function renderFieldStation(source, destination, commit, handbook = false) {
  const manifest = JSON.parse(readFileSync(path.join(source, 'index.json')));
  if (manifest.schema !== 'conduit.user-journey/v1' || manifest.sourceCommit !== commit ||
      manifest.environment !== 'Chromium' || manifest.steps?.map(step => step.id).join() !== (handbook ? 'open,check,try,inspect,edit,lull,recover' : 'open,reload,lull')) {
    throw new Error('Documentary journey is missing, incomplete, or belongs to another source');
  }
  const files = [{ path: manifest.evidence.path, sha256: manifest.evidence.sha256 },
    ...manifest.steps.map(step => ({ path: step.screenshot, sha256: step.sha256 }))];
  for (const file of files) {
    if (!/^[a-z0-9.-]+$/.test(file.path) || digest(path.join(source, file.path)) !== file.sha256) {
      throw new Error('Documentary artifact differs from its capture');
    }
  }
  const expected = ['index.json', ...files.map(file => file.path)].sort();
  if (JSON.stringify(readdirSync(source).sort()) !== JSON.stringify(expected)) throw new Error('Unexpected documentary capture files');
  rmSync(destination, { recursive: true, force: true });
  mkdirSync(destination, { recursive: true });
  cpSync(source, destination, { recursive: true });
  const steps = manifest.steps.map(step => `<li class="journey-step"><h2>${escape(step.title)}</h2><p><strong>You do:</strong> ${escape(step.action)}</p><p>${escape(step.observation)}</p><figure><a href="${escape(step.screenshot)}"><img src="${escape(step.screenshot)}" alt="${escape(step.observation)}" loading="lazy"></a><figcaption>${escape(step.title)} — captured from the running browser.</figcaption></figure></li>`).join('');
  if (handbook) {
    writeFileSync(path.join(destination, 'index.html'), `<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Your local Handbook · Conduit journeys</title><style>${styles()}</style></head><body data-application-theme="conduit.presentation/phosphor@1">${navigation()}<main class="site-content"><p><a href="../../">All journeys</a></p><p class="eyebrow">Chromium · seven steps</p><h1>${escape(manifest.title)}</h1><p class="lede">${escape(manifest.summary)}</p><h2>Before you begin</h2><p>Open <a href="/conduit/handbook/">your Handbook</a> in a browser with local storage enabled. This recording uses a fresh Chromium profile and static files. Each browser installation creates its own local Body; no shared application backend is required.</p><ol class="journey-steps">${steps}</ol><h2>What stayed with you?</h2><p>The Body kept its edited clock and workset through a reload. The new Boot admitted fresh execution; the recording does not claim the clock ran while the page was closed.</p><p>Try it in <a href="/conduit/handbook/">your own Handbook</a>, or <a href="../field-station-clock/">follow a smaller clock journey</a>.</p><details><summary>About this recording</summary><p>Captured from actual application acceptance in Chromium at source <code>${escape(commit)}</code>.</p><p><a href="index.json">Screenshot inventory</a> · <a href="runtime-evidence.json">Observed runtime identities</a></p></details></main></body></html>`);
    return;
  }
  writeFileSync(path.join(destination, 'index.html'), `<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Field Station Clock · Conduit journeys</title><style>${styles()}</style></head><body data-application-theme="conduit.presentation/phosphor@1">${navigation()}<main class="site-content"><p><a href="../../">All journeys</a></p><p class="eyebrow">Browser · three steps</p><h1>Return to your clock after a reload</h1><p class="lede">Open the clock, reload the page, and stop it when you are finished. This walkthrough shows what stays with the body and what starts again when you return.</p><h2>Before you begin</h2><p>This recording uses the Field Station browser example. Opening the page creates and starts the clock automatically. You need a browser with local storage enabled.</p><ol class="journey-steps">${steps}</ol><h2>What happened?</h2><p>The same body returned after reload, with a new run of its clock. Lull stopped that run while keeping the body. The recording does not show the clock running while the browser was closed.</p><p>Next: <a href="../../">choose another Journey</a>, try the <a href="../handbook/">Handbook walkthrough</a>, or read <a href="/conduit/handbook/Bodies-hosts-plans-and-plays.html">how bodies keep their identity</a>.</p><details><summary>About this recording</summary><p>Captured in Chromium from source <code>${escape(commit)}</code>.</p><p><a href="index.json">Screenshot inventory</a> · <a href="runtime-evidence.json">Recorded runtime observations</a></p></details></main></body></html>`);
}
