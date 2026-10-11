import struct
import unittest
from author_pcm import author

class AuthoredPcm(unittest.TestCase):
    def test_original_samples_byte_order_and_every_projection_are_independent(self):
        value = author(16, 73, 48)
        original = bytes.fromhex(value['original_pcm_hex'])
        samples = list(struct.iter_unpack('<hh', original))
        self.assertEqual(samples, [tuple(pair) for pair in value['original_samples']])
        observed = bytearray()
        for index, block in enumerate(value['authored_blocks']):
            header = block['header']
            self.assertEqual(header['start-frame'], 48+index*384)
            self.assertEqual(header['payload-bytes'], header['frame-count']*2*2)
            self.assertEqual(block['cadence']['phase'], (header['start-frame']//48)%1000)
            self.assertEqual(len(block['segments']), 8)
            for segment in block['segments']:
                payload = bytes.fromhex(segment['wire_hex'])
                self.assertEqual(len(payload), 48*2*2)
                observed.extend(payload)
        self.assertEqual(observed, original)
        self.assertNotEqual(samples[0], samples[1])
        self.assertEqual(samples[0][0], -samples[0][1])
    def test_finite_authoring_refuses_count_clock_and_frame_overflow(self):
        for args in [(0,73,0), (17,73,0), (1,0,0), (1,1,(1<<64)-384)]:
            with self.assertRaises(ValueError): author(*args)

if __name__ == '__main__': unittest.main()
