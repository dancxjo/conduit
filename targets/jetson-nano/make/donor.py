"""Internal xtask helper: stream NVIDIA's official Nano firmware donor.

Discard the Linux APP payload while validating the complete ZIP entry CRC and
recording SHA-256 of both original archive and original disk stream. The sparse
result retains all bytes outside APP and is NOT the original NVIDIA image.
"""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import struct
import tempfile
import urllib.request
import zlib

from media import CHUNK, SECTOR, app_partition, digest, gpt_header, refuse

URL = ("https://developer.download.nvidia.com/embedded/L4T/r32_Release_v7.1/"
       "JP_4.6.1_b110_SD_Card/Jeston_Nano/jetson-nano-jp461-sd-card-image.zip")
ARCHIVE_BYTES = 6_561_246_401
COMPRESSED_BYTES = 6_561_246_105
IMAGE_BYTES = 13_816_037_376
ENTRY_CRC32 = 1_439_039_732
APP_OFFSET = 28_672 * SECTOR
APP_BYTES = (26_980_351 - 28_672 + 1) * SECTOR


def read_exact(stream, size):
    data = bytearray()
    while len(data) < size:
        chunk = stream.read(size - len(data))
        if not chunk:
            refuse("official donor archive is truncated")
        data.extend(chunk)
    return bytes(data)


def local_header(stream):
    header = read_exact(stream, 30)
    fields = struct.unpack("<I5H3I2H", header)
    signature, version, flags, method, _, _, crc, compressed, expanded, name_len, extra_len = fields
    if (signature != 0x04034B50 or version != 45 or flags != 0 or method != 8
            or crc != ENTRY_CRC32 or compressed != 0xFFFFFFFF or expanded != 0xFFFFFFFF
            or name_len != 15 or extra_len > 1024):
        refuse("official donor ZIP local header changed")
    name, extra = read_exact(stream, name_len), read_exact(stream, extra_len)
    if name != b"sd-blob-b01.img":
        refuse("unexpected official donor ZIP member")
    cursor = 0
    zip64 = None
    while cursor + 4 <= len(extra):
        kind, length = struct.unpack_from("<HH", extra, cursor)
        cursor += 4
        value = extra[cursor:cursor + length]
        if len(value) != length:
            refuse("truncated ZIP extra field")
        if kind == 1:
            if zip64 is not None or length != 16:
                refuse("unexpected ZIP64 field")
            zip64 = struct.unpack("<QQ", value)
        cursor += length
    if cursor != len(extra) or zip64 != (IMAGE_BYTES, COMPRESSED_BYTES):
        refuse("official donor ZIP64 entry size changed")
    return header + name + extra


def retained_write(destination, offset, data):
    end = offset + len(data)
    # Write only intersections outside APP. This intentionally scrubs the
    # entire Linux filesystem, including its metadata, from the donor artifact.
    for start, stop in ((offset, min(end, APP_OFFSET)),
                        (max(offset, APP_OFFSET + APP_BYTES), end)):
        if start < stop:
            piece = data[start - offset:stop - offset]
            destination.seek(start)
            if piece.count(0) == len(piece):
                destination.seek(len(piece), os.SEEK_CUR)
            else:
                destination.write(piece)


def acquire(output, url=URL, progress=None):
    # URL substitution is intentionally not exposed through the CLI. The
    # optional argument lets deterministic tests supply an HTTP fixture.
    output = Path(output).absolute()
    if output.exists() or output.is_symlink():
        refuse("donor output must not exist")
    output.parent.mkdir(parents=True, exist_ok=True)
    archive_hash, original_hash = hashlib.sha256(), hashlib.sha256()
    total, crc, compressed_read = 0, 0, 0
    prefix = bytearray()
    with tempfile.NamedTemporaryFile(prefix="nano-donor-", dir=output.parent, delete=False) as result:
        temporary = Path(result.name)
        try:
            with urllib.request.urlopen(url, timeout=60) as response:
                if response.status != 200 or int(response.headers.get("Content-Length", "0")) != ARCHIVE_BYTES:
                    refuse("official donor HTTP length/status changed")
                header = local_header(response)
                archive_hash.update(header)
                inflater = zlib.decompressobj(-15)
                next_report = 256 * CHUNK
                while compressed_read < COMPRESSED_BYTES:
                    compressed = read_exact(response, min(256 * 1024, COMPRESSED_BYTES - compressed_read))
                    archive_hash.update(compressed)
                    compressed_read += len(compressed)
                    pending = compressed
                    while pending:
                        expanded = inflater.decompress(pending, CHUNK)
                        pending = inflater.unconsumed_tail
                        if total + len(expanded) > IMAGE_BYTES:
                            refuse("official donor decompressed size overflow")
                        if len(prefix) < 34 * SECTOR:
                            prefix.extend(expanded[:34 * SECTOR - len(prefix)])
                            if len(prefix) == 34 * SECTOR:
                                _, _, _, entries = gpt_header(io.BytesIO(prefix), 1, IMAGE_BYTES // SECTOR)
                                first = entries[:128]
                                if (first[56:128].decode("utf-16-le").split("\0", 1)[0] != "APP"
                                        or struct.unpack_from("<QQ", first, 32) != (28_672, 26_980_351)):
                                    refuse("official donor APP partition moved")
                        original_hash.update(expanded)
                        crc = zlib.crc32(expanded, crc)
                        retained_write(result, total, expanded)
                        total += len(expanded)
                    if progress and compressed_read >= next_report:
                        progress(compressed_read, COMPRESSED_BYTES)
                        next_report += 256 * CHUNK
                if (not inflater.eof or inflater.unused_data or total != IMAGE_BYTES
                        or crc != ENTRY_CRC32):
                    refuse("official donor ZIP entry length/CRC/deflate mismatch")
                trailer_size = ARCHIVE_BYTES - len(header) - COMPRESSED_BYTES
                trailer = read_exact(response, trailer_size)
                archive_hash.update(trailer)
                # Ensure the central directory agrees with the checked local
                # header; reject a changed multi-file archive or trailing data.
                if (not trailer.startswith(b"PK\x01\x02")
                        or struct.unpack_from("<I", trailer, 16)[0] != ENTRY_CRC32
                        or trailer[46:61] != b"sd-blob-b01.img"
                        or trailer[-22:-18] != b"PK\x05\x06"
                        or struct.unpack_from("<HH", trailer, len(trailer) - 14) != (1, 1)
                        or response.read(1)):
                    refuse("official donor central directory changed")
            result.truncate(IMAGE_BYTES)
            result.flush()
            (offset, size), names = app_partition(temporary)
            if (offset, size) != (APP_OFFSET, APP_BYTES):
                refuse("scrubbed donor GPT changed")
            receipt = {
                "schema": "conduitos-jetson-nano-donor-v1",
                "url": URL, "release": "JetPack 4.6.1 / L4T R32.7.1",
                "release_url": "https://developer.nvidia.com/embedded/linux-tegra-r3271",
                "license_url": ("https://developer.download.nvidia.com/embedded/L4T/"
                                "r32_Release_v7.1/T186/Tegra_Software_License_Agreement-Tegra-Linux.txt"),
                "archive_bytes": ARCHIVE_BYTES, "archive_sha256": archive_hash.hexdigest(),
                "original_image_bytes": IMAGE_BYTES,
                "original_image_sha256": original_hash.hexdigest(),
                "zip_entry_crc32": f"{crc:08x}",
                "scrubbed_app_offset": APP_OFFSET, "scrubbed_app_bytes": APP_BYTES,
                "scrubbed_image_sha256": digest(temporary),
                "retained_partition_names": [name for name in names if name != "APP"],
                "validation": "complete HTTPS archive and ZIP CRC checked; no physical boot proof",
                "published_sha256": None,
            }
            # Atomic no-clobber publication after full validation.
            os.link(temporary, output)
            output.with_name(output.name + ".json").write_text(json.dumps(receipt, indent=2) + "\n")
            return receipt
        finally:
            temporary.unlink(missing_ok=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True)
    arguments = parser.parse_args()
    try:
        acquire(arguments.output, progress=lambda done, size: print(
            f"NVIDIA donor download: {done / CHUNK:.0f}/{size / CHUNK:.0f} MiB", flush=True))
    except (ValueError, OSError, zlib.error) as error:
        parser.exit(1, str(error) + "\n")
