"""Synthetic checker regressions; these records are not QEMU evidence."""
import hashlib
import json
import struct
import unittest
from author_pcm import author
from check_audio_wire import check


def fixture():
    authored = author(9)
    digest = hashlib.sha256(json.dumps(authored).encode()).hexdigest()
    entries = []
    def record(kind, body):
        entries.append({'direction': 'qemu-to-device', 'type': kind,
                        'wire_hex': (struct.pack('<IIQ',kind,len(body),len(entries)+1)+body).hex()})
    record(100, struct.pack('<BBBBHHH',0,1,0x22,0,0x100,1,3)+bytes.fromhex('80bb00'))
    record(100, struct.pack('<BBBBHHH',128,0x81,0xa2,0,0x100,1,3))
    for block in authored['authored_blocks']:
        for segment in block['segments']:
            record(102, struct.pack('<BBH',1,0,192)+bytes.fromhex(segment['wire_hex']))
    receipt = {name: authored[name] for name in ['clock_id','start_frame','sample_rate_hz','channels','representation','original_pcm_hex']}
    receipt.update(configuration=1, interface=1, alternate=1, endpoint=1,
                   authored_input_sha256=digest, terminal='drained',
                   original_stop_acknowledged=True, possessions_retired=True,
                   native_batches=[{'outcome':'completed','packets':[{'frame':(100+batch*8+packet)%2048,'requested':192,'actual':192} for packet in range(8)]} for batch in range(9)])
    return entries, receipt, authored, digest


class Evidence(unittest.TestCase):
    def test_synthetic_matching_native_and_external_records_check(self):
        self.assertEqual(check(*fixture())['packets'],72)
    def test_real_underrun_or_service_gap_never_becomes_completion(self):
        for underrun in [True, False]:
            entries, receipt, authored, digest = fixture()
            if underrun: receipt['native_batches'][2]['outcome']='underrun'
            else: receipt['native_batches'][2]['packets'][0]['frame']+=1
            with self.assertRaises(ValueError): check(entries, receipt, authored, digest)
    def test_substituted_original_samples_segments_or_stop_are_refused(self):
        for mutation in ['sample','segment','digest','stop']:
            entries, receipt, authored, digest = fixture()
            if mutation=='sample': authored['original_samples'][0][0]+=1
            elif mutation=='segment': authored['authored_blocks'][0]['segments'][0]['wire_hex']='00'*192
            elif mutation=='digest': receipt['authored_input_sha256']='00'*32
            else: receipt['original_stop_acknowledged']=False
            with self.assertRaises(ValueError): check(entries, receipt, authored, digest)

if __name__=='__main__': unittest.main()
