import { createHash } from 'node:crypto';
import { execFileSync, spawnSync } from 'node:child_process';
import { lstatSync, readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';

export const TODO_BROWSER_DEVELOPMENT_ROOT = 'site/evidence/todo-browser-development';
const FILES = ['browser-after.png', 'browser-before.png', 'browser-card-after.png',
  'browser-full-after.png', 'index.html', 'long-list-browser-receipt.json',
  'long-list-requested-detail-playback.json', 'long-list-requested-detail-proof.json',
  'long-list-requested-detail-same-play.wav', 'long-list-requested-detail-turn.json',
  'long-list-spoken-proof.json', 'long-list-spoken-same-play.wav',
  'long-list-spoken-status.json', 'long-list-terminal-summary.json',
  'read-only-card-receipt.json', 'receipt.json',
  'todo-browser-after-native.png', 'todo-browser-after-native-receipt.json',
  'todo-20-card.png', 'todo-20-full.png', 'todo-native-acknowledged.png',
  'todo-native-acknowledged-provenance.json', 'todo-native-acknowledged-receipt.json',
  'todo-native-action-after.png', 'todo-native-action-before.png', 'todo-native-action-receipt.json',
  'todo-native-fork-label.json',
  'todo-native-read-only-receipt.json', 'todo-native-read-only.png',
  'todo-spoken-after-native-receipt.json', 'todo-spoken-after-native-same-play.wav',
  'todo-spoken-after-native-terminal.json'];
const sha = bytes => `sha256:${createHash('sha256').update(bytes).digest('hex')}`;
const commit = value => /^[a-f0-9]{40}$/.test(value ?? '');

// Keep failure context limited to repository identities; never dump the environment.
function ancestryContext(source, publicationCommit) {
  const inspect = args => {
    const result = spawnSync('git', args, { encoding: 'utf8' });
    return { status: result.status, output: result.stdout?.trim(),
      error: result.error?.message || result.stderr?.trim() };
  };
  return JSON.stringify({ cwd: process.cwd(),
    version: inspect(['--version']),
    checkout: inspect(['rev-parse', '--show-toplevel', '--absolute-git-dir',
      '--is-shallow-repository', 'HEAD']),
    parents: inspect(['rev-list', '--parents', '--no-walk', source, publicationCommit]),
  });
}

/** Admit a retained partial journey only with its exact, separate source labels. */
export function retainedTodoBrowserDevelopmentEvidence(root = TODO_BROWSER_DEVELOPMENT_ROOT,
  publicationCommit) {
  const entry = lstatSync(root);
  if (!entry.isDirectory() || entry.isSymbolicLink()
      || readdirSync(root).sort().join(',') !== FILES.sort().join(',')) {
    throw new Error('Todo browser development evidence has unexpected files');
  }
  const bytes = name => {
    const file = path.join(root, name);
    const stat = lstatSync(file);
    if (!stat.isFile() || stat.isSymbolicLink()) throw new Error(`Invalid Todo evidence file: ${name}`);
    return readFileSync(file);
  };
  const actionBytes = bytes('receipt.json');
  const action = JSON.parse(actionBytes);
  const card = JSON.parse(bytes('read-only-card-receipt.json'));
  const longList = JSON.parse(bytes('long-list-terminal-summary.json'));
  const longListBrowser = JSON.parse(bytes('long-list-browser-receipt.json'));
  const spoken = JSON.parse(bytes('long-list-spoken-proof.json'));
  const spokenStatus = JSON.parse(bytes('long-list-spoken-status.json'));
  const detail = JSON.parse(bytes('long-list-requested-detail-proof.json'));
  const detailPlay = JSON.parse(bytes('long-list-requested-detail-playback.json'));
  const detailTurn = JSON.parse(bytes('long-list-requested-detail-turn.json'));
  const native = JSON.parse(bytes('todo-native-read-only-receipt.json'));
  const nativeFork = JSON.parse(bytes('todo-native-fork-label.json'));
  const nativeAcknowledged = JSON.parse(bytes('todo-native-acknowledged-receipt.json'));
  const nativeAcknowledgedProvenance = JSON.parse(bytes('todo-native-acknowledged-provenance.json'));
  const nativeAction = JSON.parse(bytes('todo-native-action-receipt.json'));
  const browserAfterNative = JSON.parse(bytes('todo-browser-after-native-receipt.json'));
  const spokenAfterNative = JSON.parse(bytes('todo-spoken-after-native-receipt.json'));
  const spokenAfterNativeTerminal = JSON.parse(bytes('todo-spoken-after-native-terminal.json'));
  if (!commit(publicationCommit) || !commit(action.owner_source_commit)
      || !commit(action.browser_runtime_source_commit)
      || action.schema !== 'conduit.proof/todo-owner-browser@1'
      || card.schema !== 'conduit.proof/todo-owner-browser-read-only-card@1'
      || action.source_relation !== 'development-cross-source'
      || card.source_relation !== action.source_relation
      || action.owner_source_commit !== action.handbook_ui_source_commit
      || action.browser_runtime_source_commit === action.owner_source_commit
      || action.handbook_ui_source_clean !== true
      || card.owner_source_commit !== action.owner_source_commit
      || card.browser_runtime_source_commit !== action.browser_runtime_source_commit
      || card.handbook_ui_source_commit !== action.handbook_ui_source_commit
      || card.handbook_package_digest !== action.handbook_package_digest
      || card.body_id !== action.body_id || card.item_text !== action.item_text
      || !action.before?.show_id || !action.after?.show_id
      || action.before.show_id === action.after.show_id
      || Number(action.after.face_revision) <= Number(action.before.face_revision)
      || card.original_receipt_sha256 !== sha(actionBytes)) {
    throw new Error('Todo browser development receipts do not describe one truthful Add');
  }
  if (longList.schema !== 'conduit.proof/todo-long-list-terminal@1'
      || longListBrowser.schema !== 'conduit.proof/todo-owner-browser-long-list@1'
      || longList.body_id !== action.body_id || longListBrowser.body_id !== action.body_id
      || !commit(longList.owner_source_commit)
      || longListBrowser.terminal_actions_source_identity !== longList.owner_source_commit
      || !commit(longListBrowser.owner_source_identity)
      || longListBrowser.owner_source_identity !== longListBrowser.browser_source_identity
      || longListBrowser.source_relation !== 'exact-source'
      || longListBrowser.open !== 3 || longListBrowser.completed !== 17
      || longListBrowser.item_count !== 20 || longList.open !== 3
      || longList.completed !== 17 || longList.status !== '3 things left · 17 completed'
      || longList.action_count !== 19
      || !Array.isArray(longList.items) || longList.items.length !== 20
      || longList.items.filter(item => item.complete === true).length !== 17
      || longList.items.filter(item => item.complete === false).length !== 3
      || !longListBrowser.browser_face_id
      || Number(longListBrowser.browser_face_revision) <= Number(longList.face_revision)
      || !Array.isArray(longListBrowser.errors) || longListBrowser.errors.length !== 0) {
    throw new Error('Todo long-list evidence lacks one matching Body and exact browser rejoin');
  }
  const speechBatch = spokenStatus.speaker_playback?.batches?.[0];
  const wav = bytes('long-list-spoken-same-play.wav');
  if (spoken.body_id !== action.body_id || !commit(spoken.source_identity)
      || spoken.source_identity !== '801c83b46cb46296f0a198df2b0b7538bada09c3'
      || spoken.face_id !== spokenStatus.source_face_id
      || spoken.show_id !== spokenStatus.show_id
      || spoken.mask_plan_id !== spokenStatus.route_plan_id
      || spoken.mask_play_id !== spokenStatus.active_play_id
      || spoken.speaker_plan_id !== speechBatch?.plan_id
      || spoken.speaker_play_id !== speechBatch?.play_id
      || spoken.spoken_words !== spokenStatus.direct_opening_wording
      || spoken.spoken_words !== speechBatch.spoken_segments.join('')
      || spoken.speaker_outcome !== 'completed'
      || spokenStatus.direct_reading_complete !== true
      || spokenStatus.speaker_playback?.completed_batch_count !== 1
      || spokenStatus.speaker_playback?.source_show_id !== spoken.show_id
      || spoken.speaker_frames_committed !== speechBatch.speaker_frames_committed
      || spoken.speaker_wav_sha256 !== sha(wav).slice(7)
      || speechBatch.wav_sha256 !== spoken.speaker_wav_sha256
      || wav.toString('ascii', 0, 4) !== 'RIFF'
      || wav.toString('ascii', 8, 12) !== 'WAVE'
      || wav.readUInt32LE(24) !== 48000 || wav.readUInt16LE(22) !== 2
      || wav.readUInt16LE(34) !== 16 || wav.length !== speechBatch.wav_bytes) {
    throw new Error('Todo spoken evidence is not one completed selected-speaker Play');
  }
  const detailWav = bytes('long-list-requested-detail-same-play.wav');
  if (detail.body_id !== action.body_id || !commit(detail.release_source_identity)
      || detail.release_source_identity !== 'c49b89b58170762299cbceb36c4e52ea7d71eb9b'
      || detail.boot_id === spoken.boot_id || detail.face_id === spoken.face_id
      || detail.face_id !== detailPlay.face_id || detail.face_id !== detailTurn.face_id
      || detail.face_revision !== detailTurn.face_revision
      || detail.show_id !== detailPlay.source_show_id
      || detail.show_id !== detailTurn.source_show_id
      || detail.speech_plan_id !== detailPlay.plan_id
      || detail.speech_play_id !== detailPlay.play_id
      || detailPlay.outcome !== 'Completed' || detailTurn.outcome !== 'Completed'
      || detailTurn.completed_segments !== 3
      || detail.words.join(',') !== detailPlay.spoken_segments.map(segment => segment.text).join(',')
      || detailPlay.spoken_segments.at(-1)?.reason !== 'final-flush'
      || detail.speaker_frames_committed !== detailPlay.speaker_frames_committed
      || detail.same_play_wav_sha256 !== sha(detailWav).slice(7)
      || detail.same_play_wav_sha256 !== detailPlay.same_play_capture?.wav_sha256
      || detailWav.toString('ascii', 0, 4) !== 'RIFF'
      || detailWav.toString('ascii', 8, 12) !== 'WAVE'
      || detailWav.readUInt32LE(24) !== 48000 || detailWav.readUInt16LE(22) !== 2
      || detailWav.readUInt16LE(34) !== 16
      || detailWav.length !== detailPlay.same_play_capture?.wav_bytes) {
    throw new Error('Todo requested detail is not one completed same-Body speaker Play');
  }
  const nativePng = bytes('todo-native-read-only.png');
  const nativeImage = native.screenshots?.[0];
  if (native.schema !== 'conduit.conduitos/native-todo-face-proof@1'
      || native.proof_class !== 'live-local-qmp-installed-owner-read-only'
      || nativeFork.schema !== 'conduit.proof/todo-native-fork-label@1'
      || nativeFork.proof_class !== 'forked-state-copy-local-qmp'
      || native.guest_part?.body_id !== action.body_id
      || nativeFork.body_id !== action.body_id
      || !commit(native.source_commit) || !commit(nativeFork.harness_source_commit)
      || native.source_commit !== nativeFork.product_source_commit
      || native.owner_todo_face?.body_id !== action.body_id
      || native.owner_todo_face?.face_id !== native.face_shown?.face_id
      || native.owner_todo_face?.face_revision !== native.face_shown?.face_revision
      || native.owner_todo_face?.item_count !== 20
      || native.owner_todo_face?.status !== '3 things left · 17 completed'
      || native.face_shown?.status !== 'shown'
      || native.face_shown?.local_show_available !== true
      || native.face_shown?.owner_show_acknowledged !== false
      || native.face_shown?.interactions_admitted !== false
      || native.native_return_route_available !== false
      || native.interactions_admitted !== false || native.mutations !== 0
      || nativeFork.native_actions !== 0
      || nativeFork.owner_return_route_available !== false
      || nativeFork.original_checkpoint_hash_mismatches !== 0
      || native.qemu_alive_at_capture !== true
      || nativeImage?.png !== 'owner-standby.png'
      || nativeImage?.png_sha256 !== sha(nativePng).slice(7)
      || nativeImage?.png_bytes !== nativePng.length
      || nativeImage?.width !== 1280 || nativeImage?.height !== 800) {
    throw new Error('Todo native QMP evidence is not a matching read-only forked Face');
  }
  const acknowledgedPng = bytes('todo-native-acknowledged.png');
  const acknowledgedImage = nativeAcknowledged.screenshots?.find(image => image.checkpoint === 'owner-before');
  if (nativeAcknowledged.schema !== 'conduit.conduitos/native-todo-face-proof@1'
      || nativeAcknowledged.proof_class !== 'live-local-qmp-installed-owner-read-only'
      || nativeAcknowledgedProvenance.schema !== 'conduit.proof/todo-native-fork4-provenance@1'
      || nativeAcknowledgedProvenance.proof_class !== 'forked-state-copy-live-local-qmp-read-only'
      || nativeAcknowledged.source_commit !== nativeAcknowledgedProvenance.product_source_commit
      || nativeAcknowledged.guest_part?.body_id !== action.body_id
      || nativeAcknowledged.owner_todo_face?.body_id !== action.body_id
      || nativeAcknowledged.owner_todo_face?.face_id !== nativeAcknowledged.face_shown?.face_id
      || nativeAcknowledged.owner_todo_face?.face_revision !== nativeAcknowledged.face_shown?.face_revision
      || nativeAcknowledged.owner_todo_face?.item_count !== 20
      || nativeAcknowledged.owner_todo_face?.status !== '3 things left · 17 completed'
      || nativeAcknowledged.face_shown?.continuing_owner_route !== true
      || nativeAcknowledged.face_shown?.interactions_admitted !== true
      || nativeAcknowledged.show_ack?.status !== 'acknowledged'
      || nativeAcknowledged.show_ack?.show_id !== nativeAcknowledged.face_shown?.show_id
      || nativeAcknowledgedProvenance.acknowledged_show_id !== nativeAcknowledged.show_ack?.show_id
      || nativeAcknowledgedProvenance.owner_face_id !== nativeAcknowledged.owner_todo_face?.face_id
      || nativeAcknowledgedProvenance.guest_boot_id !== nativeAcknowledged.guest_part?.boot_id
      || nativeAcknowledgedProvenance.selected_checkpoint_root_isolated !== false
      || nativeAcknowledgedProvenance.checkpoint_snapshot_selected !== false
      || nativeAcknowledgedProvenance.original_checkpoint_hash_mismatches_after_proof !== 0
      || nativeAcknowledged.mutations !== 0 || nativeAcknowledgedProvenance.native_actions !== 0
      || nativeAcknowledged.qemu_alive_at_capture !== true
      || acknowledgedImage?.png_sha256 !== sha(acknowledgedPng).slice(7)
      || nativeAcknowledgedProvenance.qmp_screenshot_sha256 !== sha(acknowledgedPng)
      || acknowledgedImage?.png_bytes !== acknowledgedPng.length
      || acknowledgedImage?.width !== 1280 || acknowledgedImage?.height !== 800) {
    throw new Error('Todo acknowledged native Show evidence is not an exact-source read-only fork');
  }
  const nativeBefore = bytes('todo-native-action-before.png');
  const nativeAfter = bytes('todo-native-action-after.png');
  const beforeImage = nativeAction.screenshots?.find(image => image.checkpoint === 'owner-before');
  const afterImage = nativeAction.screenshots?.find(image => image.checkpoint === 'owner-after');
  if (nativeAction.schema !== 'conduit.conduitos/native-todo-action-proof@1'
      || nativeAction.proof_class !== 'live-local-qmp-installed-owner-isolated-checkpoint-fork'
      || !commit(nativeAction.source_commit)
      || nativeAction.guest_part?.body_id !== action.body_id
      || nativeAction.owner_face_before?.body_id !== action.body_id
      || nativeAction.owner_face_after?.body_id !== action.body_id
      || nativeAction.owner_face_before?.item_count !== 20
      || nativeAction.owner_face_after?.item_count !== 20
      || nativeAction.owner_face_before?.status !== '3 things left · 17 completed'
      || nativeAction.owner_face_after?.status !== '2 things left · 18 completed'
      || nativeAction.face_before?.face_id !== nativeAction.owner_face_before?.face_id
      || nativeAction.face_after?.face_id !== nativeAction.owner_face_after?.face_id
      || nativeAction.face_after?.face_revision <= nativeAction.face_before?.face_revision
      || nativeAction.show_ack_before?.show_id !== nativeAction.face_before?.show_id
      || nativeAction.show_ack_after?.show_id !== nativeAction.face_after?.show_id
      || nativeAction.show_ack_before?.status !== 'acknowledged'
      || nativeAction.show_ack_after?.status !== 'acknowledged'
      || nativeAction.action?.status !== 'accepted'
      || nativeAction.action?.code !== 'accepted'
      || nativeAction.action?.action_id !== 'todo.complete.task-18'
      || nativeAction.action?.face_id !== nativeAction.face_before?.face_id
      || nativeAction.action?.prior_show_id !== nativeAction.face_before?.show_id
      || nativeAction.native_mutations !== 1
      || nativeAction.selected_checkpoint_before?.files !== 38
      || nativeAction.selected_checkpoint_after?.files !== 39
      || nativeAction.selected_checkpoint_before?.sha256 === nativeAction.selected_checkpoint_after?.sha256
      || nativeAction.protected_checkpoint_before?.files !== 38
      || nativeAction.protected_checkpoint_before?.sha256 !== nativeAction.protected_checkpoint_after?.sha256
      || nativeAction.protected_checkpoint_root === nativeAction.selected_checkpoint_root
      || nativeAction.protected_checkpoint_dev_inode?.join(',') === nativeAction.selected_checkpoint_dev_inode?.join(',')
      || nativeAction.qemu_alive_at_capture !== true
      || beforeImage?.png_sha256 !== sha(nativeBefore).slice(7)
      || afterImage?.png_sha256 !== sha(nativeAfter).slice(7)
      || beforeImage?.png_bytes !== nativeBefore.length
      || afterImage?.png_bytes !== nativeAfter.length
      || beforeImage?.width !== 1280 || beforeImage?.height !== 800
      || afterImage?.width !== 1280 || afterImage?.height !== 800) {
    throw new Error('Todo native action evidence lacks one isolated accepted action and changed Show');
  }
  const browserAfterNativePng = bytes('todo-browser-after-native.png');
  if (browserAfterNative.schema !== 'conduit.proof/todo-browser-after-native@1'
      || browserAfterNative.proof_class !== 'read-only-pinned-chromium-rejoin-of-isolated-fork'
      || browserAfterNative.source_commit !== nativeAction.source_commit
      || browserAfterNative.browser_source_commit !== longListBrowser.browser_source_identity
      || browserAfterNative.body_id !== action.body_id
      || browserAfterNative.status !== nativeAction.owner_face_after?.status
      || browserAfterNative.owner_face_id !== browserAfterNative.face_id
      || String(browserAfterNative.owner_face_revision) !== browserAfterNative.face_revision
      || browserAfterNative.acknowledged_show_id !== browserAfterNative.show_id
      || browserAfterNative.show_state !== 'available'
      || browserAfterNative.screenshot !== 'browser-todo-after-native.png'
      || browserAfterNative.screenshot_bytes !== browserAfterNativePng.length
      || browserAfterNative.screenshot_sha256 !== sha(browserAfterNativePng).slice(7)
      || browserAfterNative.todo_actions !== 0
      || !Array.isArray(browserAfterNative.page_errors)
      || browserAfterNative.page_errors.length !== 0
      || browserAfterNativePng.length < 24) {
    throw new Error('Todo browser did not observe the changed native fork Face');
  }
  const afterNativeWav = bytes('todo-spoken-after-native-same-play.wav');
  const afterNativeBatch = spokenAfterNativeTerminal.speaker_playback?.batches?.[0];
  if (spokenAfterNative.schema !== 'conduit.proof/todo-direct-spoken-after-native@1'
      || spokenAfterNative.proof_class !== 'installed-owner-direct-mask-same-delivered-speaker-play-local'
      || spokenAfterNative.source_commit !== nativeAction.source_commit
      || spokenAfterNative.body_id !== action.body_id
      || spokenAfterNative.face_id !== spokenAfterNativeTerminal.source_face_id
      || spokenAfterNative.face_revision !== spokenAfterNativeTerminal.speaker_playback?.face_revision
      || spokenAfterNative.show_id !== spokenAfterNativeTerminal.show_id
      || spokenAfterNative.show_id !== spokenAfterNativeTerminal.speaker_playback?.source_show_id
      || spokenAfterNative.opening_wording !== spokenAfterNativeTerminal.direct_opening_wording
      || spokenAfterNative.opening_wording !== afterNativeBatch?.spoken_segments?.join('')
      || spokenAfterNative.outcome !== 'available'
      || spokenAfterNative.speaker_played !== true
      || spokenAfterNative.direct_reading_complete !== true
      || spokenAfterNativeTerminal.direct_reading_complete !== true
      || spokenAfterNativeTerminal.speaker_playback?.completed_batch_count !== 1
      || afterNativeBatch?.outcome !== 'completed'
      || spokenAfterNative.play_id !== afterNativeBatch?.play_id
      || spokenAfterNative.wav?.file !== afterNativeBatch?.wav_artifact_id
      || spokenAfterNative.wav?.bytes !== afterNativeWav.length
      || spokenAfterNative.wav?.bytes !== afterNativeBatch?.wav_bytes
      || spokenAfterNative.wav?.sha256 !== sha(afterNativeWav).slice(7)
      || spokenAfterNative.wav?.sha256 !== afterNativeBatch?.wav_sha256
      || spokenAfterNative.wav?.pcm_sha256 !== afterNativeBatch?.pcm_sha256
      || spokenAfterNative.wav?.pcm_bytes !== afterNativeWav.readUInt32LE(40)
      || spokenAfterNative.wav?.pcm_bytes !== afterNativeWav.length - 44
      || spokenAfterNative.wav?.pcm_sha256 !== sha(afterNativeWav.subarray(44)).slice(7)
      || spokenAfterNative.wav?.speaker_frames_committed !== afterNativeBatch?.speaker_frames_committed
      || spokenAfterNative.human_hearing_observed !== false
      || spokenAfterNative.todo_actions !== 0
      || afterNativeWav.toString('ascii', 0, 4) !== 'RIFF'
      || afterNativeWav.toString('ascii', 8, 12) !== 'WAVE'
      || afterNativeWav.readUInt32LE(24) !== 48000 || afterNativeWav.readUInt16LE(22) !== 2
      || afterNativeWav.readUInt16LE(34) !== 16) {
    throw new Error('Todo changed-list speech is not the delivered selected-speaker Play');
  }
  for (const source of [action.owner_source_commit, action.browser_runtime_source_commit,
    longList.owner_source_commit, longListBrowser.owner_source_identity, spoken.source_identity,
    detail.release_source_identity, native.source_commit, nativeFork.harness_source_commit,
    nativeAcknowledged.source_commit, nativeAction.source_commit,
    browserAfterNative.browser_source_commit]) {
    try { execFileSync('git', ['merge-base', '--is-ancestor', source, publicationCommit]); }
    catch (cause) {
      throw new Error(`Todo browser capture ancestry check failed: ${source} -> ${publicationCommit}; status=${cause.status ?? cause.code ?? "unknown"}, signal=${cause.signal ?? "none"}; ${cause.message}; context=${ancestryContext(source, publicationCommit)}`, { cause });
    }
  }
  for (const name of FILES.filter(file => file.endsWith('.png'))) {
    const image = bytes(name);
    if (image.length < 24 || !image.subarray(0, 8).equals(Buffer.from('89504e470d0a1a0a', 'hex'))
        || image.toString('ascii', 12, 16) !== 'IHDR') {
      throw new Error(`Todo browser capture is not a PNG: ${name}`);
    }
    if (card.screenshots?.[name] && card.screenshots[name] !== sha(image)) {
      throw new Error(`Todo browser card capture differs from its receipt: ${name}`);
    }
    if (longListBrowser.screenshots?.[name] && longListBrowser.screenshots[name] !== sha(image)) {
      throw new Error(`Todo browser long-list capture differs from its receipt: ${name}`);
    }
  }
  if (action.screenshots?.join(',') !== 'browser-before.png,browser-after.png'
      || !card.screenshots?.['browser-card-after.png']
      || !card.screenshots?.['browser-full-after.png']
      || !longListBrowser.screenshots?.['todo-20-card.png']
      || !longListBrowser.screenshots?.['todo-20-full.png']) {
    throw new Error('Todo browser receipts omit captured images');
  }
  const page = bytes('index.html').toString('utf8');
  if (!page.includes('cross-source development evidence')
      || !page.includes('proves one browser Add action')
      || !page.includes(action.owner_source_commit.slice(0, 9))
      || !page.includes(action.browser_runtime_source_commit.slice(0, 9))
      || !page.includes(longList.owner_source_commit.slice(0, 9))
      || !page.includes(longListBrowser.owner_source_identity.slice(0, 9))
      || !page.includes(spoken.source_identity.slice(0, 9))
      || !page.includes(detail.release_source_identity.slice(0, 9))
      || !page.includes(native.source_commit.slice(0, 9))
      || !page.includes(nativeAcknowledged.source_commit.slice(0, 9))
      || !page.includes(nativeAction.source_commit.slice(0, 9))
      || !page.includes(browserAfterNative.browser_source_commit.slice(0, 9))
      || !page.includes('This WAV is audio the Linux Owner Play delivered to its selected speaker')
      || !page.includes(nativeFork.harness_source_commit.slice(0, 9))
      || !page.includes('This WAV captures the audio delivered by the completed selected-speaker Play')) {
    throw new Error('Todo browser development page overclaims its capture');
  }
  for (const [, reference] of page.matchAll(/\b(?:href|src)="([^"]+)"/g)) {
    if (!reference.startsWith('/conduit/') && !reference.startsWith('https://')
        && !reference.startsWith('#')
        && !FILES.includes(reference)) {
      throw new Error(`Todo browser development page links an undeclared asset: ${reference}`);
    }
  }
  return { root, sourceCommit: action.owner_source_commit,
    browserRuntimeCommit: action.browser_runtime_source_commit, bodyId: action.body_id,
    longListActionsCommit: longList.owner_source_commit,
    longListBrowserCommit: longListBrowser.owner_source_identity,
    spokenCommit: spoken.source_identity, detailCommit: detail.release_source_identity,
    nativeCommit: native.source_commit, nativeHarnessCommit: nativeFork.harness_source_commit,
    nativeAcknowledgedCommit: nativeAcknowledged.source_commit,
    nativeActionCommit: nativeAction.source_commit,
    browserAfterNativeCommit: browserAfterNative.browser_source_commit };
}
