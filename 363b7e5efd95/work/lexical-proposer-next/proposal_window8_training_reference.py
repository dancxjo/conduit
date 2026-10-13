"""External TRAIN oracle arithmetic only; original Source remains runtime authority.

Past POS comes from the selected training trajectory, not surface spelling or
punctuation. Origin comes from the pinned proposal dictionary/policy receipt.
"""
import pathlib, sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parents[2]/'semantics/language/training'))
import window8_reference as legacy
CATEGORIES, OUTPUTS, LOOKUPS = 446, 76, 27

def features(state, selected_pos, row, profile, origins):
    n=state['n']; current=state['unread']
    if len(selected_pos)!=n or len(origins)!=n or any(o not in (0,1) for o in origins):
        raise ValueError('complete selected trajectory/proposal origin required')
    previous=17 if current==0 else selected_pos[current-1]
    if not 0<=previous<=17: raise ValueError('selected previous POS bound')
    origin=2 if current==n else origins[current]
    result=legacy.features(state,selected_pos,row,profile)+[425+previous,443+origin]
    if len(result)!=LOOKUPS or any(not 0<=i<CATEGORIES for i in result):
        raise ValueError('new feature ABI bounds')
    return result

def alternatives(state,row,profile,origins,selected_history):
    """Vary current/top only, retaining actual earlier trajectory choices.

This is untrusted external supervision. Runtime beam choices and original
proposal Source custody are never fabricated by this helper.
"""
    import itertools
    n=state['n']; top=state['stack'][-1]; current=state['unread']
    if len(selected_history)!=current: raise ValueError('selected history length')
    positions=[i for i in (top,current) if i<n]
    for choices in itertools.product(*(profile[row['forms'][i]] for i in positions)):
        pos=list(selected_history)+[16]*(n-current)
        for ordinal,code in zip(positions,choices):
            # A consumed top already has an actual selected lexical choice.
            if ordinal<current and pos[ordinal]!=code: break
            pos[ordinal]=code
        else:
            yield features(state,pos,row,profile,origins)
