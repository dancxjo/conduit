"""Conformance for streaming CRC refusal and APP-only scrubbing."""
import io
import importlib.util
from pathlib import Path
import struct
import sys
import tempfile
import unittest
from unittest.mock import patch
import zlib

sys.path.insert(0, str(Path(__file__).parent))
spec = importlib.util.spec_from_file_location("nano_donor", Path(__file__).with_name("donor.py"))
donor = importlib.util.module_from_spec(spec)
spec.loader.exec_module(donor)


class Response(io.BytesIO):
    status = 200

    def __init__(self, data):
        super().__init__(data)
        self.headers = {"Content-Length": str(len(data))}


class DonorTests(unittest.TestCase):
    def test_scrub_preserves_both_boundaries(self):
        for offset, size in ((0, 32), (8, 20), (12, 12), (16, 24), (24, 8)):
            with self.subTest(offset=offset, size=size):
                result = io.BytesIO(bytes(40))
                data = b"x" * size
                with patch.object(donor, "APP_OFFSET", 12), patch.object(donor, "APP_BYTES", 12):
                    donor.retained_write(result, offset, data)
                expected = bytearray(40)
                for i in range(offset, offset + size):
                    if not 12 <= i < 24:
                        expected[i] = ord("x")
                self.assertEqual(result.getvalue(), expected)

    def test_entire_decompressed_crc_required_before_publication(self):
        # A valid, completely inflated stream with an incorrect archive CRC
        # must never publish a donor, even when header sizes agree.
        content = b"x" * 128
        compressed = zlib.compress(content)[2:-4]
        declared_crc = zlib.crc32(b"y" * 128)
        name = b"sd-blob-b01.img"
        extra = struct.pack("<HHQQ", 1, 16, len(content), len(compressed))
        header = struct.pack("<I5H3I2H", 0x04034B50, 45, 0, 8, 0, 0,
                             declared_crc, 0xFFFFFFFF, 0xFFFFFFFF, len(name), len(extra))
        archive = header + name + extra + compressed
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "donor.img"
            with (patch.object(donor, "IMAGE_BYTES", len(content)),
                  patch.object(donor, "COMPRESSED_BYTES", len(compressed)),
                  patch.object(donor, "ARCHIVE_BYTES", len(archive)),
                  patch.object(donor, "ENTRY_CRC32", declared_crc),
                  patch.object(donor.urllib.request, "urlopen", return_value=Response(archive))):
                with self.assertRaisesRegex(ValueError, "CRC"):
                    donor.acquire(output)
            self.assertFalse(output.exists())
            self.assertEqual(list(Path(directory).iterdir()), [])


if __name__ == "__main__":
    unittest.main()
