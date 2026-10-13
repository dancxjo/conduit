"""External ECM device stimulus: two distinct packet-aligned Ethernet frames."""
import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'usb-audio-integration/targets/conduitos/plots/usb/fixtures'))
import cdc_ecm_device as device
original = device.Device.bulk
frames = [bytes.fromhex('ffffffffffff02000000000288b5') + bytes([marker]) * (length - 14) for length, marker in [(128, 0x31), (512, 0x72)]]
assert all(device.ethernet(frame) for frame in frames)
def bulk(self, packet_id, body):
    if body and body[0] == 129 and not getattr(self, 'aligned_frames_sent', False):
        self.aligned_frames_sent = True
        for frame in reversed(frames):
            self.frames.appendleft(frame)
        print('EXTERNAL_ALIGNED_FRAMES lengths=128,512', flush=True)
    return original(self, packet_id, body)
device.Device.bulk = bulk
device.main()
