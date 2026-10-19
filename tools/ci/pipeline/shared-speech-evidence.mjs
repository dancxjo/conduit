// Publication verifies retained bytes; Rust retained-carrier replay proves semantics.
import { lstatSync, readFileSync, mkdirSync, writeFileSync, existsSync } from 'node:fs';
import { createHash } from 'node:crypto';
import path from 'node:path';
export const SHARED_SPEECH_EVIDENCE_ROOT = 'site/evidence/shared-speech-carrier';
const files = ['you-would.native', 'you-would.wav', 'you-would.json'];
const hex = /^[a-f0-9]{64}$/;
const fail = message => { throw new Error(`Shared speech evidence: ${message}`); };
const check = (ok, message) => { if (!ok) fail(message); };
const keys = (object, expected) => object && Object.keys(object).sort().join() === [...expected].sort().join();
const escape = value => String(value).replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;').replaceAll('"', '&quot;');
function regular(file, limit) {
  const stat = lstatSync(file);
  check(stat.isFile() && !stat.isSymbolicLink() && stat.size > 0 && stat.size <= limit, `nonregular or oversized file ${file}`);
  return readFileSync(file);
}
function directory(file) { const stat = lstatSync(file); check(stat.isDirectory() && !stat.isSymbolicLink(), `nonregular directory ${file}`); }
export function verifySharedSpeechEvidence(sourceTree = '.') {
  const root = path.join(sourceTree, SHARED_SPEECH_EVIDENCE_ROOT);
  try { lstatSync(root); } catch (error) { if (error.code === 'ENOENT') return null; throw error; }
  for (const relative of ['site', 'site/evidence', SHARED_SPEECH_EVIDENCE_ROOT]) directory(path.join(sourceTree, relative));
  const manifestBytes = regular(path.join(root, 'manifest.json'), 16384);
  const manifest = JSON.parse(manifestBytes);
  check(keys(manifest, ['schema', 'producer_code_commit', 'command', 'files', 'proof', 'human_listening', 'device_playback', 'trained_weights']), 'manifest fields');
  check(manifest.schema === 'conduit.speech/shared-carrier-evidence@1' && /^[a-f0-9]{40}$/.test(manifest.producer_code_commit), 'manifest schema or producer');
  check(typeof manifest.command === 'string' && manifest.command.startsWith('cargo +stable xtask prove journey native-speech --output '), 'producer command');
  check(typeof manifest.proof === 'string' && manifest.proof.length > 0 && manifest.proof.length <= 1024, 'proof description');
  check(manifest.human_listening === false && manifest.device_playback === false && manifest.trained_weights === false, 'proof boundary');
  check(keys(manifest.files, files), 'manifest file inventory');
  const bytes = { 'manifest.json': manifestBytes };
  for (const name of files) {
    const entry = manifest.files[name];
    check(keys(entry, ['bytes', 'sha256']) && Number.isSafeInteger(entry.bytes) && entry.bytes > 0 && hex.test(entry.sha256), `file manifest ${name}`);
    const data = regular(path.join(root, name), name.endsWith('.native') ? 2_000_000 : 256_000);
    check(data.length === entry.bytes && createHash('sha256').update(data).digest('hex') === entry.sha256, `bytes/hash ${name}`);
    bytes[name] = data;
  }
  const report = JSON.parse(bytes['you-would.json']);
  for (const name of ['source_identity', 'projection_identity', 'original_artifact_identity', 'compiled_dsp_source', 'speech_authoring_source', 'wav_identity', 'correspondence_subset_identity']) check(hex.test(report[name]), `report identity ${name}`);
  check(report.correspondence_subset_identity !== report.source_identity, 'complete/subset source distinction');
  const model = report.model_terminal;
  check(model?.source_identity === report.source_identity && hex.test(model.terminal_evidence_identity) && model.refusal === 'NoAdmittedAcousticBack' && model.selected_backend === null && model.trained_weights === false, 'model/source consistency');
  check(JSON.stringify(model.retained_without_conditioning) === JSON.stringify(['TextAndAlignment', 'SyllableAndWordStructure', 'ProsodyAndAcoustics', 'LanguageCommitment']), 'model conditioning boundary');
  check(JSON.stringify(report.formant_retained_without_realization) === JSON.stringify(['SeparateSyllableStructure', 'ScopedFrequencyEnergyAndRate', 'LanguageCommitment']), 'formant realization boundary');
  check(report.device_playback === 'not performed' && report.human_listening === 'not performed' && report.text === 'you would', 'report proof boundary');
  check(Number.isSafeInteger(report.pcm_frames) && report.pcm_frames > 0 && report.sample_rate_hz === 8000, 'PCM report geometry');
  let bound = 0;
  function inspect(value) {
    if (!value || typeof value !== 'object') return;
    if (Array.isArray(value.configuration)) {
      for (const entry of value.configuration) {
        if (entry.key === 'source' || entry.key === 'projection') {
          const expected = report[`${entry.key}_identity`];
          const actual = entry.value?.Text ?? entry.default_value?.Text;
          check(actual === expected, `plan ${entry.key} binding`);
          if (entry.rule?.TextOneOf) check(keys(entry.rule.TextOneOf, ['values']) && entry.rule.TextOneOf.values.length === 1 && entry.rule.TextOneOf.values[0] === expected, `plan ${entry.key} rule`);
          bound++;
        }
      }
    }
    for (const child of Object.values(value)) inspect(child);
  }
  inspect(report.plan);
  check(bound >= 4 && hex.test(report.plan?.source_document_id) && hex.test(report.plan?.checked_plot_id) && hex.test(report.plan?.expanded_plot_id), 'retained plan bindings');
  const wav = bytes['you-would.wav'];
  check(wav.length >= 44 && wav.toString('ascii', 0, 4) === 'RIFF' && wav.readUInt32LE(4) === wav.length - 8 && wav.toString('ascii', 8, 16) === 'WAVEfmt ' && wav.readUInt32LE(16) === 16 && wav.readUInt16LE(20) === 1 && wav.readUInt16LE(22) === 1 && wav.readUInt32LE(24) === report.sample_rate_hz && wav.readUInt32LE(28) === report.sample_rate_hz * 2 && wav.readUInt16LE(32) === 2 && wav.readUInt16LE(34) === 16 && wav.toString('ascii', 36, 40) === 'data' && wav.readUInt32LE(40) === report.pcm_frames * 2 && wav.length === 44 + report.pcm_frames * 2, 'WAV geometry');
  return { root, manifest, report, bytes };
}
export function stageSharedSpeechEvidence({ sourceTree = '.', output, sourceCommit }) {
  check(/^[a-f0-9]{40}$/.test(sourceCommit), 'publication source commit');
  const evidence = verifySharedSpeechEvidence(sourceTree);
  if (!evidence) return null;
  const destination = path.join(output, 'evidence/shared-speech-carrier');
  check(!existsSync(destination), 'destination already exists');
  mkdirSync(destination, { recursive: true });
  for (const [name, data] of Object.entries(evidence.bytes)) writeFileSync(path.join(destination, name), data, { flag: 'wx' });
  const { report, manifest } = evidence;
  writeFileSync(path.join(destination, 'publication.json'), JSON.stringify({ schema: 'conduit.speech/shared-carrier-publication@1', staged_source_commit: sourceCommit, producer_code_commit: manifest.producer_code_commit, files: manifest.files }, null, 2) + '\n', { flag: 'wx' });
  const source = `https://github.com/dancxjo/conduit/blob/${sourceCommit}`;
  const navigation = readFileSync(path.join(sourceTree, 'site/navigation.html'), 'utf8');
  const styles = readFileSync(path.join(sourceTree, 'targets/browser/host/assets/conduit.css'), 'utf8') + '\n' + readFileSync(path.join(sourceTree, 'site/chrome.css'), 'utf8');
  const html = `<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Shared speech carrier: you would</title><style>${styles}\n.site-content.speech-proof { max-width: 75ch; }\n.speech-proof code { overflow-wrap: anywhere; }\n.speech-proof audio { display: block; width: 100%; }</style></head><body data-application-look="conduit.look/phosphor@1">${navigation}<main class="site-content speech-proof"><h1>Shared speech carrier: “you would”</h1><p>Offline authored PCM from the installed factory and execution kernel. No physical-device playback, human listening validation or trained voice is claimed. The model path explicitly returns <code>NoAdmittedAcousticBack</code>; no acoustic model Back was selected.</p><audio controls preload="none" aria-label="Offline formant specimen: you would" src="you-would.wav">Download the WAV below.</audio><p>${report.pcm_frames} mono signed-16-bit frames at ${report.sample_rate_hz} Hz.</p><p>Producer code: <code>${manifest.producer_code_commit}</code>. Staged publication source: <code>${sourceCommit}</code>. These are distinct identities; publication does not claim the retained producer was rerun.</p><p>Original source: <code>${report.source_identity}</code><br>Projection: <code>${report.projection_identity}</code></p><ul>${[...Object.keys(evidence.bytes), 'publication.json'].map(name => `<li><a href="${name}">${escape(name)}</a></li>`).join('')}</ul><p><a href="${source}/docs/design/speech-acoustic-custody.md">Acoustic custody and runnable commands</a> · <a href="${source}/docs/audits/speech-acoustic-projection-addendum.md">Projection boundaries</a></p><p>Publication checks raw SHA-256, sizes, report bindings and WAV geometry. Native semantic re-admission and PCM/Plan/Sign replay belong to the retained-carrier Rust conformance test. This staging receipt is not an acceptance receipt.</p></main></body></html>`;
  writeFileSync(path.join(destination, 'index.html'), html, { flag: 'wx' });
  return { destination, producerCodeCommit: manifest.producer_code_commit, sourceCommit };
}
