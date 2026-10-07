#!/usr/bin/env python3
"""Wrong-UPOS supervised finite scorer. External oracle diagnostics are not runtime proof."""
import argparse, collections, hashlib, itertools, json, pathlib, random, struct
import train_ewt as base

FEATURES = 413
LOOKUPS = 25

def feature_indices(state, pos, row, profile):
    indices = base.features(state, pos)
    indices.append(374 + len(pos))
    lookahead = state["unread"] + 1
    allowed = profile[row["forms"][lookahead]] if lookahead < len(pos) else []
    indices.extend(379 + 2 * code + int(code in allowed) for code in range(17))
    return indices


def forms(path):
    result = {}
    for block in path.read_text().strip().split('\n\n'):
        sid = next((line[12:] for line in block.splitlines() if line.startswith('# sent_id = ')), '')
        result[sid] = [f[1] for line in block.splitlines() if len(f := line.split('\t')) == 10 and f[0].isdigit()]
    return result


def contexts(row, profile):
    return itertools.product(*(profile[word] for word in row['forms']))


def examples(row, profile):
    state = base.initial(len(row['pos']))
    for _, target in row['oracle']:
        gold_features = feature_indices(state, row["pos"], row, profile)
        alternatives = sorted(set(tuple(feature_indices(state, pos, row, profile)) for pos in contexts(row, profile)))
        yield gold_features, target, alternatives
        base.apply(state, target)


def train(rows, profile, epochs):
    samples = [x for row in rows for x in examples(row, profile)]
    weights = [[0] * FEATURES for _ in range(base.CLASSES)]
    totals = [[0] * FEATURES for _ in range(base.CLASSES)]
    times = [[0] * FEATURES for _ in range(base.CLASSES)]
    rng = random.Random(base.SEED); step = 0
    for _ in range(epochs):
        rng.shuffle(samples)
        for gold, target, alternatives in samples:
            step += 1
            best_features, best = max(((indices, cls) for indices in alternatives for cls in range(base.CLASSES)), key=lambda x: (sum(weights[x[1]][i] for i in x[0]), -x[1], tuple(-i for i in x[0])))
            if list(best_features) == gold and best == target:
                continue
            changes = collections.Counter()
            for i in gold: changes[target, i] += 1
            for i in best_features: changes[best, i] -= 1
            for (cls, i), delta in changes.items():
                totals[cls][i] += (step - times[cls][i]) * weights[cls][i]
                times[cls][i] = step; weights[cls][i] += delta
    for cls in range(base.CLASSES):
        for i in range(FEATURES):
            totals[cls][i] += (step - times[cls][i]) * weights[cls][i]
    quantized = [[round(v / step * base.SCALE) for v in row] for row in totals]
    assert all(-32768 <= v <= 32767 for row in quantized for v in row)
    return quantized, len(samples), step


def diagnostic(rows, profile, weights):
    # Lexical diagnostic uses gold action sequence and gold parser states: it is
    # deliberately separate from blind native beam lexical/attachment metrics.
    counts = collections.Counter()
    for row in rows:
        def score(pos):
            state = base.initial(len(pos)); result = 0
            for _, cls in row['oracle']:
                result += sum(weights[cls][i] for i in feature_indices(state, pos, row, profile))
                base.apply(state, cls)
            return result
        chosen = max(contexts(row, profile), key=lambda pos: (score(pos), tuple(-p for p in pos)))
        for word, gold, predicted in zip(row['forms'], row['pos'], chosen):
            counts['tokens'] += 1; counts['pos_correct'] += predicted == gold
            if len(profile[word]) > 1:
                counts['ambiguous_tokens'] += 1; counts['ambiguous_pos_correct'] += predicted == gold
        for gold, target, alternatives in examples(row, profile):
            indices, cls = max(((idx, cls) for idx in alternatives for cls in range(base.CLASSES)), key=lambda x: (sum(weights[x[1]][i] for i in x[0]), -x[1], tuple(-i for i in x[0])))
            counts['oracle_steps'] += 1
            counts['joint_context_action_correct'] += list(indices) == gold and cls == target
    return dict(counts)


def train_artifact(corpus, output, epochs=12):
    class Args: pass
    args = Args(); args.corpus = pathlib.Path(corpus); args.output = pathlib.Path(output); args.epochs = epochs
    return _main(args)

def _main(args):

    splits = {}; excluded = {}; digests = {}
    for name in ['train', 'dev', 'test']:
        path = args.corpus / f'en_ewt-ud-{name}.conllu'
        splits[name], excluded[name] = base.read(path)
        by_id = forms(path)
        for row in splits[name]: row['forms'] = by_id[row['id']]
        digests[name] = hashlib.sha256(path.read_bytes()).hexdigest()
    frequencies = collections.Counter(word for row in splits['train'] for word in row['forms'])
    observed = collections.defaultdict(set)
    for row in splits['train']:
        for word, pos in zip(row['forms'], row['pos']): observed[word].add(pos)
    ranked = sorted(frequencies, key=lambda word: (-frequencies[word], word))
    profile = {word: sorted(observed[word]) for word in ranked if len(observed[word]) <= 4}
    teaching_path = pathlib.Path(__file__).parent / 'ewt_joint_v2/reviewed_teaching.json'
    teaching = json.loads(teaching_path.read_text())
    reviewed_profile = {'Travis': [7,11], 'Hello': [6,7], 'friend': [7],
                        'Morgan': [7,11], 'Alex': [7,11], 'Hi': [6,7], 'pal': [7]}
    # These admitted alternatives are authored profile data, not TRAIN frequency evidence.
    profile = dict([(word, codes) for word, codes in profile.items() if word not in reviewed_profile][:64-len(reviewed_profile)])
    profile.update(reviewed_profile)
    assert len(profile) == 64
    for row in teaching: row['oracle'] = base.oracle(row)
    covered = {}; coverage_exclusions = {}
    for name, rows in splits.items():
        covered[name] = []; reasons = collections.Counter()
        for row in rows:
            if any(word not in profile for word in row['forms']): reasons['profile_oov'] += 1
            elif any(pos not in profile[word] for word, pos in zip(row['forms'], row['pos'])): reasons['gold_pos_absent_from_train_alternatives'] += 1
            else: covered[name].append(row)
        coverage_exclusions[name] = dict(reasons)
    weights, examples_count, steps = train(covered['train'] + teaching * 16, profile, args.epochs)
    artifact = struct.pack('<8sIII', b'CI16SUM1', FEATURES, base.CLASSES, LOOKUPS) + b''.join(struct.pack('<h', v) for row in weights for v in row)
    args.output.mkdir(parents=True, exist_ok=True)
    (args.output / 'ewt_joint.i16').write_bytes(artifact)
    profile_bytes = (json.dumps(profile, sort_keys=True, indent=2) + '\n').encode()
    (args.output / 'lexical_profile.json').write_bytes(profile_bytes)
    for name in ['dev', 'test']:
        (args.output / f'{name}_annotations.jsonl').write_text(''.join(json.dumps({k: v for k, v in row.items() if k != 'oracle'}, sort_keys=True) + '\n' for row in covered[name]))
    source_dir = pathlib.Path(__file__).parents[1]
    manifest = dict(dataset='UD_English-EWT', ud_release='2.18', commit=base.PIN, license='CC BY-SA 4.0', corpus_sha256=digests, seed=base.SEED, epochs=args.epochs, training_examples=examples_count, training_steps=steps, features=FEATURES, outputs=base.CLASSES, lookups=LOOKUPS, precision='i16-weights-i64-sums', quantization_scale=base.SCALE, model_bytes=len(artifact), artifact_sha256=hashlib.sha256(artifact).hexdigest(), model_content_identity=base.semantic('model/content@1', artifact), weights_tensor_content_identity=base.semantic('data/tensor-content@1', artifact[20:]), feature_class_contract_identity=base.semantic('language/parser-v2-scorer-encoding@1', (source_dir / 'parser_scorer_v2.conduit').read_bytes()), action_class_contract_identity=base.semantic('language/parser-scorer-encoding@1', (source_dir / 'parser_scorer.conduit').read_bytes()), availability_contract_identity=base.semantic('language/parser-available-contract@1', (source_dir / 'parser_available.conduit').read_bytes()), joint_choice_contract_identity=base.semantic('language/parser-joint-encoding@1', (source_dir / 'parser_joint.conduit').read_bytes()), lexical_profile_identity=base.semantic('language/parser-lexical-profile@1', profile_bytes), lexical_profile_selection='TRAIN frequency57 excluding seven separately authored reviewed demonstrator/generalization forms; exact-case; native capacity64', reviewed_teaching_sha256=hashlib.sha256(teaching_path.read_bytes()).hexdigest(), reviewed_teaching_sentences=len(teaching), reviewed_teaching_training_repetitions=16, reviewed_teaching_conditional_gold_action_and_state_diagnostic=diagnostic(teaching, profile, weights), reviewed_lexical_alternatives=reviewed_profile, eligible_sentences={name: len(rows) for name, rows in splits.items()}, corpus_profile_exclusions=excluded, lexical_profile_covered_sentences={name: len(rows) for name, rows in covered.items()}, lexical_coverage_exclusions=coverage_exclusions, conditional_gold_action_lexical_and_gold_state_joint_diagnostics={name: diagnostic(rows, profile, weights) for name, rows in covered.items()}, native_decode_metrics='not evaluated by this training script; diagnostics condition on gold state/action and are not blind POS or attachment accuracy', calibration='raw structured ranking scores; no probability or lexical confidence claim')
    (args.output / 'manifest.json').write_text(json.dumps(manifest, sort_keys=True, indent=2) + '\n')
    print(json.dumps({k: manifest[k] for k in ['lexical_profile_covered_sentences', 'conditional_gold_action_lexical_and_gold_state_joint_diagnostics']}, indent=2))

if __name__ == '__main__':
    parser = argparse.ArgumentParser(); parser.add_argument('--corpus', required=True); parser.add_argument('--output', required=True); parser.add_argument('--epochs', type=int, default=12); args = parser.parse_args()
    train_artifact(args.corpus, args.output, args.epochs)
