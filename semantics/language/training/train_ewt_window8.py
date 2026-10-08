#!/usr/bin/env python3
"""External TRAIN-only supervision for the distinct425/76/25 window8 ABI.

Reference states/oracles and conditional diagnostics never establish native
parser accuracy. Fresh Source/Plan decoding is required independently.
"""
import argparse
import collections
import copy
import hashlib
import json
import pathlib
import random
import struct
import train_ewt as base
import window8_reference as reference


def examples(row, profile):
    state = reference.initial(len(row['pos']))
    for target in row['oracle']:
        gold = reference.features(state, row['pos'], row, profile)
        alternatives = sorted(set(tuple(indices) for indices in reference.alternatives(state, row, profile)))
        yield gold, target, alternatives
        reference.apply(state, target)


def train(rows, profile, epochs):
    samples = [sample for row in rows for sample in examples(row, profile)]
    if not samples:
        raise ValueError('no covered TRAIN supervision')
    weights = [[0] * reference.CATEGORIES for _ in range(reference.OUTPUTS)]
    totals = [[0] * reference.CATEGORIES for _ in range(reference.OUTPUTS)]
    times = [[0] * reference.CATEGORIES for _ in range(reference.OUTPUTS)]
    rng, step = random.Random(base.SEED), 0
    for _ in range(epochs):
        rng.shuffle(samples)
        for gold, target, alternatives in samples:
            step += 1
            chosen_indices, chosen_class = max(((indices, cls) for indices in alternatives for cls in range(reference.OUTPUTS)), key=lambda value: (sum(weights[value[1]][i] for i in value[0]), -value[1], tuple(-i for i in value[0])))
            if list(chosen_indices) == gold and chosen_class == target:
                continue
            changes = collections.Counter((target, i) for i in gold)
            changes.subtract((chosen_class, i) for i in chosen_indices)
            for (cls, i), delta in changes.items():
                totals[cls][i] += (step - times[cls][i]) * weights[cls][i]
                times[cls][i] = step
                weights[cls][i] += delta
    quantized = []
    for cls in range(reference.OUTPUTS):
        for i in range(reference.CATEGORIES):
            totals[cls][i] += (step - times[cls][i]) * weights[cls][i]
        quantized.append([round(value / step * base.SCALE) for value in totals[cls]])
    if not all(-32768 <= value <= 32767 for row in quantized for value in row):
        raise ValueError('I16 precision overflow')
    return quantized, len(samples), step


def train_teaching_search(rows, profile, weights, rounds):
    """TRAIN-only early-update search, no native/held-out selection authority."""
    def key(item):
        score, state, chosen, selected, history = item
        return (-score, state['heads'], chosen, state['stack'], history)
    updates = 0
    for _ in range(rounds):
        for row in rows:
            beam = [(0, reference.initial(len(row['forms'])), [0]*len(row['forms']), 0, [])]
            gold_state, gold_history = reference.initial(len(row['forms'])), []
            for target in row['oracle']:
                gold_indices = reference.features(gold_state, row['pos'], row, profile)
                gold_history.append((target, tuple(gold_indices)))
                reference.apply(gold_state, target)
                proposals = []
                for score, state, chosen, selected, history in beam:
                    current = state['unread']
                    options = profile[row['forms'][current]] if current < state['n'] and selected == current else [None]
                    for option in options:
                        pos, next_selected = chosen[:], selected
                        if option is not None:
                            pos[current], next_selected = option, current + 1
                        indices = reference.features(state, pos, row, profile)
                        for code in range(reference.OUTPUTS):
                            if reference.legal(state, code):
                                next_state = copy.deepcopy(state)
                                reference.apply(next_state, code)
                                proposals.append((score + sum(weights[code][i] for i in indices), next_state, pos, next_selected, history + [(code, tuple(indices))]))
                proposals.sort(key=key)
                beam = proposals[:4]
                if not any(item[4] == gold_history for item in beam) or (len(gold_history) == len(row['oracle']) and beam[0][4] != gold_history):
                    if not beam:
                        raise ValueError('TRAIN reference search unexpectedly empty')
                    changes = collections.Counter((code, i) for code, indices in gold_history for i in indices)
                    changes.subtract((code, i) for code, indices in beam[0][4] for i in indices)
                    for (code, index), delta in changes.items():
                        weights[code][index] += delta * base.SCALE
                    updates += 1
                    break
    if not all(-32768 <= value <= 32767 for row in weights for value in row):
        raise ValueError('structured teaching I16 precision overflow')
    return updates


def diagnostic(rows, profile, weights):
    counts = collections.Counter()
    for row in rows:
        for gold, target, alternatives in examples(row, profile):
            indices, cls = max(((indices, cls) for indices in alternatives for cls in range(reference.OUTPUTS)), key=lambda value: (sum(weights[value[1]][i] for i in value[0]), -value[1], tuple(-i for i in value[0])))
            counts['gold_state_steps'] += 1
            counts['gold_state_joint_context_action_correct'] += list(indices) == gold and cls == target
    return dict(counts)


def main(args):
    directory = pathlib.Path(__file__).parent / 'ewt_joint_v3_window8'
    teaching_path = args.teaching or directory / 'reviewed_teaching.json'
    reviewed_path = args.lexical_alternatives or directory / 'reviewed_lexical_alternatives.json'
    teaching, reviewed = json.loads(teaching_path.read_text()), json.loads(reviewed_path.read_text())
    splits, excluded, digests = {}, {}, {}
    for name in ['train', 'dev', 'test']:
        path = args.corpus / f'en_ewt-ud-{name}.conllu'
        splits[name], excluded[name] = reference.read(path)
        digests[name] = hashlib.sha256(path.read_bytes()).hexdigest()
    frequency, observed = collections.Counter(), collections.defaultdict(set)
    for row in splits['train']:
        frequency.update(row['forms'])
        for word, pos in zip(row['forms'], row['pos']):
            observed[word].add(pos)
    ranked = sorted(frequency, key=lambda word: (-frequency[word], word))
    ordinary = [(word, sorted(observed[word])) for word in ranked if word not in reviewed and len(observed[word]) <= 4]
    if len(reviewed) > 64 or any(not 1 <= len(codes) <= 4 for codes in reviewed.values()):
        raise ValueError('reviewed native profile capacity')
    profile = dict(ordinary[:64-len(reviewed)])
    profile.update(reviewed)
    covered, lexical_exclusions = {}, {}
    for split, rows in splits.items():
        accepted, reasons = [], collections.Counter()
        for row in rows:
            if any(word not in profile for word in row['forms']):
                reasons['profile_oov'] += 1
            elif any(pos not in profile[word] for word, pos in zip(row['forms'], row['pos'])):
                reasons['gold_pos_absent_from_train_alternatives'] += 1
            else:
                accepted.append(row)
        covered[split], lexical_exclusions[split] = accepted, dict(reasons)
    for row in teaching:
        row['oracle'] = reference.oracle(row)
    weights, sample_count, steps = train(covered['train'] + teaching * args.teaching_repetitions, profile, args.epochs)
    search_updates = train_teaching_search(teaching, profile, weights, args.teaching_search_rounds)
    artifact = struct.pack('<8sIII', b'CI16SUM1', reference.CATEGORIES, reference.OUTPUTS, reference.LOOKUPS) + b''.join(struct.pack('<h', value) for row in weights for value in row)
    assert len(artifact) == 64620
    args.output.mkdir(parents=True, exist_ok=True)
    (args.output / 'ewt_window8.i16').write_bytes(artifact)
    profile_bytes = (json.dumps(profile, sort_keys=True, indent=2) + '\n').encode()
    (args.output / 'lexical_profile.json').write_bytes(profile_bytes)
    for split in ['dev', 'test']:
        (args.output / f'{split}_annotations.jsonl').write_text(''.join(json.dumps({key: value for key, value in row.items() if key != 'oracle'}, sort_keys=True) + '\n' for row in covered[split]))
    source = (directory.parents[1] / 'parser_window8.conduit').read_bytes()
    manifest = dict(dataset='UD_English-EWT', ud_release='2.18', commit=base.PIN, license='CC BY-SA 4.0', corpus_sha256=digests, seed=base.SEED, epochs=args.epochs, features=reference.CATEGORIES, outputs=reference.OUTPUTS, lookups=reference.LOOKUPS, precision='i16-weights-i64-sums', quantization_scale=base.SCALE, training_examples=sample_count, training_steps=steps, model_bytes=len(artifact), artifact_sha256=hashlib.sha256(artifact).hexdigest(), model_content_identity=base.semantic('model/content@1', artifact), weights_tensor_content_identity=base.semantic('data/tensor-content@1', artifact[20:]), feature_class_contract_identity=base.semantic('language/parser-v3-window8-scorer-encoding@1', source), lexical_profile_identity=base.semantic('language/parser-lexical-profile@1', profile_bytes), reviewed_teaching_sha256=hashlib.sha256(teaching_path.read_bytes()).hexdigest(), reviewed_profile_sha256=hashlib.sha256(reviewed_path.read_bytes()).hexdigest(), reviewed_teaching_training_repetitions=args.teaching_repetitions, reviewed_teaching_search_rounds=args.teaching_search_rounds, reviewed_teaching_search_updates=search_updates, trainer_sha256=hashlib.sha256(pathlib.Path(__file__).read_bytes()).hexdigest(), external_reference_sha256=hashlib.sha256(pathlib.Path(reference.__file__).read_bytes()).hexdigest(), lexical_profile_selection=f'TRAIN frequency{64-len(reviewed)} plus{len(reviewed)} separately authored exact-case reviewed forms; native capacity64', eligible_sentences={name: len(rows) for name, rows in splits.items()}, corpus_profile_exclusions=excluded, lexical_profile_covered_sentences={name: len(rows) for name, rows in covered.items()}, lexical_coverage_exclusions=lexical_exclusions, conditional_gold_state_diagnostics={name: diagnostic(rows, profile, weights) for name, rows in covered.items()}, reviewed_teaching_conditional_gold_state_diagnostic=diagnostic(teaching, profile, weights), native_decode_metrics='not evaluated by this script; fresh native Source decoding required', calibration='raw ranking scores; no confidence/probability claim')
    (args.output / 'manifest.json').write_text(json.dumps(manifest, sort_keys=True, indent=2) + '\n')
    print(json.dumps({key: manifest[key] for key in ['lexical_profile_covered_sentences', 'conditional_gold_state_diagnostics', 'reviewed_teaching_conditional_gold_state_diagnostic']}, indent=2))


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--corpus', type=pathlib.Path, required=True)
    parser.add_argument('--output', type=pathlib.Path, required=True)
    parser.add_argument('--teaching', type=pathlib.Path, help='explicit TRAIN-only teaching data; retained by digest')
    parser.add_argument('--lexical-alternatives', type=pathlib.Path, help='explicit finite lexical policy; retained by digest')
    parser.add_argument('--epochs', type=int, default=12)
    parser.add_argument('--teaching-repetitions', type=int, default=16)
    parser.add_argument('--teaching-search-rounds', type=int, default=100)
    args = parser.parse_args()
    if args.epochs < 1 or args.teaching_repetitions < 1 or args.teaching_search_rounds < 0:
        parser.error('positive finite epochs/repetitions required')
    main(args)
