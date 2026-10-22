#!/usr/bin/env python3
"""Independent documentary verification, not guest execution or signature authentication.
Usage: /usr/bin/python3 work/6158-verify-selected-controls.py PACKET OUTPUT.json
Requires original local sealed packet (including canonical ELF/ISO), and Pillow.
No new selected-recipe packet has been verified merely by preparing this script.
"""
import hashlib
import json
import pathlib
import re
import sys
from PIL import Image


def require(ok, message):
    if not ok:
        raise ValueError(message)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def decode(data):
    def unique(pairs):
        out = {}
        for key, value in pairs:
            require(key not in out, 'duplicate JSON key: ' + key)
            out[key] = value
        return out
    return json.loads(data, object_pairs_hook=unique,
                      parse_constant=lambda x: (_ for _ in ()).throw(ValueError(x)))


def verify(packet):
    p = pathlib.Path(packet).resolve()

    def path(name):
        x = (p / name).resolve()
        require(x.is_relative_to(p) and x != p, 'escaped packet path')
        return x

    def load(name):
        return decode(path(name).read_bytes())

    seal = load('seal.json')
    require(seal['exit_code'] == 0 and seal['source_clean_after'] is True
            and seal['source_unchanged'] is True, 'not clean successful seal')
    for name, digest in seal['hashes'].items():
        require(re.fullmatch('[0-9a-f]{64}', digest) is not None, 'invalid seal hash')
        require(sha(path(name).read_bytes()) == digest, 'seal hash: ' + name)

    def sealed(name):
        require(name in seal['hashes'], 'unsealed input: ' + name)
        return path(name)

    for name in ['creche-controls-proof.json', 'journey-frames/manifest.json',
                 'journey-serial.log', 'live-images/x86_64-pc/build-manifest.json',
                 'live-images/x86_64-pc/resolved-image.json']:
        sealed(name)
    r = load('creche-controls-proof.json')
    fm = load('journey-frames/manifest.json')
    m = load('live-images/x86_64-pc/build-manifest.json')
    resolved = load('live-images/x86_64-pc/resolved-image.json')
    raw = path('journey-serial.log').read_bytes()
    require(len(raw) <= 16 * 1024 * 1024, 'serial bound')
    events = []
    offset = 0
    for line in raw.splitlines(keepends=True):
        offset += len(line)
        if not line.endswith(b'\n'):
            break
        for prefix in [b'CONDUIT_NATIVE_FACE ', b'CONDUIT_CRECHE_FOCUS ',
                       b'CONDUIT_BOOT_SIGN ', b'CONDUIT_PRODUCT_JOURNEY ',
                       b'CONDUIT_NATIVE_COMPONENT_REALIZATION ', b'CONDUIT_CRECHE_CHANGE ',
                       b'CONDUIT_NATIVE_ACTION ', b'CONDUIT_NATIVE_BIRTH ',
                       b'CONDUIT_NATIVE_POINTER ']:
            if line.startswith(prefix):
                require(len(line) <= 2 * 1024 * 1024, 'record bound')
                events.append((offset, prefix.decode().strip(), decode(line[len(prefix):])))
                break

    def rows(kind, end=len(raw)):
        return [(n, row) for n, prefix, row in events
                if prefix == 'CONDUIT_' + kind and n <= end]

    def occurs(kind, row):
        found = [n for n, x in rows(kind) if x == row]
        require(found, 'receipt absent from original serial: ' + kind)
        return found

    def latest(kind, end):
        found = rows(kind, end)
        require(found, 'missing current record: ' + kind)
        return found[-1][1]

    require(r['schema'] == 'conduit.conduitos/creche-controls-proof@1'
            and r['status'] == 'passed', 'not passed controls')
    require(r['source_commit'] == seal['source_commit'] == m['source_identity'], 'Source binding')
    require(m['schema'] == 'conduit.host/target-build-manifest@3'
            and m['artifact_role'] == 'product-host', 'canonical product role')
    base = 'live-images/x86_64-pc/'
    for key in ['kernel', 'image']:
        name = base + m[key]['file']
        require(sha(sealed(name).read_bytes()) == m[key]['sha256'], key + ' bytes')
    require(r['image_sha256'] == m['image']['sha256']
            and r['final_image_id'] == m['image_id'] == 'image:sha256:' + r['image_sha256'],
            'final ISO binding')
    # Original resolved file preserves Rust payload field ordering; reject floating values.
    payload = resolved['payload']
    require(not re.search(r'(?<![\w"])\d+\.\d+', json.dumps(payload)), 'float payload unsupported')
    binding = 'image:sha256:' + sha(json.dumps(payload, ensure_ascii=False,
                                             separators=(',', ':')).encode())
    require(binding == resolved['manifest']['image_id'] == m['resolved_description_binding']
            == r['logical_image_binding'], 'logical resolved payload binding')
    require(m['resolved_build'] == resolved['manifest'], 'resolved build manifest')
    require(r['canonical_build_metadata'] == {'target_build_manifest': m, 'resolved_image': resolved},
            'retained actual canonical metadata')
    bootrows = rows('BOOT_SIGN')
    require(len(bootrows) == 1, 'exactly one Boot')
    boot = bootrows[0][1]
    require(boot == r['boot_sign'] and boot['status'] == 'accepted', 'accepted Boot')
    for key in ['profile_id', 'build_id']:
        require(boot[key] == r[key] == m[key] == resolved['manifest'][key] == payload[key],
                key + ' binding')
    require(boot['image_binding'] == binding, 'Boot logical image binding')
    for key in ['host_id', 'boot_id']:
        require(boot[key] == r[key], 'proof ' + key)
    for _, world in rows('PRODUCT_JOURNEY'):
        require(world['status'] == 'world' and world['body_id'] is None, 'unexpected Body effect')
        for key in ['profile_id', 'build_id', 'host_id', 'boot_id', 'offer_generation']:
            require(world[key] == boot[key], 'world ' + key)
        require(world['image_id'] == binding, 'world logical image')

    faces = rows('NATIVE_FACE')
    require(faces, 'no acknowledged Faces')

    def native(row):
        face, show = row['face'], row['show']
        require(show['face_id'] == face['identity'] and show['face_revision'] == face['revision']
                and show['lifecycle'] == 'Available', 'Face/Show binding')
        man = show['manifestation']
        for key in ['host_id', 'boot_id', 'offer_generation']:
            require(show[key] == man[key] == boot[key], 'Show/manifestation ' + key)
        for key in ['face_id', 'face_revision', 'manifestation_id']:
            require(show[key] == man[key], 'manifestation ' + key)
        require(man['presenter_implementation_id'] == 'face/renderer-conduitos-component@1'
                and man['failure'] is None and man['lifecycle'] == 'Available'
                and man['body_id'] is None and man['wake_id'] is None,
                'available component manifestation')
        require(man['active_play_id'] == show['mask_active_play_id']
                and man['plan_id'] == show['mask_plan_id'], 'Mask Plan/Play')
        return row

    for _, row in faces:
        native(row)

    def focus(row, face):
        require(row['schema'] == 'conduit.creche-focus/v1', 'focus schema')
        for key, value in [('face_id', face['face']['identity']),
                           ('face_revision', face['face']['revision']),
                           ('show_id', face['show']['show_id'])]:
            require(row[key] == value, 'current focus ' + key)
        require(1 <= row['page'] <= row['pages'] <= 24, 'bounded focus page')
        occurs('CRECHE_FOCUS', row)

    arrival, initial = r['arrival_acknowledged_face'], r['initial_acknowledged_face']
    arrival_end = occurs('NATIVE_FACE', arrival)[-1]
    require(r['initial_world'] in [w for _, w in rows('PRODUCT_JOURNEY')], 'initial world serial')
    require(r['initial_world']['face_id'] == arrival['face']['identity']
            and r['initial_world']['manifestation_id'] == arrival['show']['manifestation_id'],
            'arrival world')
    chosen = r['selected_recipe']
    selections = rows('NATIVE_COMPONENT_REALIZATION')
    require(len(selections) == 2 and [x for _, x in selections] == chosen['transitions']
            == r['component_realization_records'], 'exact two original selections')
    require(chosen['schema'] == 'conduit.conduitos/chosen-creche-recipe@1'
            and chosen['status'] == 'passed' and chosen['source_effects'] == 0
            and chosen['physical_evidence'] is False, 'chosen receipt')
    previous = arrival
    seen = {arrival['show']['show_id']}
    config_identity = None
    for (end, selection), (density, scale, root, identity) in zip(selections, [
        ('spacious', 125, 'creche-mask-columns-spacious-phosphor', 'conduit.face/spacious-phosphor@1'),
        ('compact', 100, 'creche-mask-columns', 'conduit.face/compact@1')]):
        candidates = [(n, f) for n, f in faces if f['show']['show_id'] == selection['show_id']]
        require(len(candidates) == 1, 'unique selected full NativeFace')
        face_end, fresh = candidates[0]
        require(face_end > arrival_end and fresh['face'] == arrival['face'], 'selected Source constancy')
        # Realization may precede its matching NativeFace; require later matching focus before next selection.
        next_end = next((n for n, _ in selections if n > end), len(raw))
        matched_focus = [(n, f) for n, f in rows('CRECHE_FOCUS', next_end)
                         if n >= face_end and f['show_id'] == selection['show_id']]
        require(matched_focus, 'fresh current selection focus')
        focus(matched_focus[0][1], fresh)
        stop = matched_focus[0][0]
        require(latest('NATIVE_FACE', stop) == fresh, 'selection current NativeFace')
        for effect in ['CRECHE_CHANGE', 'NATIVE_ACTION', 'NATIVE_BIRTH']:
            require(len(rows(effect, stop)) == len(rows(effect, arrival_end)), 'selection owner effect')
        show, cfg = fresh['show'], selection['configuration']
        require(show['show_id'] not in seen, 'replayed selection Show')
        seen.add(show['show_id'])
        for key in ['observation_sequence', 'play_sequence']:
            require(fresh[key] > previous[key], 'selection ordering')
        for key in ['host_id', 'boot_id', 'offer_generation']:
            require(selection[key] == boot[key], 'selection foreign ' + key)
        expected = {'schema': 'conduit.conduitos/component-realization@1',
                    'selection_entrance': 'local-creche', 'previous_show_id': previous['show']['show_id'],
                    'show_id': show['show_id'], 'face_id': fresh['face']['identity'],
                    'face_revision': fresh['face']['revision'], 'mask_plan_id': show['mask_plan_id'],
                    'active_play_id': show['mask_active_play_id'], 'manifestation_id': show['manifestation_id'],
                    'fallback': None, 'owner_show_acknowledged': False, 'local_scanout_acknowledged': True}
        require(all(selection[k] == v for k, v in expected.items()), 'selection Show/config custody')
        require(show['mask_plan_id'] != previous['show']['mask_plan_id'], 'stale selection Plan')
        require(cfg['root'] == root and cfg['invocation_path'] == [root, 'component']
                and cfg['implementation_id'] == 'face/renderer-conduitos-component@1'
                and cfg['execution_profile_id'] == 'conduitos/bounded-mask@1'
                and cfg['placement_id'] == show['manifestation']['placement_id'], 'configuration route')
        require(all(isinstance(cfg['component'][k], str) and cfg['component'][k]
                    for k in ['source_document_id', 'checked_plot_id', 'fore_id']), 'component identity')
        require(cfg['component']['source_document_id'] == show['mask_plot']['source_document_id'], 'Mask Source')
        require(re.fullmatch('[0-9a-fA-F]{64}', cfg['recipe_canonical_sha256']) is not None
                and cfg['recipe_canonical_bytes'] > 0, 'recipe digest/length')
        ident = {k: cfg[k] for k in ['component', 'recipe_canonical_sha256', 'recipe_canonical_bytes']}
        require(config_identity is None or ident == config_identity, 'changed component/recipe')
        config_identity = ident
        require(selection['expression'] == {
            'accessibility_overrides': {'forced_colors': False, 'reduced_motion': True, 'text_scale': False},
            'expression': {'density': density, 'identity': identity, 'motion': 'none', 'palette': 'phosphor',
                           'text_scale_percent': scale, 'version': 1},
            'outcome': 'Selected', 'platform_forced_colors': False}, 'exact chosen expression')
        require(1 <= selection['render']['commands'] <= 48, 'bounded component render')
        previous = fresh
    require(previous == initial and chosen['arrival_show_id'] == arrival['show']['show_id']
            and chosen['selected_show_id'] == initial['show']['show_id']
            and chosen['selected_root'] == 'creche-mask-columns'
            and chosen['same_checked_component_and_recipe'] == config_identity, 'second chosen baseline')
    expected_controls = []
    require(r['source_action_inventory'] == initial['face']['actions'], 'actual Source action inventory')
    for action in initial['face']['actions']:
        if action['availability'] != 'Available':
            continue
        expected_controls.append([action['identity'], None])
        args = action['arguments']
        require(len(args) <= 1, 'unsupported multi-argument control')
        if args:
            contract = args[0]['contract']
            require(args[0]['name'] == 'value', 'unsupported argument name')
            if contract['value_kind'] == 'value/bool':
                require(contract == {'constraints': [], 'maximum_bytes': 1, 'value_kind': 'value/bool'},
                        'unsupported Boolean shape')
            else:
                require(contract['value_kind'] == 'value/text', 'unsupported control kind')
                expected_controls.append([action['identity'], 'value'])
    require(r['expected_controls'] == expected_controls, 'derived available controls')
    expected_set = {tuple(x) for x in expected_controls}
    for direction in ['forward_focus', 'reverse_focus']:
        cycle = r[direction]
        require(len(cycle) == len(expected_controls) + 1 and cycle[0] == cycle[-1]
                and {(x['action_id'], x['argument_name']) for x in cycle} == expected_set,
                'finite complete focus cycle: ' + direction)
        for row in cycle:
            focus(row, initial)

    checkpoints = fm['checkpoints']
    require(fm['status'] == 'complete' and fm['failure'] is None
            and r['captures'] == len(checkpoints) <= 27, 'complete bounded capture inventory')
    names = [c['checkpoint'] for c in checkpoints]
    require(len(names) == len(set(names)), 'duplicate capture')
    wanted = {'front-door-ready', 'creche-controls-ready', 'creche-controls-probe-name',
              'creche-naming-welsh', 'creche-naming-pointer-focused',
              'creche-naming-tradition-restored', 'creche-naming-exact-name-restored'}
    unsampled = {}
    for view in ['primary', 'details']:
        pages = r[view + '_pages']
        count = len(pages)
        require(1 <= count <= 24, 'bounded all-page receipt inventory')
        previous_end = 0
        for i, row in enumerate(pages, 1):
            require(row['page'] == i and row['pages'] == count, 'page gap/order')
            focus(row, initial)
            ends = [n for n in occurs('CRECHE_FOCUS', row) if n > previous_end]
            require(ends, 'page receipt serial order')
            previous_end = ends[0]
        sampled = {1, (count + 1) // 2, count}
        unsampled[view] = [i for i in range(1, count + 1) if i not in sampled]
        for i in sampled:
            name = f'creche-controls-{view}-{i}'
            wanted.add(name)
            matching = [c for c in checkpoints if c['checkpoint'] == name]
            require(len(matching) == 1 and latest('CRECHE_FOCUS', matching[0]['serial_byte_end']) == pages[i-1],
                    'sample PNG/current page receipt')
    require(set(names) == wanted, 'extra/missing sampled checkpoints')
    width, height = map(int, seal['requested_viewport'].split('x'))
    previous_end = 0
    pngs = []
    for c in checkpoints:
        end = c['serial_byte_end']
        require(previous_end <= end <= len(raw) and end > 0 and raw[end-1:end] == b'\n', 'prefix custody')
        previous_end = end
        require(c['guest_boot_record'] == boot, 'capture Boot')
        current = rows('NATIVE_FACE', end)
        if current:
            n = c['native_receipts']['latest_acknowledged_face']
            row = current[-1][1]
            require(n['face_id'] == row['face']['identity'] and n['face_revision'] == row['face']['revision']
                    and n['show'] == row['show'], 'capture current acknowledged Face/Show')
        frame = c['frame']
        name = 'journey-frames/' + frame['png']
        require(frame['png'] == c['checkpoint'] + '.png' and name not in pngs, 'PNG name inventory')
        pngs.append(name)
        data = sealed(name).read_bytes()
        require(len(data) == frame['png_bytes'] and sha(data) == frame['png_sha256'], 'original PNG bytes')
        with Image.open(path(name)) as image:
            rgba = image.convert('RGBA')
            require(rgba.size == (width, height) == (frame['width'], frame['height'])
                    and frame['pixel_format'] == 'RGBA8' and sha(rgba.tobytes()) == frame['pixel_sha256'],
                    'original PNG pixel custody')
    require({x.relative_to(p).as_posix() for x in path('journey-frames').glob('*.png')} == set(pngs),
            'unlisted/missing original PNG')

    changes = rows('CRECHE_CHANGE')
    require(len(changes) == 4, 'exactly four checked naming changes')
    actions = ['creche.name', 'creche.naming', 'creche.naming', 'creche.name']
    values = [r['probe_baseline_name'], list(b'welsh'), r['original_tradition'], r['probe_baseline_name']]
    require(r['probe_baseline_name'] == list(b'Naming proof'), 'fixed ASCII probe baseline')
    before = initial
    probe_face = None
    for (end, change), action_id, value in zip(changes, actions, values):
        require(change['disposition'] == 'source-accepted-and-shown', 'checked Change disposition')
        interaction = change['interaction']
        action = next(a for a in before['face']['actions'] if a['identity'] == action_id)
        contract = action['arguments'][0]['contract']
        require(action['availability'] == 'Available' and interaction['action_id'] == action_id
                and interaction['target'] == action['target'] and interaction['arguments'] == [
                    {'name': 'value', 'value_kind': contract['value_kind'], 'value': value}], 'typed checked action')
        for key, val in [('face_id', before['face']['identity']), ('revision', before['face']['revision']),
                         ('show_id', before['show']['show_id'])]:
            require(change['before_' + key] == val, 'Change before binding')
        for key, val in [('face_id', before['face']['identity']), ('face_revision', before['face']['revision']),
                         ('show_id', before['show']['show_id'])]:
            require(interaction[key] == val, 'interaction current Show')
        afters = [f for _, f in faces if f['face']['identity'] == change['after_face_id']
                  and f['face']['revision'] == change['after_revision']
                  and f['show']['show_id'] == change['after_show_id']]
        require(len(afters) == 1, 'unique checked Change after Face/Show')
        before = afters[0]
        if probe_face is None:
            probe_face = before['face']
    require({k: v for k, v in before['face'].items() if k not in ['identity', 'revision']}
            == {k: v for k, v in probe_face.items() if k not in ['identity', 'revision']},
            'exact probe draft restoration, including name/tradition/search/selection')
    require(before == r['final_acknowledged_face'] and r['checked_probe_name_change'] == changes[0][1]
            and r['restored_name_change'] == changes[-1][1], 'final/restored checked receipts')
    naming = next(a for a in initial['face']['actions'] if a['identity'] == 'creche.naming')
    membership = [x['CanonicalMembership'] for x in naming['arguments'][0]['contract']['constraints']
                  if 'CanonicalMembership' in x]
    require(len(membership) == 1 and membership[0]['negated'] is False
            and membership[0]['members'] == r['naming_membership']
            and list(b'welsh') in r['naming_membership']
            and r['original_tradition'] in r['naming_membership'], 'Source naming membership')
    ptr = r['pointer_focus']
    require(ptr['source_change_count'] == 0 and ptr['action'] == 'creche.naming'
            and ptr['argument'] == 'value', 'pointer focus-only receipt')
    target = ptr['target_geometry']
    pointer_face = next(f for _, f in faces if f['show']['show_id'] == target['show_id'])
    for key in ['target_geometry', 'before_press_focus', 'after_focus', 'final_focus']:
        focus(ptr[key], pointer_face)
    require(ptr['before_press_focus']['argument_name'] is None
            and all(ptr[k]['argument_name'] == 'value' for k in ['target_geometry', 'after_focus', 'final_focus']),
            'pointer changes focus from action to argument')
    ends = []
    for key in ['press', 'release', 'post_release_barrier']:
        row = ptr[key]
        require(row['coalesced'] == row['dropped'] == 0, 'pointer event loss')
        for field in ['face_id', 'face_revision', 'show_id']:
            require(row[field] == target[field], 'pointer current identity')
        ends.append(occurs('NATIVE_POINTER', row)[-1])
    require(ends[0] < ends[1] < ends[2] and ptr['press']['sequence'] < ptr['release']['sequence']
            < ptr['post_release_barrier']['sequence'], 'release dispatch barrier order')
    require(ptr['press']['press_edge'] is True and ptr['press']['primary_pressed'] is True
            and all(ptr[k]['press_edge'] is False and ptr[k]['primary_pressed'] is False
                    for k in ['release', 'post_release_barrier']), 'physical pointer edges')
    bounds = target['hit_bounds']
    require(bounds['x'] <= ptr['press']['display_x'] < bounds['x'] + bounds['width']
            and bounds['y'] <= ptr['press']['display_y'] < bounds['y'] + bounds['height'], 'actual hit bounds')
    require(not any(ends[0] <= n <= ends[2] for n, _ in changes), 'pointer dispatched Source effect')
    require(latest('NATIVE_FACE', ends[2]) == latest('NATIVE_FACE', ends[0]) == pointer_face,
            'release-side Source/Show mutation')
    require(r['full_6158_acceptance'] is False and r['physical_evidence'] is False
            and r['historical_renderer_reproduced'] is False, 'proof scope upgrade')
    return {'status': 'verified', 'source': seal['source_commit'], 'packet': str(p),
            'viewport': seal['requested_viewport'], 'original_files_rehashed': len(seal['hashes']),
            'png_names': pngs, 'unsampled_pages_no_pixel_claim': unsampled,
            'serial_prefixes_verified': len(checkpoints), 'selected_recipe_records': 2,
            'source_changes': 4, 'available_controls': len(expected_controls),
            'kernel_sha256': m['kernel']['sha256'], 'image_sha256': m['image']['sha256'],
            'logical_image_binding': binding,
            'scope': 'Original documentary custody and explicit two-selection chain; no initial recipe restoration, '
                     'unsampled pixels, row wording, physical/human evidence or full6158 acceptance. '
                     'No independent recomputation of canonical Face digest; seals are hashes, not signatures.'}


if __name__ == '__main__':
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    try:
        result = verify(sys.argv[1])
        pathlib.Path(sys.argv[2]).write_text(json.dumps(result, indent=2) + '\n')
        print(json.dumps(result, indent=2))
    except (ValueError, KeyError, TypeError, StopIteration, OSError, IndexError) as error:
        sys.exit('REFUSED: ' + str(error))
