import { createHash } from 'node:crypto';
import { cpSync, existsSync, lstatSync, mkdirSync, readFileSync, readdirSync, renameSync, rmSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';

export const TODO_EVIDENCE_ROOT = 'site/evidence/todo-journey';
const CHAPTERS = ['birth', 'add', 'join', 'complete', 'inspect', 'hear', 'read', 'recover'];
const SHA = /^[a-f0-9]{64}$/;
const COMMIT = /^[a-f0-9]{40}$/;
const MAX_OUTPUTS = 128;
const MAX_FILE = 16 * 1024 * 1024;
const MAX_TOTAL = 128 * 1024 * 1024;
const escape = value => String(value).replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;').replaceAll('"', '&quot;').replaceAll("'", '&#39;');
const insist = (condition, message) => { if (!condition) throw new Error(`Todo journey: ${message}`); };
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const nonempty = value => typeof value === 'string' && value.trim().length > 0;

function safePath(value) {
  insist(typeof value === 'string' && /^[a-z0-9][a-z0-9._/-]*$/.test(value)
    && !value.split('/').includes('..') && !value.split('/').includes('.'), 'unsafe evidence path');
  return value;
}

function json(bytes, label) {
  try { return JSON.parse(bytes.toString('utf8')); }
  catch { throw new Error(`Todo journey: invalid ${label} JSON`); }
}

function filesUnder(root, relative = '') {
  const found = [];
  for (const name of readdirSync(path.join(root, relative))) {
    const file = path.join(relative, name);
    const stat = lstatSync(path.join(root, file));
    insist(!stat.isSymbolicLink(), 'symlinked evidence input');
    if (stat.isDirectory()) found.push(...filesUnder(root, file));
    else { insist(stat.isFile(), 'non-file evidence input'); found.push(file); }
  }
  return found;
}

function wavPcm(bytes, claimed) {
  insist(bytes.length >= 44 && bytes.toString('ascii', 0, 4) === 'RIFF'
    && bytes.toString('ascii', 8, 12) === 'WAVE', 'audio is not RIFF WAV');
  insist(bytes.readUInt32LE(4) + 8 === bytes.length, 'WAV length mismatch');
  let offset = 12;
  let pcm;
  let format = false;
  while (offset + 8 <= bytes.length) {
    const kind = bytes.toString('ascii', offset, offset + 4);
    const size = bytes.readUInt32LE(offset + 4);
    const start = offset + 8;
    insist(start + size <= bytes.length, 'truncated WAV chunk');
    if (kind === 'fmt ') {
      insist(size >= 16 && bytes.readUInt16LE(start) === 1
        && bytes.readUInt16LE(start + 2) === claimed.channels
        && bytes.readUInt32LE(start + 4) === claimed.sample_rate_hz
        && bytes.readUInt16LE(start + 14) === claimed.bits_per_sample
        && claimed.channels > 0 && claimed.channels <= 8
        && claimed.sample_rate_hz >= 8000 && claimed.sample_rate_hz <= 192000
        && [16, 24, 32].includes(claimed.bits_per_sample), 'WAV format differs from delivered Play receipt');
      format = true;
    }
    if (kind === 'data') { insist(!pcm && size > 0 && size % (claimed.channels * claimed.bits_per_sample / 8) === 0, 'invalid WAV samples'); pcm = bytes.subarray(start, start + size); }
    offset = start + size + (size % 2);
  }
  insist(format && pcm && offset === bytes.length, 'WAV lacks exact PCM');
  return hash(pcm);
}

function png(bytes) {
  insist(bytes.length > 24 && bytes.subarray(0, 8).equals(Buffer.from('89504e470d0a1a0a', 'hex'))
    && bytes.toString('ascii', 12, 16) === 'IHDR'
    && bytes.readUInt32BE(16) > 0 && bytes.readUInt32BE(20) > 0, 'invalid PNG capture');
}

function linkedIdentity(actual, expected, label) {
  for (const key of ['source_commit', 'run_id', 'body_id']) {
    insist(actual[key] === expected[key], `${label} ${key} drift`);
  }
}

/** Documentary correlation only. The producer must prove that its own receipts came from live actions. */
export function validateTodoJourney(root, publicationCommit, { checkAncestry = true } = {}) {
  insist(COMMIT.test(publicationCommit), 'publication source is not an exact commit');
  insist(existsSync(root) && lstatSync(root).isDirectory() && !lstatSync(root).isSymbolicLink(), 'missing regular evidence root');
  const manifestBytes = readFileSync(path.join(root, 'manifest.json'));
  const manifest = json(manifestBytes, 'manifest');
  insist(manifest.schema === 'conduit.evidence-manifest/v1' && manifest.result === 'complete'
    && manifest.proof_id === 'journey-todo-one-body' && manifest.suite_id === 'journey-gallery'
    && COMMIT.test(manifest.git_commit), 'missing complete exact-source Todo manifest');
  if (checkAncestry) {
    try { execFileSync('git', ['merge-base', '--is-ancestor', manifest.git_commit, publicationCommit]); }
    catch { throw new Error('Todo journey: capture commit is not in the publication source ancestry'); }
  }
  insist(Array.isArray(manifest.outputs) && manifest.outputs.length > 0 && manifest.outputs.length <= MAX_OUTPUTS, 'output count exceeds bound');
  const outputs = new Map();
  const paths = new Set(['manifest.json']);
  let total = 0;
  for (const output of manifest.outputs) {
    insist(nonempty(output.id) && !outputs.has(output.id) && output.required === true
      && output.scenario_id && typeof output.kind === 'string' && typeof output.media_type === 'string', 'invalid or duplicate declared output');
    const relative = safePath(output.path);
    insist(!paths.has(relative), 'duplicate declared path');
    const file = path.join(root, relative);
    insist(existsSync(file) && lstatSync(file).isFile() && !lstatSync(file).isSymbolicLink(), 'missing regular declared output');
    const bytes = readFileSync(file);
    insist(Number.isSafeInteger(output.bytes) && output.bytes === bytes.length && bytes.length <= MAX_FILE
      && SHA.test(output.sha256) && hash(bytes) === output.sha256, `digest or size mismatch for ${output.id}`);
    total += bytes.length;
    insist(total <= MAX_TOTAL, 'total evidence exceeds bound');
    outputs.set(output.id, { ...output, bytesValue: bytes });
    paths.add(relative);
  }
  insist(filesUnder(root).sort().join('\n') === [...paths].sort().join('\n'), 'undeclared or missing evidence file');
  const requireOutput = (id, kind) => {
    const output = outputs.get(id);
    insist(output && output.kind === kind, `missing ${kind} output ${id}`);
    insist(output.scenario_id === journey.run_id, `mixed run in ${id}`);
    return output;
  };
  const journeyOutput = outputs.get('journey');
  insist(journeyOutput?.kind === 'machine-readable-manifest', 'missing journey document');
  const journey = json(journeyOutput.bytesValue, 'journey');
  insist(journey.schema === 'conduit.journey/todo@1' && journey.source_commit === manifest.git_commit
    && nonempty(journey.run_id) && nonempty(journey.body_id)
    && journeyOutput.scenario_id === journey.run_id, 'journey identity mismatch');
  insist(Array.isArray(journey.chapters) && journey.chapters.map(chapter => chapter.id).join(',') === CHAPTERS.join(','), 'incomplete or unordered Todo sequence');
  const seenEvents = new Set();
  const seenMedia = new Set();
  const sources = new Set();
  const maskKinds = new Map();
  let lastTime = 0;
  for (const chapter of journey.chapters) {
    for (const field of ['title', 'intention', 'action', 'result', 'why', 'next']) insist(nonempty(chapter[field]), `${chapter.id} missing ${field}`);
    insist(Array.isArray(chapter.limitations) && chapter.limitations.length && chapter.limitations.every(nonempty), `${chapter.id} missing limits`);
    insist(Array.isArray(chapter.media) && chapter.media.length, `${chapter.id} has no captured medium`);
    const receipt = json(requireOutput(chapter.receipt_id, 'machine-readable-manifest').bytesValue, `${chapter.id} receipt`);
    linkedIdentity(receipt, journey, `${chapter.id} receipt`);
    insist(receipt.schema === 'conduit.todo-journey/chapter-receipt@1' && receipt.chapter_id === chapter.id
      && nonempty(receipt.event_id) && !seenEvents.has(receipt.event_id)
      && Number.isSafeInteger(receipt.observed_at_unix_ms) && receipt.observed_at_unix_ms > lastTime
      && nonempty(receipt.face_id) && Number.isSafeInteger(receipt.face_revision)
      && receipt.face_revision >= 0 && nonempty(receipt.show_id) && nonempty(receipt.source_receipt_id), `${chapter.id} receipt is incomplete`);
    lastTime = receipt.observed_at_unix_ms;
    seenEvents.add(receipt.event_id);
    const source = json(requireOutput(receipt.source_receipt_id, 'machine-readable-manifest').bytesValue, `${chapter.id} source`);
    linkedIdentity(source, journey, `${chapter.id} source`);
    insist(source.schema === 'conduit.todo-journey/producer-event@1' && source.event_id === receipt.event_id
      && source.chapter_id === chapter.id && source.face_id === receipt.face_id
      && source.face_revision === receipt.face_revision && source.show_id === receipt.show_id
      && source.observed_at_unix_ms === receipt.observed_at_unix_ms, `${chapter.id} producer event drift`);
    sources.add(receipt.source_receipt_id);
    if (['add', 'complete'].includes(chapter.id)) {
      insist(nonempty(receipt.interaction_id) && nonempty(receipt.action_id)
        && receipt.interaction_id === source.interaction_id && receipt.action_id === source.action_id
        && nonempty(source.mask_kind) && source.mask_kind === receipt.mask_kind,
      `${chapter.id} lacks matching typed Face action`);
      maskKinds.set(chapter.id, source.mask_kind);
    }
    if (chapter.id === 'read') {
      insist(receipt.reader_command === 'read-current-items'
        && source.reader_command === receipt.reader_command
        && nonempty(receipt.mask_play_id) && source.mask_play_id === receipt.mask_play_id
        && receipt.mask_kind === 'direct-spoken' && source.mask_kind === receipt.mask_kind
        && receipt.interaction_id === undefined && source.interaction_id === undefined
        && receipt.action_id === undefined && source.action_id === undefined,
      'read detail must be a correlated Mask-local command, not an invented Face action');
    }
    if (['add', 'complete'].includes(chapter.id)) {
      insist(Number.isSafeInteger(receipt.queue_sequence) && receipt.queue_sequence >= 0
        && receipt.queue_sequence === source.queue_sequence && nonempty(receipt.child_sign_id)
        && receipt.child_sign_id === source.child_sign_id && source.outcome === 'produced',
      `${chapter.id} has staging but no correlated produced result`);
    }
    if (chapter.id === 'recover') {
      insist(nonempty(source.previous_boot_id) && nonempty(source.new_boot_id)
        && source.previous_boot_id !== source.new_boot_id
        && SHA.test(source.pre_lull_state_sha256)
        && source.pre_lull_state_sha256 === source.recovered_state_sha256
        && receipt.previous_boot_id === source.previous_boot_id
        && receipt.new_boot_id === source.new_boot_id
        && receipt.recovered_state_sha256 === source.recovered_state_sha256,
      'recovery lacks same-Body state across a new Boot');
    }
    for (const media of chapter.media) {
      insist(nonempty(media.output_id) && !seenMedia.has(media.output_id) && nonempty(media.alt), 'duplicate or unlabelled media');
      seenMedia.add(media.output_id);
      const output = outputs.get(media.output_id);
      insist(output && ['screenshot', 'console-transcript', 'audio'].includes(output.kind), `missing media ${media.output_id}`);
      requireOutput(media.output_id, output.kind);
      const capture = json(requireOutput(media.receipt_id, 'machine-readable-manifest').bytesValue, 'capture receipt');
      linkedIdentity(capture, journey, 'capture receipt');
      insist(capture.schema === 'conduit.todo-journey/capture-receipt@1'
        && capture.chapter_id === chapter.id && capture.event_id === receipt.event_id
        && capture.face_id === receipt.face_id && capture.face_revision === receipt.face_revision
        && capture.show_id === receipt.show_id && capture.source_receipt_id === receipt.source_receipt_id
        && capture.media_output_id === media.output_id && capture.media_sha256 === output.sha256,
      `capture does not belong to ${chapter.id}`);
      insist(Array.isArray(source.media) && source.media.some(item => item.media_output_id === media.output_id
        && item.capture_source === capture.capture_source && item.media_sha256 === output.sha256),
      `producer did not attest ${media.output_id} capture`);
      if (output.kind === 'screenshot') {
        insist(output.media_type === 'image/png' && ['chromium', 'qmp', 'native'].includes(capture.capture_source), 'unsupported screenshot provenance');
        png(output.bytesValue);
        sources.add(capture.capture_source);
      } else if (output.kind === 'console-transcript') {
        insist(output.media_type === 'text/plain; charset=utf-8' && capture.capture_source === 'terminal', 'unsupported terminal provenance');
        sources.add('terminal');
      } else {
        insist(output.media_type === 'audio/wav' && ['speaker-play', 'qemu-audio'].includes(capture.capture_source)
          && ['direct', 'model-assisted'].includes(capture.speech_mode)
          && nonempty(capture.play_id) && nonempty(capture.plan_id) && nonempty(capture.voice_id)
          && SHA.test(capture.delivered_pcm_sha256) && wavPcm(output.bytesValue, capture) === capture.delivered_pcm_sha256,
        'audio differs from claimed delivered Play PCM');
        insist(source.play_id === capture.play_id && source.plan_id === capture.plan_id
          && source.show_id === capture.show_id && source.delivered_pcm_sha256 === capture.delivered_pcm_sha256
          && source.channels === capture.channels && source.sample_rate_hz === capture.sample_rate_hz
          && source.bits_per_sample === capture.bits_per_sample, 'audio lacks same-Play delivery receipt');
        if (capture.capture_source === 'qemu-audio') {
          insist(nonempty(capture.qemu_boot_id) && source.qemu_boot_id === capture.qemu_boot_id
            && nonempty(capture.qemu_output_id) && source.qemu_output_id === capture.qemu_output_id
            && source.qemu_audio_frames_captured > 0, 'guest audio lacks QEMU output provenance');
          const qemuOutput = outputs.get(capture.qemu_output_id);
          insist(qemuOutput && qemuOutput.scenario_id === journey.run_id
            && qemuOutput.sha256 === source.qemu_output_sha256, 'guest QEMU output is not retained');
          if (qemuOutput.media_type === 'application/octet-stream') {
            insist(qemuOutput.bytesValue.length > 0
              && qemuOutput.bytesValue.length % (capture.channels * capture.bits_per_sample / 8) === 0,
            'QEMU raw output has incomplete frames');
          }
          const qemuPcm = qemuOutput.media_type === 'audio/wav'
            ? wavPcm(qemuOutput.bytesValue, capture)
            : qemuOutput.media_type === 'application/octet-stream' ? hash(qemuOutput.bytesValue) : null;
          insist(qemuPcm === capture.delivered_pcm_sha256, 'published WAV differs from same-run QEMU PCM');
        } else insist(source.speaker_frames_committed > 0, 'speaker Play has no committed frames');
        const transcript = requireOutput(capture.transcript_output_id, 'document');
        insist(transcript.media_type === 'text/plain; charset=utf-8'
          && transcript.bytesValue.length > 0 && transcript.sha256 === capture.transcript_sha256
          && transcript.sha256 === source.spoken_text_sha256, 'audio has no matching spoken transcript');
        if (chapter.id === 'hear' || chapter.id === 'read') insist(capture.speech_mode === 'direct', 'screen-free chapter needs direct speech');
        sources.add(capture.capture_source);
      }
    }
  }
  for (const source of ['chromium', 'qmp', 'native', 'terminal', 'speaker-play']) {
    if (source === 'speaker-play' && sources.has('qemu-audio')) continue;
    insist(sources.has(source), `missing ${source} capture`);
  }
  insist(maskKinds.get('add') !== maskKinds.get('complete'), 'actions did not cross Masks');
  for (const id of ['hear', 'read']) insist(journey.chapters.find(chapter => chapter.id === id).media.some(media => outputs.get(media.output_id).kind === 'audio'), `${id} has no same-Play audio`);
  const terminal = json(requireOutput(journey.producer_terminal_receipt_id, 'machine-readable-manifest').bytesValue, 'producer terminal');
  linkedIdentity(terminal, journey, 'producer terminal');
  insist(terminal.schema === 'conduit.todo-journey/producer-terminal@1'
    && terminal.capture_entrance === 'cargo xtask prove todo-journey'
    && terminal.outcome === 'completed'
    && Array.isArray(terminal.event_ids) && terminal.event_ids.join(',') === [...seenEvents].join(',')
    && Array.isArray(terminal.media_output_ids) && terminal.media_output_ids.join(',') === [...seenMedia].join(','),
  'trusted capture terminal receipt is missing or incomplete');
  return { manifest, manifestSha256: hash(manifestBytes), journey, outputs };
}

export function renderTodoJourney(root, destination, publicationCommit, styles, navigation, options = {}) {
  const { manifest, manifestSha256, journey, outputs } = validateTodoJourney(root, publicationCommit, options);
  insist(!existsSync(destination), 'destination already exists');
  const staging = `${destination}.staging-${process.pid}`;
  insist(!existsSync(staging), 'Todo staging path already exists');
  mkdirSync(staging, { recursive: true });
  try {
    for (const output of outputs.values()) {
      const target = path.join(staging, output.path);
      mkdirSync(path.dirname(target), { recursive: true });
      cpSync(path.join(root, output.path), target, { errorOnExist: true });
      insist(hash(readFileSync(target)) === output.sha256, 'copied evidence changed');
    }
    cpSync(path.join(root, 'manifest.json'), path.join(staging, 'manifest.json'), { errorOnExist: true });
    insist(hash(readFileSync(path.join(staging, 'manifest.json'))) === manifestSha256, 'copied manifest changed');
    validateTodoJourney(staging, publicationCommit, options);
  const cards = journey.chapters.map((chapter, index) => {
    const media = chapter.media.map(item => {
      const output = outputs.get(item.output_id);
      const href = escape(output.path);
      const receipt = escape(outputs.get(item.receipt_id).path);
      let figure;
      if (output.kind === 'screenshot') figure = `<a href="${href}"><img src="${href}" alt="${escape(item.alt)}" loading="lazy"></a>`;
      else if (output.kind === 'console-transcript') figure = `<pre>${escape(output.bytesValue.toString('utf8').slice(0, 8000))}</pre><a href="${href}">Complete terminal capture</a>`;
      else {
        const capture = json(outputs.get(item.receipt_id).bytesValue, 'capture receipt');
        const transcript = outputs.get(capture.transcript_output_id);
        figure = `<audio controls preload="none" src="${href}"><a href="${href}">Download recorded Play</a></audio><p><strong>Spoken words:</strong> ${escape(transcript.bytesValue.toString('utf8'))}</p><p><a href="${escape(transcript.path)}">Exact transcript</a></p>`;
      }
      return `<figure>${figure}<figcaption>${escape(item.alt)} · <a href="${receipt}">Capture receipt</a></figcaption></figure>`;
    }).join('');
    return `<article id="${escape(chapter.id)}"><p class="eyebrow">Step ${index + 1} of ${CHAPTERS.length}</p><h2>${escape(chapter.title)}</h2><p><strong>You want to:</strong> ${escape(chapter.intention)}</p><p><strong>You do:</strong> ${escape(chapter.action)}</p><p><strong>What changes:</strong> ${escape(chapter.result)}</p><p><strong>Why it matters:</strong> ${escape(chapter.why)}</p><div class="todo-media">${media}</div><p><strong>Next:</strong> ${escape(chapter.next)}</p><details><summary>Exact evidence and limits</summary><p><a href="${escape(outputs.get(chapter.receipt_id).path)}">Chapter event receipt</a></p><ul>${chapter.limitations.map(note => `<li>${escape(note)}</li>`).join('')}</ul></details></article>`;
  }).join('');
  const toc = journey.chapters.map((chapter, index) => `<li><a href="#${escape(chapter.id)}">${index + 1}. ${escape(chapter.title)}</a></li>`).join('');
  const css = `.todo-journey{max-width:78rem;margin:auto;padding:clamp(1rem,4vw,4rem)}.todo-journey h1{font-size:clamp(2.5rem,6vw,4.7rem);line-height:1.1}.todo-journey article{border-top:1px solid var(--conduit-structure-secondary);padding:2rem 0}.todo-media{display:grid;grid-template-columns:repeat(auto-fit,minmax(min(100%,25rem),1fr));gap:1rem}.todo-media figure{margin:0;padding:1rem;background:var(--conduit-surface);border:1px solid var(--conduit-structure-secondary);border-radius:.5rem;min-width:0}.todo-media img{display:block;width:100%;height:auto}.todo-media audio{width:100%}.todo-media pre{overflow:auto;max-height:20rem}.todo-media figcaption{color:var(--conduit-text-secondary)}.todo-journey details{overflow-wrap:anywhere}`;
  const html = `<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Keep one Todo Body across Masks — Conduit</title><style>${styles}\n${css}</style></head><body data-application-theme="conduit.presentation/phosphor@1">${navigation}<main class="todo-journey"><p><a href="../../">All journeys</a></p><p class="eyebrow">One retained Body · documented user actions</p><h1>Keep one Todo list with you</h1><p class="lede">Birth a list, add and complete an item through different Masks, ask aloud what remains, and return to the same Body.</p><p>The retained producer attributes each screen and recording to this run. The page checks their relationships and digests; acceptance of the live capture command is separate. Ordinary controls lead; exact identities and limits sit behind each step.</p><nav aria-label="Journey steps"><ol>${toc}</ol></nav>${cards}<details><summary>Source and complete capture inventory</summary><p>Source <code>${escape(manifest.git_commit)}</code> · Run <code>${escape(journey.run_id)}</code> · Body <code>${escape(journey.body_id)}</code></p><p><a href="journey.json">Journey source</a> · <a href="${escape(outputs.get(journey.producer_terminal_receipt_id).path)}">Producer terminal receipt</a> · <a href="manifest.json">Digest-bound output inventory</a></p></details></main></body></html>`;
    writeFileSync(path.join(staging, 'index.html'), html);
    renameSync(staging, destination);
  } catch (error) {
    rmSync(staging, { recursive: true, force: true });
    throw error;
  }
  return { sourceCommit: manifest.git_commit, runId: journey.run_id, bodyId: journey.body_id };
}
