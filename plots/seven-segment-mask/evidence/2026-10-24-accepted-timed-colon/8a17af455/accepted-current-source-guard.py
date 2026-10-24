"""Narrow source correspondence. Callers must separately require full actual CI gates."""
import pathlib,subprocess,hashlib,json,runpy
ROOT=pathlib.Path(__file__).resolve().parent.parent;R=ROOT/'work/6267-review';A='8a17af455f08702bc3ef3124b69e7d8c624b3b8a';C='e8797ffd23928391face70b5586ae37d9234c041';T='be722c7c85e0553256f1cb4c42ac6919081f3755';E='08dd7a420cf6a65aafc7a445d92ec9b1ac92fc2e'
def git(*a):return subprocess.check_output(['git','-C',str(R),*a],text=True).strip()
def sha(b):return hashlib.sha256(b).hexdigest()
def verify(accepted,current):
 assert accepted==A and current==C,'Only exact named accepted8a17/currente879 pair; no descendant waiver'
 assert git('rev-parse',C+'^{tree}')==T
 subprocess.run(['git','-C',str(R),'merge-base','--is-ancestor',A,C],check=True)
 delta=git('diff','--raw','--no-abbrev',A,C);assert sha(delta.encode())=='ac961a649df273f08c848ed146f8ef41f133b7ab02a33f9d1861b673d47268c7'
 paths=git('diff','--name-only',A,C).splitlines();assert len(paths)==25
 review=json.loads((ROOT/'outputs/6313-6381-8a17-be722-requirement-fidelity-review.json').read_bytes());assert review['status']=='conditional_requirement_fidelity_yes'and review['accepted_source']==A and review['current_source']==C and review['current_tree']==T
 stable={}
 for p,row in review['exact_feature_and_stable_inputs'].items():assert git('rev-parse',A+':'+p)==git('rev-parse',C+':'+p)==row['accepted']==row['current'];stable[p]=row['current']
 stage=runpy.run_path(str(ROOT/'work/verify-reviewed-environment-combined.py'))['verify_runtime_stage'](C);p=stage['proof'];assert p['packet']==E and p['actual_tests']==126 and p['tree']==T and p['actual_ELFs']==248
 return {'method':'named_fullIntegration_accepted8a17_with_separate_currentbe722_owning_proof','accepted':A,'current':C,'accepted_owning_execution_claimed':False,'actual_execution_source':p['execution_source'],'packet':E,'whole_current_tree':T,'runtime_counts':[35,36],'Mask_tests':13,'strict_alltarget_default_capture':True,'stable_features_and_inputs':stable,'exact25_changed_paths':paths,'raw25delta_sha256':sha(delta.encode()),'current_owning_proof':stage,'qualification':'Full Integration belongs to accepted8a17. Actual current owning126 belongs to f7a/be722; no local owning execution at8a17 or currentglobalIntegration is inferred. Callers retain fullCandidate/delivery/Integration/freshscope checks.'}
