"""Focused GPT preservation and native APP assembly conformance."""
import gzip
import hashlib
import importlib.util
from pathlib import Path
import struct
import tempfile
import unittest
import uuid
import zlib

spec = importlib.util.spec_from_file_location("nano_media", Path(__file__).with_name("media.py"))
media = importlib.util.module_from_spec(spec)
spec.loader.exec_module(media)


def donor_image(path):
    sectors = 72 * 2048
    first, last = 2048, sectors - 34
    entries = bytearray(128 * 128)
    partitions = [("APP", first, first + 64 * 2048 - 1),
                  ("RP1", first + 64 * 2048, first + 64 * 2048 + 7),
                  ("EBT", first + 64 * 2048 + 8, first + 64 * 2048 + 15),
                  ("LNX", first + 64 * 2048 + 16, first + 64 * 2048 + 23)]
    for index, (name, start, end) in enumerate(partitions):
        entry = memoryview(entries)[index * 128:(index + 1) * 128]
        entry[:16] = uuid.UUID("0fc63daf-8483-4772-8e79-3d69d8477de4").bytes_le
        entry[16:32] = uuid.UUID(int=index + 1).bytes_le
        struct.pack_into("<2Q", entry, 32, start, end)
        encoded = name.encode("utf-16-le")
        entry[56:56 + len(encoded)] = encoded
    with path.open("wb") as stream:
        stream.truncate(sectors * 512)
        mbr = bytearray(512)
        mbr[450] = 0xEE
        struct.pack_into("<2I", mbr, 454, 1, sectors - 1)
        mbr[510:512] = b"\x55\xaa"
        stream.write(mbr)
        for current, alternate, table in [(1, sectors - 1, 2), (sectors - 1, 1, sectors - 33)]:
            header = bytearray(512)
            header[:8] = b"EFI PART"
            struct.pack_into("<4I4Q", header, 8, 0x10000, 92, 0, 0, current, alternate, first, last)
            header[56:72] = uuid.UUID(int=1).bytes_le
            struct.pack_into("<Q3I", header, 72, table, 128, 128, zlib.crc32(entries))
            struct.pack_into("<I", header, 16, zlib.crc32(header[:92]))
            stream.seek(current * 512)
            stream.write(header)
            stream.seek(table * 512)
            stream.write(entries)
        for name, start, _ in partitions[1:]:
            stream.seek(start * 512)
            stream.write(("firmware-" + name).encode())
    return first * 512, 64 * 1024 * 1024


class MediaTests(unittest.TestCase):
    def test_native_image_header_refuses_missing_magic_and_zero_size(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "Image"
            path.write_bytes(bytes(64))
            with self.assertRaisesRegex(ValueError, "ARM64"):
                media.validate_image(path)
            data = bytearray(64)
            data[56:60] = b"ARM\x64"
            path.write_bytes(data)
            with self.assertRaisesRegex(ValueError, "effective size"):
                media.validate_image(path)

    def test_corrupt_backup_refuses_before_output(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "donor.img"
            donor_image(path)
            with path.open("r+b") as stream:
                stream.seek(-512, 2)
                stream.write(b"BAD GPT!")
            with self.assertRaisesRegex(ValueError, "GPT"):
                media.app_partition(path)

    def test_firmware_preserved_and_native_payload_read_back(self):
        with tempfile.TemporaryDirectory() as directory:
            work = Path(directory)
            donor = work / "donor.img"
            offset, size = donor_image(donor)
            image = work / "Image"
            data = bytearray(128)
            data[56:60] = b"ARM\x64"
            struct.pack_into("<Q", data, 16, 4096)
            image.write_bytes(data)
            output = work / "conduitos.img.gz"
            license_path = work / "license.txt"
            license_path.write_bytes(b"synthetic-test-license")
            manifest = media.package(donor, image, output, "a" * 40, "synthetic-test-only", license_path)
            self.assertTrue(manifest["nvidia_firmware_license_included"])
            self.assertEqual(manifest["nvidia_firmware_license_sha256"],
                             hashlib.sha256(b"synthetic-test-license").hexdigest())
            assembled = work / "readback.img"
            with gzip.open(output, "rb") as source, assembled.open("wb") as target:
                while chunk := source.read(1024 * 1024):
                    target.write(chunk)
            with donor.open("rb") as original, assembled.open("rb") as result:
                self.assertEqual(original.read(offset), result.read(offset))
                original.seek(offset + size)
                result.seek(offset + size)
                self.assertEqual(original.read(), result.read())
            self.assertEqual(manifest["native_image_sha256"], hashlib.sha256(data).hexdigest())
            self.assertNotIn(b"FDT", media.EXTLINUX)
            self.assertTrue(output.with_name(output.name + ".sha256").is_file())


if __name__ == "__main__":
    unittest.main()
