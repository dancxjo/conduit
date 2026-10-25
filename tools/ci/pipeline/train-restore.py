"""Bounded archive restoration; never delegates unsafe extraction to archive tools."""
import json
import os
from pathlib import Path
import stat
import sys
import zipfile

MAX_TAR = 8 * 1024**3
MAX_FILE = 4 * 1024**3
MAX_MEMBERS = 100000
RESERVE = 4 * 1024**3


def require(ok, reason):
    if not ok:
        raise ValueError(reason)


def reserve(root, extra=0):
    v = os.statvfs(root)
    require(v.f_bavail * v.f_frsize >= RESERVE + extra, 'Restoration reserve exhausted')


def copy_exact(source, output, size, root):
    remaining = size
    while remaining:
        chunk = source.read(min(1024 * 1024, remaining))
        require(chunk, 'Truncated archive')
        reserve(root, len(chunk))
        output.write(chunk)
        remaining -= len(chunk)


def text(value):
    return value.split(b'\0', 1)[0].decode('utf-8', 'strict')


def number(value):
    value = value.rstrip(b'\0 ').lstrip(b' ')
    require(not value or all(c in b'01234567' for c in value), 'Non-octal tar metadata')
    return int(value or b'0', 8)


def pax(data):
    result = {}
    while data:
        sep = data.find(b' ')
        require(0 < sep < 12 and data[:sep].isdigit(), 'Malformed pax length')
        size = int(data[:sep])
        require(sep + 2 < size <= len(data) and data[size - 1:size] == b'\n', 'Malformed pax entry')
        key, value = data[sep + 1:size - 1].split(b'=', 1)
        require(key not in result and key in (b'path', b'mtime', b'atime', b'ctime'), 'Unsupported pax metadata')
        result[key] = value
        data = data[size:]
    return result


def restore(archive, target, directory, maximum_bytes):
    maximum_bytes = int(maximum_bytes)
    require(0 <= maximum_bytes <= MAX_TAR, 'Invalid expansion admission')
    root = Path(directory)
    root.mkdir()  # Fresh target staging directory, preserve on failure.
    archive = Path(archive)
    require(stat.S_ISREG(archive.lstat().st_mode), 'Archive must be regular')
    tarpath = root / 'retained.tar'
    with zipfile.ZipFile(archive) as zip:
        entries = zip.infolist()
        require(len(entries) == 1, 'ZIP must contain exactly one tar')
        entry = entries[0]
        require(entry.filename == target + '.tar' and not entry.is_dir(), 'Wrong ZIP member')
        mode = entry.external_attr >> 16
        require(not mode or stat.S_IFMT(mode) in (0, stat.S_IFREG), 'Special ZIP member')
        require(entry.compress_type in (zipfile.ZIP_STORED, zipfile.ZIP_DEFLATED)
                and not entry.flag_bits & 1 and 0 < entry.file_size <= MAX_TAR, 'Unsupported or oversized ZIP')
        reserve(root, entry.file_size)
        with zip.open(entry) as source, tarpath.open('xb') as output:
            copy_exact(source, output, entry.file_size, root)
            require(not source.read(1), 'ZIP size changed')
    output_root = root / 'bundle'
    output_root.mkdir()
    seen = set()
    pending = None
    count = 0
    total = 0
    with tarpath.open('rb') as source:
        while True:
            header = source.read(512)
            require(len(header) == 512, 'Missing tar terminator')
            if header == bytes(512):
                require(source.read(512) == bytes(512), 'Invalid tar terminator')
                while chunk := source.read(1024 * 1024):
                    require(not any(chunk), 'Trailing tar data')
                require(pending is None, 'Dangling extended tar name')
                break
            count += 1
            require(count <= MAX_MEMBERS, 'Too many tar members')
            require(number(header[148:156]) == sum(header[:148]) + 256 + sum(header[156:]), 'Tar checksum mismatch')
            size = number(header[124:136])
            kind = header[156:157]
            name = text(header[:100])
            prefix = text(header[345:500]) if header[257:263] == b'ustar\0' else ''
            if prefix:
                name = prefix + '/' + name
            if kind in (b'L', b'x'):
                require(pending is None and 0 < size <= 65536, 'Oversized or repeated tar extension')
                data = source.read(size)
                require(len(data) == size, 'Truncated tar extension')
                pending = text(data) if kind == b'L' else pax(data).get(b'path', name.encode()).decode('utf-8')
            else:
                require(kind in (b'0', b'\0', b'5'), 'Tar links and special members refused')
                name = pending if pending is not None else name
                pending = None
                name = name[:-1] if kind == b'5' and name.endswith('/') else name
                parts = name.split('/')
                require(parts[0] == target and all(p and p not in ('.', '..') for p in parts)
                        and '\\' not in name and not name.startswith('/') and ':' not in parts[0], 'Unsafe tar path')
                require(name not in seen, 'Duplicate tar member')
                seen.add(name)
                destination = output_root.joinpath(*parts)
                require(size <= MAX_FILE, 'Oversized tar member')
                total += size
                require(total <= maximum_bytes, 'Expanded target exceeds bound')
                if kind == b'5':
                    require(size == 0, 'Directory with payload')
                    destination.mkdir(parents=True, exist_ok=True)
                else:
                    require(len(parts) > 1, 'Target must be a directory')
                    destination.parent.mkdir(parents=True, exist_ok=True)
                    with destination.open('xb') as output:
                        copy_exact(source, output, size, root)
                    destination.chmod(0o755 if number(header[100:108]) & 0o111 else 0o644)
            padding = (-size) % 512
            require(source.read(padding) == bytes(padding), 'Nonzero tar padding')
    require(seen and (output_root / target).is_dir(), 'Missing target products')
    print(json.dumps({'bytes': total, 'members': count, 'bundle': str(output_root)}))


if __name__ == '__main__':
    restore(*sys.argv[1:])
