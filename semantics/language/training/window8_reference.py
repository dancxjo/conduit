"""TRAIN-only oracle/reference arithmetic; production grammar belongs to Source.

This module creates supervised examples and diagnoses arithmetic parity. Its
states, actions and scores are never admitted parser or linguistic receipts.
"""
import itertools
from train_ewt import POS, REL

ROOT, UNASSIGNED, CATEGORIES, OUTPUTS, LOOKUPS = 8, 9, 425, 76, 25


def initial(count):
    if not 1 <= count <= 8:
        raise ValueError("window8 token bound")
    return dict(n=count, unread=0, stack=[ROOT], heads=[UNASSIGNED] * 8,
                relations=['dep'] * 8)


def action(code):
    if not 0 <= code < OUTPUTS:
        raise ValueError("window8 class bound")
    if code < 2:
        return ('shift' if code == 0 else 'reduce', 'dep')
    return ('left_arc' if code < 39 else 'right_arc', REL[(code - 2) % 37])


def apply(state, code):
    """Untrusted training oracle mutation; Source parity is checked separately."""
    kind, relation = action(code)
    top, current = state['stack'][-1], state['unread']
    if kind == 'shift':
        state['stack'].append(current)
        state['unread'] += 1
    elif kind == 'reduce':
        state['stack'].pop()
    elif kind == 'left_arc':
        state['heads'][top], state['relations'][top] = current, relation
        state['stack'].pop()
    else:
        state['heads'][current], state['relations'][current] = top, relation
        state['stack'].append(current)
        state['unread'] += 1


def features(state, pos, row, profile):
    top, current = state['stack'][-1], state['unread']
    top_pos = 17 if top == ROOT else pos[top]
    current_pos = 17 if current == state['n'] else pos[current]
    lookahead = current + 1
    candidates = profile[row['forms'][lookahead]] if lookahead < state['n'] else []
    result = [top_pos, 18 + current_pos, 36 + 18 * top_pos + current_pos,
              360 + len(state['stack']), 370 + current,
              379 + int(top != ROOT and state['heads'][top] != UNASSIGNED),
              381, 382 + state['n']]
    result.extend(391 + 2 * code + int(code in candidates) for code in range(17))
    assert len(result) == LOOKUPS and all(0 <= i < CATEGORIES for i in result)
    return result


def alternatives(state, row, profile):
    """At most sixteen relevant top/current choices, no future POS selection."""
    top, current = state['stack'][-1], state['unread']
    positions = [i for i in (top, current) if i < state['n']]
    for chosen in itertools.product(*(profile[row['forms'][i]] for i in positions)):
        pos = [16] * state['n']
        for ordinal, code in zip(positions, chosen):
            pos[ordinal] = code
        yield features(state, pos, row, profile)


def oracle(row):
    state = initial(len(row['pos']))
    result = []
    for _ in range(32):
        if state['unread'] == state['n'] and len(state['stack']) == 1:
            if state['heads'][:state['n']] != row['heads']:
                raise ValueError('unreachable window8 teaching graph')
            return result
        top, current = state['stack'][-1], state['unread']
        if current < state['n'] and top != ROOT and row['heads'][top] == current:
            chosen = 2 + REL.index(row['relations'][top].split(':')[0])
        elif current < state['n'] and row['heads'][current] == top:
            chosen = 39 + REL.index(row['relations'][current].split(':')[0])
        elif top != ROOT and state['heads'][top] != UNASSIGNED and all(
                state['heads'][d] != UNASSIGNED
                for d, head in enumerate(row['heads']) if head == top):
            chosen = 1
        elif current < state['n']:
            chosen = 0
        else:
            raise ValueError('unreachable window8 projective profile')
        result.append(chosen)
        apply(state, chosen)
    raise ValueError('window8 oracle step bound')


def read(path):
    """Pinned corpus extraction for training only, separate from native admission."""
    import collections
    rows, skipped = [], collections.Counter()
    for block in path.read_text().strip().split('\n\n'):
        tokens, sid, special = [], '', False
        for line in block.splitlines():
            if line.startswith('# sent_id = '):
                sid = line[12:]
            if line.startswith('#'):
                continue
            fields = line.split('\t')
            if len(fields) != 10:
                continue
            if not fields[0].isdigit():
                special = True
            else:
                tokens.append(fields)
        if not tokens:
            continue
        if len(tokens) > 8:
            skipped['over_eight_tokens'] += 1
            continue
        if special:
            skipped['multiword_or_empty_node'] += 1
            continue
        if [int(t[0]) for t in tokens] != list(range(1, len(tokens) + 1)):
            skipped['noncontiguous_token_ids'] += 1
            continue
        heads = [ROOT if t[6] == '0' else int(t[6]) - 1 for t in tokens]
        if heads.count(ROOT) != 1 or any(h != ROOT and not 0 <= h < len(tokens) for h in heads):
            skipped['invalid_tree'] += 1
            continue
        if any(t[3] not in POS or t[7].split(':')[0] not in REL for t in tokens):
            skipped['unsupported_vocabulary'] += 1
            continue
        row = dict(id=sid, forms=[t[1] for t in tokens],
                   pos=[POS.index(t[3]) for t in tokens], heads=heads,
                   relations=[t[7] for t in tokens])
        try:
            row['oracle'] = oracle(row)
        except ValueError:
            skipped['unreachable_projective_profile'] += 1
            continue
        rows.append(row)
    return rows, dict(skipped)


def legal(state, code):
    """Untrusted TRAIN search reference; native Source is admission authority."""
    kind, relation = action(code)
    top, current, count = state['stack'][-1], state['unread'], state['n']
    if kind == 'shift':
        return current < count and len(state['stack']) < 9
    if kind == 'reduce':
        return top < ROOT and state['heads'][top] < UNASSIGNED
    if kind == 'left_arc':
        return current < count and top < ROOT and state['heads'][top] == UNASSIGNED and relation != 'root'
    if not (current < count and len(state['stack']) < 9 and state['heads'][current] == UNASSIGNED):
        return False
    if relation == 'root':
        return top == ROOT and ROOT not in state['heads']
    if top == ROOT:
        return False
    for _ in range(8):
        if top == current:
            return False
        if top >= ROOT:
            return True
        top = state['heads'][top]
    return top != current
