"""Managed compiler objects only. Never restore executable proof authority."""
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import sys
import tarfile
import zipfile

MAX_BYTES = 4 * 1024**3
MAX_TAR = MAX_BYTES + 128 * 1024**2
MAX_JSON = 32 * 1024**2
MAX_FILES = 100000
RESERVE = 1024**3


def require(ok, message):
    if not ok:
        raise ValueError(message)


def regular(file):
    st = file.lstat()
    require(stat.S_ISREG(st.st_mode) and st.st_nlink == 1 and st.st_uid == os.getuid(), 'Unsafe seed file')
    return st


def digest(file):
    h = hashlib.sha256()
    with file.open('rb') as source:
        while data := source.read(1024 * 1024):
            h.update(data)
    return h.hexdigest()


def reserve(root, extra=0):
    st = os.statvfs(root)
    require(st.f_bavail * st.f_frsize >= RESERVE + extra, 'Seed disk reserve unavailable')


def inventory(root, maximum):
    require(set(p.name for p in root.iterdir()) == {'policy.json', 'objects', '.cargo-lease'}, 'Unmanaged cache seed root')
    require(regular(root / 'policy.json').st_size <= 16384, 'Seed policy bound')
    objects = root / 'objects'
    st = objects.lstat()
    require(stat.S_ISDIR(st.st_mode) and st.st_uid == os.getuid(), 'Unsafe seed objects')
    files = []
    total = 0
    count = 0

    def walk(directory):
        nonlocal total, count
        for file in sorted(directory.iterdir()):
            count += 1
            require(count <= MAX_FILES, 'Seed member bound')
            rel = file.relative_to(root).as_posix()
            st = file.lstat()
            require(st.st_uid == os.getuid(), 'Seed owner mismatch')
            if stat.S_ISDIR(st.st_mode):
                require(re.fullmatch(r'objects/[a-f0-9](/[a-f0-9])?', rel), 'Unmanaged seed directory')
                walk(file)
            else:
                regular(file)
                match = re.fullmatch(r'objects/([a-f0-9])/([a-f0-9])/([a-f0-9]{64})', rel)
                require(match and match[3].startswith(match[1] + match[2]), 'Unmanaged seed object')
                total += st.st_size
                require(total <= maximum, 'Seed byte bound')
                files.append({'path': rel, 'bytes': st.st_size, 'sha256': digest(file)})
    walk(objects)
    return files, total


def inactive(root):
    identities = {(root.stat().st_dev, root.stat().st_ino), ((root / 'objects').stat().st_dev, (root / 'objects').stat().st_ino)}
    for file in (root / 'objects').rglob('*'):
        st = file.lstat()
        identities.add((st.st_dev, st.st_ino))
    for process in Path('/proc').iterdir():
        if not process.name.isdigit() or process.name == str(os.getpid()):
            continue
        cmd = []
        try:
            if process.stat().st_uid != os.getuid():
                continue
            cmd = (process / 'cmdline').read_bytes().split(b'\0')
            if cmd and Path(os.fsdecode(cmd[0])).name == 'sccache':
                try:
                    mounted = (process / 'root/cache').stat()
                    require((mounted.st_dev, mounted.st_ino) not in identities, 'Seed engine active')
                except (FileNotFoundError, ProcessLookupError):
                    pass
                env = (process / 'environ').read_bytes().split(b'\0')
                for setting in env:
                    if setting.startswith(b'SCCACHE_DIR='):
                        selected = Path(os.fsdecode(setting.split(b'=', 1)[1]))
                        if selected.is_absolute() and selected.exists():
                            require(selected.resolve() not in [root, root / 'objects'], 'Seed engine active')
            try:
                descriptors = list((process / 'fd').iterdir())
            except PermissionError:
                require(not cmd or Path(os.fsdecode(cmd[0])).name != 'sccache', 'Seed engine audit incomplete')
                continue
            for fd in descriptors:
                try:
                    st = fd.stat()
                    require((st.st_dev, st.st_ino) not in identities, 'Seed objects active')
                except (FileNotFoundError, ProcessLookupError, PermissionError):
                    continue
        except (FileNotFoundError, ProcessLookupError):
            continue
        except PermissionError:
            # Unknown engines are not safe to publish as inactive.
            if cmd and Path(os.fsdecode(cmd[0])).name == 'sccache':
                raise ValueError('Seed engine audit incomplete')


def seal(request):
    root = Path(request['cacheRoot'])
    maximum = request['maximum']
    require(0 < maximum <= MAX_BYTES, 'Seed policy byte bound')
    lease = root / '.cargo-lease'
    lease.mkdir(mode=0o700)
    try:
        require((root / 'policy.json').read_text() == request['policyText'], 'Seed policy mismatch')
        files, total = inventory(root, maximum)
        inactive(root)
        receipt = dict(request['receipt'], files=files, bytes=total)
        data = json.dumps(receipt, separators=(',', ':')).encode()
        require(len(data) <= MAX_JSON, 'Seed receipt bound')
        destination = Path(request['archive'])
        reserve(destination.parent, total + len(data) + 64 * 1024**2)
        with destination.open('xb') as output:
            with tarfile.open(fileobj=output, mode='w', format=tarfile.USTAR_FORMAT) as archive:
                import io
                info = tarfile.TarInfo('seed.json'); info.size = len(data); info.mode = 0o600
                archive.addfile(info, io.BytesIO(data))
                for rel in ['policy.json'] + [f['path'] for f in files]:
                    file = root / rel
                    st = regular(file)
                    info = tarfile.TarInfo(rel); info.size = st.st_size; info.mode = 0o600
                    with file.open('rb') as source:
                        archive.addfile(info, source)
        require(inventory(root, maximum)[0] == files, 'Seed changed during sealing')
        require((root / 'policy.json').read_text() == request['policyText'], 'Seed policy changed')
        inactive(root)
        require(destination.stat().st_size <= MAX_TAR, 'Seed tar bound')
        return receipt
    finally:
        lease.rmdir()


def restore(request):
    require(type(request.get('maximum')) is int and 0 < request['maximum'] <= MAX_BYTES, 'Seed policy byte bound')
    directory = Path(request['directory'])
    directory.mkdir(mode=0o700)
    archive_file = Path(request['archive'])
    regular(archive_file)
    tar = directory / 'seed.tar'
    with zipfile.ZipFile(archive_file) as archive:
        entries = archive.infolist()
        require(len(entries) == 1, 'Seed ZIP member count')
        entry = entries[0]
        mode = entry.external_attr >> 16
        require(entry.filename == 'seed.tar' and not entry.is_dir() and (not mode or stat.S_IFMT(mode) in (0, stat.S_IFREG)), 'Seed ZIP member')
        require(not entry.flag_bits & 1 and entry.compress_type in (zipfile.ZIP_STORED, zipfile.ZIP_DEFLATED) and 0 < entry.file_size <= MAX_TAR, 'Seed ZIP bound/type')
        reserve(directory, entry.file_size + request['maximum'])
        with archive.open(entry) as source, tar.open('xb') as output:
            remaining = entry.file_size
            while remaining:
                data = source.read(min(1024**2, remaining))
                require(data, 'Truncated seed ZIP')
                output.write(data); remaining -= len(data)
            require(not source.read(1), 'Seed ZIP expansion changed')
    # Check raw fixed-width sizes before tarfile's parser can suppress malformed
    # headers or interpret GNU signed/base256 sizes. This protocol emits USTAR
    # octal sizes only, and never admits a negative read/seek length.
    with tar.open('rb') as raw:
        count = 0
        tar_bytes = tar.stat().st_size
        while True:
            header = raw.read(512)
            require(len(header) == 512, 'Seed tar header truncated')
            if header == bytes(512):
                require(raw.read(512) == bytes(512), 'Seed tar terminator')
                while tail := raw.read(1024**2):
                    require(not any(tail), 'Seed tar trailing payload')
                break
            count += 1
            require(count <= MAX_FILES + 2, 'Seed tar member bound')
            field = header[124:136].strip(b'\0 ')
            require(all(char in b'01234567' for char in field), 'Seed tar size bound')
            size = int(field or b'0', 8)
            require(0 <= size <= MAX_BYTES, 'Seed tar size bound')
            padded = ((size + 511) // 512) * 512
            require(raw.tell() + padded + 1024 <= tar_bytes, 'Seed tar member exceeds archive')
            raw.seek(padded, 1)
    root = directory / 'cache'; root.mkdir(mode=0o700)
    (root / 'objects').mkdir(mode=0o700)
    with tarfile.open(tar, mode='r:') as archive:
        members = []
        seen = set()
        total = 0
        expected_offset = 0
        # Stream member inventory; reject all extensions and special members.
        for member in archive:
            require(member.isreg() and not member.pax_headers and not member.sparse, 'Seed tar special member')
            require(type(member.size) is int and 0 <= member.size <= MAX_BYTES, 'Seed tar size bound')
            require(member.offset == expected_offset and member.offset_data == member.offset + 512, 'Seed tar extension refused')
            expected_offset = member.offset_data + ((member.size + 511) // 512) * 512
            require(len(members) < MAX_FILES + 2, 'Seed tar member bound')
            require(member.name not in seen, 'Duplicate seed member')
            seen.add(member.name)
            if member.name == 'seed.json':
                require(member.size <= MAX_JSON, 'Seed receipt bound')
            elif member.name == 'policy.json':
                require(member.size <= 16384, 'Seed marker bound')
            else:
                match = re.fullmatch(r'objects/([a-f0-9])/([a-f0-9])/([a-f0-9]{64})', member.name)
                require(match and match[3].startswith(match[1] + match[2]), 'Seed tar path')
                total += member.size
                require(total <= request['maximum'], 'Seed expansion bound')
            members.append(member)
        with tar.open('rb') as raw:
            raw.seek(expected_offset)
            tail = raw.read(1024)
            require(tail == bytes(1024), 'Seed tar terminator')
            while tail := raw.read(1024**2):
                require(not any(tail), 'Seed tar trailing payload')
        names = {m.name for m in members}
        require('seed.json' in names and 'policy.json' in names, 'Missing seed metadata')
        receipt = json.loads(archive.extractfile(archive.getmember('seed.json')).read(MAX_JSON + 1))
        for key, value in request['expected'].items():
            require(receipt.get(key) == value, 'Seed producer receipt mismatch: ' + key)
        files = receipt.get('files')
        require(isinstance(files, list) and len(files) <= MAX_FILES, 'Seed object inventory')
        for file in files:
            require(isinstance(file, dict) and set(file) == {'path', 'bytes', 'sha256'}, 'Seed object receipt shape')
            require(isinstance(file['path'], str) and isinstance(file['sha256'], str), 'Seed object receipt fields')
            require(type(file['bytes']) is int and 0 <= file['bytes'] <= request['maximum'], 'Seed object byte bound')
        require(names == {'seed.json', 'policy.json'} | {f['path'] for f in files} and len(names) == len(files) + 2, 'Seed inventory differs')
        require(type(receipt.get('bytes')) is int and 0 <= receipt['bytes'] <= request['maximum'] and receipt['bytes'] == total, 'Seed total bytes differs')
        require(archive.extractfile(archive.getmember('policy.json')).read(16385).decode() == request['policyText'], 'Seed marker differs')
        for file in files:
            member = archive.getmember(file['path'])
            require(member.size == file['bytes'] and re.fullmatch('[a-f0-9]{64}', file['sha256']), 'Seed object receipt')
            destination = root / file['path']; destination.parent.mkdir(parents=True, exist_ok=True)
            h = hashlib.sha256()
            with archive.extractfile(member) as source, destination.open('xb') as output:
                remaining = member.size
                while remaining > 0:
                    reserve(directory)
                    data = source.read(min(1024**2, remaining)); require(data, 'Truncated seed object')
                    output.write(data); h.update(data); remaining -= len(data)
                require(not source.read(1) and h.hexdigest() == file['sha256'], 'Seed object digest differs')
        (root / 'policy.json').write_text(request['policyText'])
    return receipt


request = json.load(sys.stdin)
result = seal(request) if sys.argv[1] == 'seal' else restore(request)
print(json.dumps(result, separators=(',', ':')))
