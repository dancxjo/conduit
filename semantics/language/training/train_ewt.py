#!/usr/bin/env python3
"""Reproducible four-token conditional-UPOS scorer; runtime grammar stays in Plots."""
import collections, hashlib, struct

PIN = 'b7711cce01cdd4f5fcc0a8199b8a50d951b16c0c'
POS = 'ADJ ADP ADV AUX CCONJ DET INTJ NOUN NUM PART PRON PROPN PUNCT SCONJ SYM VERB X'.split()
REL = 'acl advcl advmod amod appos aux case cc ccomp clf compound conj cop csubj dep det discourse dislocated expl fixed flat goeswith iobj list mark nmod nsubj nummod obj obl orphan parataxis punct reparandum root vocative xcomp'.split()
FEATURES, CLASSES, SEED, SCALE = 374, 76, 4907, 256

def read(path):
    rows, skipped = [], collections.Counter()
    for block in path.read_text().strip().split('\n\n'):
        tokens, sid, special = [], '', False
        for line in block.splitlines():
            if line.startswith('# sent_id = '): sid = line[12:]
            if line.startswith('#'): continue
            fields = line.split('\t')
            if len(fields) != 10: continue
            if not fields[0].isdigit(): special = True; continue
            tokens.append(fields)
        if not tokens: continue
        if len(tokens) > 4: skipped['over_four_tokens'] += 1; continue
        if special: skipped['multiword_or_empty_node'] += 1; continue
        if [int(t[0]) for t in tokens] != list(range(1,len(tokens)+1)):
            skipped['noncontiguous_token_ids'] += 1; continue
        heads = [4 if t[6]=='0' else int(t[6])-1 for t in tokens]
        relations = [t[7] for t in tokens]
        if heads.count(4)!=1 or any(h!=4 and h>=len(tokens) for h in heads):
            skipped['invalid_tree'] += 1; continue
        if any(t[3] not in POS or t[7].split(':')[0] not in REL for t in tokens):
            skipped['unsupported_vocabulary'] += 1; continue
        row = dict(id=sid, pos=[POS.index(t[3]) for t in tokens], heads=heads, relations=relations)
        try: row['oracle'] = oracle(row)
        except ValueError: skipped['unreachable_projective_profile'] += 1; continue
        rows.append(row)
    return rows, dict(skipped)

def initial(n): return dict(n=n, unread=0, stack=[4], heads=[5]*4, relations=['dep']*4)
def features(state,pos):
    top=state['stack'][-1]; nxt=state['unread']
    tp=17 if top==4 else pos[top]
    np=17 if nxt==state['n'] else pos[nxt]
    return [tp,18+np,36+tp*18+np,360+len(state['stack']),366+nxt,371+int(top!=4 and state['heads'][top]!=5),373]
def action(a):
    if a<2: return ('shift' if a==0 else 'reduce','dep')
    return ('left_arc' if a<39 else 'right_arc',REL[(a-2)%37])
def apply(state,a):
    kind,rel=action(a);top=state['stack'][-1];nxt=state['unread']
    if kind=='shift': state['stack'].append(nxt);state['unread']+=1
    elif kind=='reduce': state['stack'].pop()
    elif kind=='left_arc': state['heads'][top]=nxt;state['relations'][top]=rel;state['stack'].pop()
    else: state['heads'][nxt]=top;state['relations'][nxt]=rel;state['stack'].append(nxt);state['unread']+=1

def oracle(row):
    state=initial(len(row['pos'])); result=[]
    for _ in range(12):
        if state['unread']==state['n'] and len(state['stack'])==1:
            if state['heads'][:state['n']]!=row['heads']: raise ValueError('unreachable')
            return result
        top=state['stack'][-1];nxt=state['unread'];chosen=None
        if nxt<state['n'] and top!=4 and row['heads'][top]==nxt:
            chosen=2+REL.index(row['relations'][top].split(':')[0])
        elif nxt<state['n'] and row['heads'][nxt]==top:
            chosen=39+REL.index(row['relations'][nxt].split(':')[0])
        elif top!=4 and state['heads'][top]!=5 and all(state['heads'][d]!=5 for d,h in enumerate(row['heads']) if h==top): chosen=1
        elif nxt<state['n']: chosen=0
        if chosen is None: raise ValueError('unreachable')
        result.append([features(state,row['pos']),chosen]);apply(state,chosen)
    raise ValueError('bound')

def semantic(kind, content):
    return hashlib.sha256(b'conduit.info.semantic.v1'+struct.pack('<H',len(kind))+kind.encode()+content).hexdigest()

