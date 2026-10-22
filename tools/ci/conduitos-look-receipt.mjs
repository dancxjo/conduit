import { isDeepStrictEqual as same } from 'node:util';

const PREFIX = 'CONDUIT_NATIVE_COMPONENT_REALIZATION ';
const MATRIX = [
  ['creche-editorial-compact', 'compact', 'editorial', 100],
  ['creche-editorial-spacious', 'spacious', 'editorial', 125],
  ['creche-engineering-spacious', 'spacious', 'phosphor', 125],
  ['creche-engineering-compact', 'compact', 'phosphor', 100],
];
const nonempty = value => typeof value === 'string' && value.length > 0;
const hash = value => typeof value === 'string' && /^[0-9a-f]{64}$/.test(value);

// Verify custody of the already enacted local Look matrix. This does not
// independently execute Face validation or establish physical/human evidence.
export function validateNativeLookReceipt(proof, manifest, serialBytes) {
  const fail = () => { throw new Error('ConduitOS Look receipt differs from its enacted Face, Show or captured checkpoint'); };
  const bytes = Buffer.from(serialBytes);
  if (bytes.length > 16 * 1024 * 1024) fail();
  const records = (end, prefix) => {
    if (!Number.isSafeInteger(end) || end < 0 || end > bytes.length) fail();
    const lines = bytes.subarray(0, end).toString('utf8').split('\n');
    lines.pop(); // Never import a partial final record.
    return lines.filter(line => line.startsWith(prefix)).map(line => JSON.parse(line.slice(prefix.length)));
  };
  const look = proof?.look_selection;
  if (proof?.proof_class !== 'freestanding-emulator'
    || !/^[0-9a-f]{40}$/.test(proof.source_commit ?? '')
    || !hash(proof.image_sha256) || !nonempty(proof.boot_id) || !nonempty(proof.host_id)
    || !['build_id', 'image_id', 'profile_id'].every(key => nonempty(proof[key]))
    || manifest?.context?.source_commit !== proof.source_commit
    || manifest.context.image_sha256 !== proof.image_sha256
    || look?.schema !== 'conduit.conduitos/local-look-proof@1' || look.status !== 'passed'
    || look.input !== 'real-qmp-keyboard' || look.body_actions !== 0
    || look.physical_evidence !== false || look.human_enactment !== false
    || !Array.isArray(look.transitions) || look.transitions.length !== 4
    || !hash(look.same_recipe_canonical_sha256)
    || !Array.isArray(manifest.checkpoints)) fail();
  const actual = records(bytes.length, PREFIX);
  if (!same(actual, look.transitions)) fail();
  const selected = ['creche-ready', ...MATRIX.map(row => row[0])];
  const checkpoints = selected.map(name => {
    const matches = manifest.checkpoints.filter(entry => entry.checkpoint === name);
    if (matches.length !== 1) fail();
    return matches[0];
  });
  const indices = checkpoints.map(entry => manifest.checkpoints.indexOf(entry));
  if (indices.some((index, i) => i > 0 && index !== indices[i - 1] + 1)) fail();
  const decode = record => {
    const face = record?.face;
    const receipt = record?.show;
    const show = receipt?.manifestation;
    if (record?.schema !== 'conduit.native-face/v1' || !face || !receipt || !show
      || face.basis?.body_id !== null || !nonempty(face.identity)
      || !Number.isSafeInteger(face.revision) || face.revision < 1
      || receipt.face_id !== face.identity || receipt.face_revision !== face.revision
      || show.face_id !== face.identity || show.face_revision !== face.revision
      || show.lifecycle !== 'Available' || show.failure !== null
      || receipt.mask_plan_id !== show.plan_id || receipt.mask_active_play_id !== show.active_play_id
      || !receipt.mask_plot || !nonempty(receipt.show_id)
      || !Number.isSafeInteger(record.observation_sequence) || record.observation_sequence < 1
      || record.play_sequence !== show.play_sequence) fail();
    for (const field of ['face_id', 'face_revision', 'manifestation_id', 'host_id', 'boot_id', 'offer_generation', 'lifecycle']) {
      if (receipt[field] == null || receipt[field] !== show[field]) fail();
    }
    if (show.host_id !== proof.host_id || show.boot_id !== proof.boot_id
      || !nonempty(show.plan_id) || !nonempty(show.active_play_id) || !nonempty(show.manifestation_id)) fail();
    return { face, receipt, show };
  };
  const checkpointRecords = (entry, previousEnd) => {
    const end = entry.serial_byte_end;
    if (!Number.isSafeInteger(end) || end <= previousEnd || entry.health_refusal !== null || entry.unchanged !== false) fail();
    const boot = entry.guest_boot_record;
    if (boot?.boot_id !== proof.boot_id || boot.host_id !== proof.host_id
      || boot.build_id !== proof.build_id || boot.image_binding !== proof.image_id
      || boot.profile_id !== proof.profile_id) fail();
    const faces = records(end, 'CONDUIT_NATIVE_FACE ');
    const latest = faces.at(-1);
    const decoded = decode(latest);
    const compact = entry.native_receipts?.latest_acknowledged_face;
    if (!compact || compact.face_id !== decoded.face.identity || compact.face_revision !== decoded.face.revision
      || !same(compact.show, decoded.receipt) || !same(compact.basis, decoded.face.basis)) fail();
    return { ...decoded, faces, end };
  };
  const origin = checkpointRecords(checkpoints[0], -1);
  if (records(origin.end, 'CONDUIT_NATIVE_BIRTH ').length !== 0
    || records(origin.end, PREFIX).length !== 0
    || look.face_id !== origin.face.identity || look.face_revision !== origin.face.revision) fail();
  const actionCount = records(origin.end, 'CONDUIT_NATIVE_ACTION ').length;
  let previous = origin;
  const seen = new Set([origin.receipt.show_id]);
  for (const [index, [name, density, palette, scale]] of MATRIX.entries()) {
    const entry = checkpoints[index + 1];
    if (entry.expected_change !== true || entry.frame?.checkpoint !== name) fail();
    const fresh = checkpointRecords(entry, previous.end);
    const transition = actual[index];
    const expression = transition.expression?.expression;
    const configuration = transition.configuration;
    if (records(fresh.end, 'CONDUIT_NATIVE_BIRTH ').length !== 0
      || records(fresh.end, PREFIX).length !== index + 1
      || !same(records(fresh.end, PREFIX).at(-1), transition)
      || records(fresh.end, 'CONDUIT_NATIVE_ACTION ').length !== actionCount
      || !same(fresh.face, origin.face) || seen.has(fresh.receipt.show_id)
      || fresh.show.plan_id === previous.show.plan_id
      || fresh.show.offer_generation !== origin.show.offer_generation
      || transition.schema !== 'conduit.conduitos/component-realization@1'
      || transition.selection_entrance !== 'local-creche'
      || transition.local_scanout_acknowledged !== true || transition.owner_show_acknowledged !== false
      || transition.previous_show_id !== previous.receipt.show_id
      || transition.show_id !== fresh.receipt.show_id
      || transition.face_id !== origin.face.identity || transition.face_revision !== origin.face.revision
      || transition.host_id !== proof.host_id || transition.boot_id !== proof.boot_id
      || transition.offer_generation !== origin.show.offer_generation
      || transition.mask_plan_id !== fresh.show.plan_id || transition.active_play_id !== fresh.show.active_play_id
      || transition.manifestation_id !== fresh.show.manifestation_id
      || expression?.density !== density || expression.palette !== palette || expression.text_scale_percent !== scale
      || transition.fallback !== null || !nonempty(configuration?.root)
      || !same(configuration.component, look.same_checked_component)
      || !configuration.component || !['source_document_id', 'checked_plot_id', 'fore_id'].every(key => hash(configuration.component[key]))
      || configuration.recipe_canonical_sha256 !== look.same_recipe_canonical_sha256
      || !Number.isSafeInteger(transition.render?.commands) || transition.render.commands < 1
      || transition.render.commands > 48) fail();
    seen.add(fresh.receipt.show_id);
    previous = fresh;
  }
  if (previous.show.plan_id !== origin.show.plan_id) fail();
}
