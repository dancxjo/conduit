"""Independent Root-budget and original device cancellation checks."""
import json
from pathlib import Path
import struct

def verify(run, device, mode):
    run, device = Path(run), Path(device)
    log=(run/'serial.log').read_text()
    assert log.count('CONDUIT_PROTOCOL_REFUSAL protocol-step-budget-exhausted')==1
    assert 'CONDUIT_PROTOCOL_OUTPUT port=result ' not in log
    assert 'protocol-retirement-refused' not in log
    assert 'panicked at' not in log
    packets=[json.loads(line) for line in (device/'wire.jsonl').read_text().splitlines()]
    cancels=[(i,p) for i,p in enumerate(packets) if p['direction']=='qemu-to-device' and p['type']==21]
    assert len(cancels)==1
    position,cancel=cancels[0]
    held=[p for p in packets[:position] if p['direction']=='qemu-to-device'
        and p['type']==101 and p['id']==cancel['id']]
    assert len(held)==1
    assert struct.unpack_from('<BBHIH',bytes.fromhex(held[0]['wire_hex']),16)[::2]==(129,512,0)
    replies=[p for p in packets[position+1:] if p['direction']=='device-to-qemu'
        and p['type']==101 and p['id']==cancel['id']]
    assert len(replies)==1
    endpoint,status,length,stream,high=struct.unpack_from('<BBHIH',bytes.fromhex(replies[0]['wire_hex']),16)
    assert endpoint==129 and stream==high==0
    assert (status,length)==((0,512) if mode=='late-after-cancel' else (1,0))
    return {'root_outcome':'protocol-step-budget-exhausted','block_outputs':0,
        'original_cancel_id':cancel['id'],'device_response_after_cancel_status':status,
        'late_native_completion_proven':False}

if __name__=='__main__':
    import sys
    print(json.dumps(verify(*sys.argv[1:])))
