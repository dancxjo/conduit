import { createHash } from 'node:crypto';
import { lstatSync, readFileSync } from 'node:fs';
import path from 'node:path';

export const DIRECT_SPOKEN_DEVELOPMENT_ROOT = 'site/evidence/direct-spoken-development';
export const DIRECT_SPOKEN_DEVELOPMENT_PROOF = 'direct-spoken-face-development';
export const DIRECT_SPOKEN_DEVELOPMENT_SUITE = 'journey-gallery';

const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const sha = value => /^[a-f0-9]{64}$/.test(value ?? '');
const id = value => typeof value === 'string' && value.length > 0;

// The retained corpus is a separate installed-Linux run, never an audio chapter
// of the correlated three-host journey. Validate its semantic receipt before
// the generic evidence verifier checks every manifest byte and undeclared file.
export function retainedDirectSpokenDevelopmentEvidence(root = DIRECT_SPOKEN_DEVELOPMENT_ROOT) {
  let stat;
  try { stat = lstatSync(root); }
  catch (error) {
    if (error.code === 'ENOENT') return null;
    throw error;
  }
  if (!stat.isDirectory() || stat.isSymbolicLink()) throw new Error('Direct spoken evidence root must be a regular directory');
  const manifest = JSON.parse(readFileSync(path.join(root, 'manifest.json'), 'utf8'));
  const sourceCommit = manifest.git_commit;
  if (!/^[a-f0-9]{40}$/.test(sourceCommit)
      || manifest.schema !== 'conduit.evidence-manifest/v1'
      || manifest.result !== 'diagnostic-incomplete'
      || manifest.proof_id !== DIRECT_SPOKEN_DEVELOPMENT_PROOF
      || manifest.suite_id !== DIRECT_SPOKEN_DEVELOPMENT_SUITE) {
    throw new Error('Direct spoken evidence lacks an exact-source diagnostic manifest');
  }
  const outputs = new Map(manifest.outputs?.map(output => [output.path, output]));
  if (outputs.size !== manifest.outputs?.length || !outputs.has('index.html') || !outputs.has('report.json')) {
    throw new Error('Direct spoken evidence lacks a unique page and receipt');
  }
  const report = JSON.parse(readFileSync(path.join(root, 'report.json'), 'utf8'));
  if (report.schema !== 'conduit.evidence/direct-spoken-development@1'
      || report.disposition !== 'diagnostic-incomplete'
      || report.proof_class !== 'live-installed-linux-selected-alsa'
      || report.human_listening_observed !== false
      || report.capture_source_commit !== sourceCommit
      || !id(report.body_id) || !id(report.host_id) || !id(report.birth_boot_id)
      || !id(report.boot_id) || !id(report.face_id) || !id(report.face_revision)
      || !id(report.show_id) || !id(report.route_plan_id) || !id(report.operation_id)
      || report.reading_complete !== true || report.selected_speaker?.outcome !== 'completed'
      || !sha(report.selected_speaker?.provider_sha256)
      || !sha(report.selected_speaker?.correlation_sha256)
      || report.selected_speaker?.completed_segments !== 41
      || report.batches?.length !== 11) {
    throw new Error('Direct spoken receipt lacks exact completed-run identities');
  }
  const plays = new Set();
  const wavs = new Set();
  let pcmBytes = 0;
  let segmentCount = 0;
  for (const batch of report.batches) {
    if (!id(batch.play_id) || !id(batch.plan_id) || plays.has(batch.play_id)
        || !id(batch.stream_identity) || !sha(batch.source_segments_sha256)
        || !sha(batch.pcm_sha256) || !sha(batch.wav_sha256)
        || batch.provider_sha256 !== report.selected_speaker.provider_sha256
        || batch.outcome !== 'completed' || !Array.isArray(batch.spoken_segments)
        || batch.spoken_segments.length === 0 || batch.spoken_segments.some(text => !id(text))) {
      throw new Error('Direct spoken receipt contains a missing or duplicate completed Play');
    }
    plays.add(batch.play_id);
    if (wavs.has(batch.wav_artifact_id)) throw new Error('Direct spoken reading repeats a WAV across Plays');
    wavs.add(batch.wav_artifact_id);
    pcmBytes += batch.pcm_bytes;
    segmentCount += batch.spoken_segments.length;
    verifyWav(root, outputs, batch);
  }
  if (pcmBytes !== report.selected_speaker.produced_pcm_bytes
      || segmentCount !== report.selected_speaker.completed_segments) {
    throw new Error('Direct spoken reading does not account for every PCM byte and segment');
  }
  if (plays.has(report.opening?.play_id) || report.opening?.show_id !== report.show_id
      || !id(report.opening?.spoken_text) || !id(report.opening?.plan_id)
      || !id(report.opening?.completion_sign_id)) {
    throw new Error('Direct spoken opening lacks a distinct correlated Play');
  }
  verifyWav(root, outputs, report.opening);
  if (wavs.has(report.opening.wav_artifact_id)) throw new Error('Direct spoken opening repeats a reading WAV');
  wavs.add(report.opening.wav_artifact_id);
  if (outputs.size !== 14) throw new Error('Direct spoken corpus must declare its page, receipt, and 12 WAVs');
  if ([...outputs.keys()].some(name => !['index.html', 'report.json'].includes(name) && !wavs.has(name))) {
    throw new Error('Direct spoken corpus declares an unrelated output');
  }
  const page = readFileSync(path.join(root, 'index.html'), 'utf8');
  if (!page.includes('not an audio chapter of the three-host Journey')
      || !page.includes('not attended human hearing')) {
    throw new Error('Direct spoken page overclaims the separate local proof');
  }
  for (const [, reference] of page.matchAll(/\b(?:href|src)="([^"]+)"/g)) {
    if (reference !== 'manifest.json' && !reference.startsWith('https://') && !reference.startsWith('#')
        && !new Set(['/conduit/', '/conduit/journeys/', '/conduit/handbook/',
          '/conduit/#get-conduit', '/conduit/workspace/', '/conduit/site.css',
          '../one-body-five-masks/']).has(reference)
        && !outputs.has(reference)) {
      throw new Error(`Direct spoken page links an undeclared asset: ${reference}`);
    }
  }
  return { root, sourceCommit };
}

function verifyWav(root, outputs, receipt) {
  const name = receipt?.wav_artifact_id;
  if (!/^play-[a-f0-9]{64}\.wav$/.test(name ?? '') || !outputs.has(name)
      || outputs.get(name)?.sha256 !== receipt.wav_sha256
      || outputs.get(name)?.bytes !== receipt.wav_bytes) {
    throw new Error('Direct spoken WAV is not declared with its exact Play receipt');
  }
  const bytes = readFileSync(path.join(root, name));
  if (bytes.length !== receipt.wav_bytes || digest(bytes) !== receipt.wav_sha256
      || bytes.toString('ascii', 0, 4) !== 'RIFF' || bytes.toString('ascii', 8, 12) !== 'WAVE'
      || bytes.length - 44 !== receipt.pcm_bytes
      || digest(bytes.subarray(44)) !== receipt.pcm_sha256) {
    throw new Error(`Direct spoken WAV differs from the same-Play PCM: ${name}`);
  }
}
