#!/usr/bin/env python3
"""Retained conditional diagnostics and checked-feature interval bounds only."""
import argparse, hashlib, json, pathlib, struct
import train_ewt as base
import train_ewt_joint_v2 as trainer

def analyze(directory):
    root = pathlib.Path(__file__).parent
    directory = pathlib.Path(directory)
    artifact = (directory / 'ewt_joint.i16').read_bytes()
    magic, features, outputs, lookups = struct.unpack('<8sIII', artifact[:20])
    assert (magic, features, outputs, lookups) == (b'CI16SUM1', 413, 76, 25)
    flat = struct.unpack('<' + str((len(artifact)-20)//2) + 'h', artifact[20:])
    weights = [flat[i*413:(i+1)*413] for i in range(76)]
    profile = json.loads((directory / 'lexical_profile.json').read_text())
    groups = [list(range(18)), list(range(18,36)), list(range(36,360)),
              list(range(361,366)), list(range(366,371)), [371,372], [373],
              list(range(375,379))] + [[379+2*i,380+2*i] for i in range(17)]
    assert len(groups) == 25
    bounds = [dict(class_id=i, minimum=sum(min(row[j] for j in g) for g in groups),
                   maximum=sum(max(row[j] for j in g) for g in groups))
              for i,row in enumerate(weights)]
    bound = dict(kind='independent checked categorical feature-group interval enclosure; not observed decode',
                 groups=groups, group_count=25, original_source_score_bound=128525,
                 maximum_absolute_valid_group_score=max(max(abs(r['minimum']),abs(r['maximum'])) for r in bounds),
                 classes=bounds, limitations=[
                     'Only the current checked Source25-index feature ABI is covered; arbitrary categorical inputs are not.',
                     'Independent groups overapproximate possible state combinations.',
                     'No beam agreement, model accuracy or runtime speed claim.'])
    assert bound['maximum_absolute_valid_group_score'] <= 128525
    (directory / 'feature-group-bound.json').write_text(json.dumps(bound,indent=2)+'\n')
    reports = {}
    for name,path in [('new_authored_teaching',directory/'reviewed_teaching.json'),
                      ('previously_observed_v2_teaching',root/'ewt_joint_v2/reviewed_teaching.json'),
                      ('reserved_not_used_for_training',directory/'reserved_evaluation.json')]:
        rows=json.loads(path.read_text())
        for row in rows: row['oracle']=base.oracle(row)
        reports[name]=dict(annotation_sha256=hashlib.sha256(path.read_bytes()).hexdigest(),
                           conditional_gold_state=trainer.diagnostic(rows,profile,weights),
                           conditional_incremental_gold_state=trainer.incremental_diagnostic(rows,profile,weights))
    result=dict(artifact_sha256=hashlib.sha256(artifact).hexdigest(),
                analyzer_sha256=hashlib.sha256(pathlib.Path(__file__).read_bytes()).hexdigest(),
                classification='conditioned diagnostics only; no native checked beam execution; DEV/TEST and motivator previously observed',
                reports=reports)
    (directory/'conditional-regression-analysis.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps(result,indent=2))
if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--directory',required=True)
    analyze(parser.parse_args().directory)
