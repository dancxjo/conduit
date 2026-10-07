#!/usr/bin/env python3
"""Reproducible four-token conditional-UPOS scorer; runtime grammar stays in Plots."""
import argparse, collections, hashlib, json, pathlib, random, struct

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

def train(rows,epochs):
    examples=[example for row in rows for example in row['oracle']]
    weights=[[0]*FEATURES for _ in range(CLASSES)]
    totals=[[0]*FEATURES for _ in range(CLASSES)];times=[[0]*FEATURES for _ in range(CLASSES)]
    rng=random.Random(SEED);step=0
    for _ in range(epochs):
        rng.shuffle(examples)
        for indices,target in examples:
            step+=1
            scores=[sum(w[i] for i in indices) for w in weights]
            best=max(range(CLASSES),key=lambda a:(scores[a],-a))
            if best==target: continue
            for cls,delta in [(target,1),(best,-1)]:
                for i in indices:
                    totals[cls][i]+=(step-times[cls][i])*weights[cls][i]
                    times[cls][i]=step;weights[cls][i]+=delta
    for cls in range(CLASSES):
        for i in range(FEATURES):
            totals[cls][i]+=(step-times[cls][i])*weights[cls][i]
    averaged=[[v/step for v in row] for row in totals]
    quantized=[[round(v*SCALE) for v in row] for row in averaged]
    assert all(-32768<=v<=32767 for row in quantized for v in row)
    return quantized,averaged,len(examples),step

def diagnostic(rows,weights):
    correct=total=0
    for row in rows:
        for indices,target in row['oracle']:
            scores=[sum(w[i] for i in indices) for w in weights]
            best=max(range(CLASSES),key=lambda a:(scores[a],-a))
            correct+=best==target;total+=1
    return dict(oracle_action_correct=correct,oracle_action_total=total,accuracy=correct/total if total else None)

def semantic(kind, content):
    return hashlib.sha256(b'conduit.info.semantic.v1'+struct.pack('<H',len(kind))+kind.encode()+content).hexdigest()

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--corpus',type=pathlib.Path,required=True);parser.add_argument('--output',type=pathlib.Path,required=True);parser.add_argument('--epochs',type=int,default=12);args=parser.parse_args()
    splits={};exclusions={};digests={}
    for name in ['train','dev','test']:
        path=args.corpus/f'en_ewt-ud-{name}.conllu';digests[name]=hashlib.sha256(path.read_bytes()).hexdigest()
        splits[name],exclusions[name]=read(path)
    weights,floating,examples,steps=train(splits['train'],args.epochs)
    # Generic container: magic, category count, output count, lookup count,
    # then output-major signed little-endian I16 learned weights.
    artifact=struct.pack('<8sIII',b'CI16SUM1',FEATURES,CLASSES,7)+b''.join(struct.pack('<h',v) for row in weights for v in row)
    args.output.mkdir(parents=True,exist_ok=True)
    (args.output/'ewt_four_token.i16').write_bytes(artifact)
    for name in ['dev','test']:
        (args.output/f'{name}_annotations.jsonl').write_text(''.join(json.dumps({k:v for k,v in r.items() if k!='oracle'},sort_keys=True)+'\n' for r in splits[name]))
    metrics={n:diagnostic(rows,weights) for n,rows in splits.items()}
    disagreement=0;observed=0
    for row in splits['test']:
        for indices,_ in row['oracle']:
            choices=[max(range(CLASSES),key=lambda a:(sum(w[a][i] for i in indices),-a)) for w in [weights,floating]]
            disagreement+=choices[0]!=choices[1];observed+=1
    source_encoding=pathlib.Path(__file__).parents[1]/'parser_scorer.conduit'
    manifest=dict(feature_class_contract_identity=semantic('language/parser-scorer-encoding@1',source_encoding.read_bytes()),dataset='UD_English-EWT',ud_release='2.18',commit=PIN,license='CC BY-SA 4.0',official_splits=True,corpus_sha256=digests,eligible_sentences={k:len(v) for k,v in splits.items()},exclusions=exclusions,profile='arc-eager-four-token-ud-base37-gold-upos-categorical-v1',pos=POS,relations=REL,features=FEATURES,outputs=CLASSES,lookups=7,precision='i16-weights-i64-sums',quantization_scale=SCALE,seed=SEED,epochs=args.epochs,training_examples=examples,training_steps=steps,model_bytes=len(artifact),artifact_sha256=hashlib.sha256(artifact).hexdigest(),model_content_identity=semantic('model/content@1',artifact),weights_tensor_content_identity=semantic('data/tensor-content@1',artifact[20:]),diagnostic_oracle_action_metrics=metrics,quantization_test_disagreements=disagreement,quantization_test_observations=observed,native_decode_metrics='not yet evaluated; oracle diagnostic is not attachment accuracy',lexical_condition='gold UD UPOS and gold syntactic tokenization; no POS/lexical prediction metric')
    (args.output/'manifest.json').write_text(json.dumps(manifest,indent=2,sort_keys=True)+'\n')
    print(json.dumps({k:manifest[k] for k in ['eligible_sentences','exclusions','model_bytes','diagnostic_oracle_action_metrics']},indent=2))
if __name__=='__main__':main()
