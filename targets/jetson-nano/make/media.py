"""Internal xtask helper: replace only APP in a complete NVIDIA Nano SD image.

The donor supplies NVIDIA's boot partitions; this helper never creates or flashes
firmware. The resulting APP contains only native ConduitOS and extlinux config.
"""
import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import tempfile
import uuid
import zlib

SECTOR = 512
CHUNK = 1024 * 1024
NVIDIA_RELEASE_URL = "https://developer.nvidia.com/embedded/linux-tegra-r3271"
NVIDIA_LICENSE_URL = ("https://developer.download.nvidia.com/embedded/L4T/"
                      "r32_Release_v7.1/T186/Tegra_Software_License_Agreement-Tegra-Linux.txt")
EXTLINUX = b"""DEFAULT conduitos
TIMEOUT 10
MENU TITLE ConduitOS Jetson Nano
LABEL conduitos
    MENU LABEL Native ConduitOS (experimental)
    LINUX /boot/conduitos/Image
    APPEND ${cbootargs}
"""


def refuse(detail):
    raise ValueError("jetson-nano-media-refused: " + detail)


def digest(path):
    hasher = hashlib.sha256()
    with open(path, "rb") as stream:
        for data in iter(lambda: stream.read(CHUNK), b""):
            hasher.update(data)
    return hasher.hexdigest()


def read_exact(stream, offset, size):
    stream.seek(offset)
    data = stream.read(size)
    if len(data) != size:
        refuse("truncated GPT")
    return data


def gpt_header(stream, lba, sectors):
    block = read_exact(stream, lba * SECTOR, SECTOR)
    if block[:8] != b"EFI PART":
        refuse("complete GPT donor required")
    revision, size, crc, reserved = struct.unpack_from("<4I", block, 8)
    if revision != 0x10000 or not 92 <= size <= SECTOR or reserved:
        refuse("unsupported GPT header")
    checked = bytearray(block[:size])
    checked[16:20] = bytes(4)
    if zlib.crc32(checked) != crc:
        refuse("GPT header CRC mismatch")
    current, alternate, first, last = struct.unpack_from("<4Q", block, 24)
    entries_lba, count, width, entries_crc = struct.unpack_from("<Q3I", block, 72)
    if (current != lba or alternate != (sectors - 1 if lba == 1 else 1)
            or not 2 <= first <= last < sectors - 1
            or not 1 <= count <= 128 or width != 128):
        refuse("unsupported GPT geometry")
    entry_bytes = count * width
    entry_end = entries_lba * SECTOR + entry_bytes
    if (entries_lba < 2 or entry_end > sectors * SECTOR
            or not (entry_end <= first * SECTOR or entries_lba > last)
            or entries_lba <= lba < entries_lba + (entry_bytes + 511) // 512):
        refuse("GPT entries overlap usable space or header")
    entries = read_exact(stream, entries_lba * SECTOR, entry_bytes)
    if zlib.crc32(entries) != entries_crc:
        refuse("GPT entry CRC mismatch")
    return block[56:72], first, last, entries


def app_partition(donor):
    size = donor.stat().st_size
    if size % SECTOR or size < 128 * SECTOR:
        refuse("invalid donor image size")
    sectors = size // SECTOR
    with donor.open("rb") as stream:
        mbr = read_exact(stream, 0, SECTOR)
        if mbr[510:512] != b"\x55\xaa" or mbr[450] != 0xEE:
            refuse("GPT protective MBR required")
        primary = gpt_header(stream, 1, sectors)
        backup = gpt_header(stream, sectors - 1, sectors)
    if primary != backup:
        refuse("primary and backup GPT disagree")
    _, first, last, entries = primary
    ranges = []
    app = None
    names = []
    for index in range(len(entries) // 128):
        entry = entries[index * 128:(index + 1) * 128]
        if entry[:16] == bytes(16):
            continue
        start, end = struct.unpack_from("<2Q", entry, 32)
        name = entry[56:128].decode("utf-16-le").split("\0", 1)[0]
        if not first <= start <= end <= last:
            refuse("partition outside GPT usable space")
        if any(start <= old_end and old_start <= end for old_start, old_end in ranges):
            refuse("overlapping donor partitions")
        ranges.append((start, end))
        names.append(name)
        if name == "APP":
            if app is not None or index != 0:
                refuse("APP must be unique GPT partition 1")
            app = (start * SECTOR, (end - start + 1) * SECTOR)
    # R32 P3448-0000 keeps boot-stage binaries on SD. A plain rootfs card
    # cannot establish that those prerequisites exist on this board's QSPI.
    if app is None or not {"RP1", "EBT", "LNX"}.issubset(names):
        refuse("P3448-0000 complete firmware donor required (APP/RP1/EBT/LNX)")
    if app[1] < 64 * CHUNK:
        refuse("APP partition is too small")
    return app, names


def validate_image(image):
    if not image.is_file() or image.is_symlink():
        refuse("native Image must be a regular file")
    with image.open("rb") as stream:
        header = stream.read(64)
    if len(header) != 64 or header[56:60] != b"ARM\x64":
        refuse("native ARM64 Image header required")
    image_size, flags = struct.unpack_from("<2Q", header, 16)
    if not image_size or image_size < image.stat().st_size or flags & 1:
        refuse("invalid native Image effective size or endianness")


def sparse_copy(source, destination, source_offset, output_offset, size):
    source.seek(source_offset)
    destination.seek(output_offset)
    left = size
    while left:
        data = source.read(min(CHUNK, left))
        if not data:
            refuse("source changed or truncated during assembly")
        if data.count(0) == len(data):
            destination.seek(len(data), os.SEEK_CUR)
        else:
            destination.write(data)
        left -= len(data)


def run(*args):
    subprocess.run(args, check=True, stdin=subprocess.DEVNULL,
                   stdout=subprocess.PIPE, stderr=subprocess.PIPE)


def package(donor, image, output, source_sha, firmware_release, license_file=None):
    donor, image, output = (Path(p).absolute() for p in (donor, image, output))
    if not donor.is_file() or donor.is_symlink():
        refuse("donor must be a regular complete SD image")
    if any(path.exists() or path.is_symlink() for path in (
            output, output.with_name(output.name + ".json"),
            output.with_name(output.name + ".sha256"),
            output.with_name(output.name + ".NVIDIA-LICENSE.txt"))):
        refuse("output and sidecars must not exist")
    if len(source_sha) != 40 or any(c not in "0123456789abcdef" for c in source_sha):
        refuse("exact lowercase source commit SHA required")
    if not firmware_release or any(c in firmware_release for c in "\r\n\0"):
        refuse("firmware donor release identity required")
    license_path = Path(license_file).absolute() if license_file is not None else None
    if license_path is not None and (not license_path.is_file() or license_path.is_symlink()
                                     or not 1 <= license_path.stat().st_size <= 2 * CHUNK):
        refuse("firmware license must be a nonempty regular file of at most 2 MiB")
    license_bytes = license_path.read_bytes() if license_path is not None else None
    if license_bytes is not None and not 1 <= len(license_bytes) <= 2 * CHUNK:
        refuse("firmware license changed while reading")
    validate_image(image)
    (offset, app_size), names = app_partition(donor)
    if image.stat().st_size > app_size // 2:
        refuse("Image does not fit bounded APP staging allowance")
    output.parent.mkdir(parents=True, exist_ok=True)
    for tool in ("mkfs.ext4", "e2fsck", "debugfs"):
        if shutil.which(tool) is None:
            refuse("missing host tool: " + tool)
    donor_sha, image_sha = digest(donor), digest(image)
    manifest = {
        "schema": "conduitos-jetson-nano-media-v1",
        "board": "p3450-0000/p3448-0000",
        "execution": "native-conduitos",
        "validation": "assembled-only; no physical boot proof",
        "source_commit": source_sha,
        "donor_release": firmware_release,
        "donor_release_url": NVIDIA_RELEASE_URL,
        "nvidia_firmware_license_url": NVIDIA_LICENSE_URL,
        "nvidia_firmware_license_included": license_path is not None,
        "nvidia_firmware_license_sha256": hashlib.sha256(license_bytes).hexdigest() if license_bytes else None,
        "donor_sha256": donor_sha,
        "native_image_sha256": image_sha,
        "firmware_dtb": "retained from NVIDIA boot chain; no FDT override",
        "app_offset": offset, "app_bytes": app_size,
        "preserved_partition_names": [name for name in names if name != "APP"],
    }
    receipt_path = donor.with_name(donor.name + ".json")
    if receipt_path.exists():
        receipt = json.loads(receipt_path.read_text())
        if receipt.get("scrubbed_image_sha256") != donor_sha:
            refuse("donor acquisition receipt digest mismatch")
        manifest["donor_acquisition"] = receipt
    with tempfile.TemporaryDirectory(prefix="nano-media-", dir=output.parent) as work:
        work = Path(work)
        tree = work / "root"
        boot = tree / "boot" / "conduitos"
        boot.mkdir(parents=True)
        shutil.copyfile(image, boot / "Image")
        if license_path is not None:
            (boot / "NVIDIA-LICENSE.txt").write_bytes(license_bytes)
        (boot / "NVIDIA-FIRMWARE-SOURCE.txt").write_text(
            "NVIDIA firmware retained unchanged outside APP.\n"
            + "Donor release: " + firmware_release + "\n"
            + "Release: " + NVIDIA_RELEASE_URL + "\n"
            + "Applicable NVIDIA license reference: " + NVIDIA_LICENSE_URL + "\n"
            + "Source reference only; this file does not replace applicable notices.\n")
        (tree / "boot" / "extlinux").mkdir()
        (tree / "boot" / "extlinux" / "extlinux.conf").write_bytes(EXTLINUX)
        (boot / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
        filesystem = work / "app.ext4"
        with filesystem.open("xb") as stream:
            stream.truncate(app_size)
        # Conservative features for the older NVIDIA U-Boot ext4 reader.
        run("mkfs.ext4", "-q", "-F", "-b", "4096", "-L", "APP", "-U",
            str(uuid.UUID(image_sha[:32])), "-O", "^64bit,^metadata_csum,^orphan_file",
            "-E", "lazy_itable_init=0,lazy_journal_init=0", "-d", str(tree), str(filesystem))
        run("e2fsck", "-f", "-n", str(filesystem))
        extracted = work / "Image.readback"
        run("debugfs", "-R", 'dump /boot/conduitos/Image "' + str(extracted).replace("\\", "\\\\").replace('"', '\\"') + '"', str(filesystem))
        if not extracted.is_file() or digest(extracted) != image_sha:
            refuse("APP native Image readback mismatch")
        assembled = work / "assembled.img"
        with donor.open("rb") as original, filesystem.open("rb") as app, assembled.open("xb") as result:
            sparse_copy(original, result, 0, 0, offset)
            sparse_copy(app, result, 0, offset, app_size)
            tail = offset + app_size
            sparse_copy(original, result, tail, tail, donor.stat().st_size - tail)
            result.truncate(donor.stat().st_size)
        if app_partition(assembled) != ((offset, app_size), names):
            refuse("assembled GPT changed")
        # Bind the finished output to the same immutable donor and image.
        if digest(donor) != donor_sha or digest(image) != image_sha:
            refuse("input changed during assembly")
        manifest["disk_image_sha256"] = digest(assembled)
        compressed_output = work / "assembled.img.gz"
        with assembled.open("rb") as source, compressed_output.open("xb") as target:
            with gzip.GzipFile(filename="", mode="wb", fileobj=target, mtime=0) as compressed:
                shutil.copyfileobj(source, compressed, CHUNK)
        os.link(compressed_output, output)
    if license_bytes is not None:
        output.with_name(output.name + ".NVIDIA-LICENSE.txt").write_bytes(license_bytes)
    manifest["compressed_sha256"] = digest(output)
    output.with_name(output.name + ".json").write_text(json.dumps(manifest, indent=2) + "\n")
    output.with_name(output.name + ".sha256").write_text(manifest["compressed_sha256"] + "  " + output.name + "\n")
    return manifest


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--donor", required=True)
    parser.add_argument("--image", required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument("--source-sha", required=True)
    parser.add_argument("--firmware-release", required=True)
    parser.add_argument("--firmware-license")
    arguments = parser.parse_args()
    try:
        package(arguments.donor, arguments.image, arguments.output,
                arguments.source_sha, arguments.firmware_release, arguments.firmware_license)
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        parser.exit(1, str(error) + "\n")
