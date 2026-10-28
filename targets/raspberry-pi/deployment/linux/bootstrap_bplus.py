#!/usr/bin/env python3
"""Prepare an original B+ ARMv6 Linux Host image; never flash a device."""
import argparse
import hashlib
import json
import lzma
import os
from pathlib import Path
import re
import shutil
import struct
import subprocess
import uuid

import bootstrap as common

SOURCE = 'https://downloads.raspberrypi.com/raspios_lite_armhf/images/raspios_lite_armhf-2026-10-06/2026-10-06-raspios-trixie-armhf-lite.img.xz'
# Official adjacent .sha256 and Raspberry Pi OS download page, checked 2026-10-27.
SOURCE_SHA256 = '2c302771f34c67a0b7ae7aa2e0dcdd3eac660504be59193c011ba79d412e1632'
MANIFEST = 'raspios-model-b-plus-v1.2.json'
EXECUTABLE = 'conduit-linux-armv6'
PROVENANCE = 'linux-armv6-build.json'
TARGET = 'std/armv6/raspberry-pi-model-b-plus-v1.2'
EXPECTED_FILES = {EXECUTABLE: 'application/vnd.conduit.host+executable', PROVENANCE: 'application/json'}
MAXIMUM_FILE_BYTES = 64 * 1024**2
SHA = re.compile(r'sha256:[0-9a-f]{64}')


def unique_keys(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError('duplicate JSON field: ' + key)
        result[key] = value
    return result


def read_json(path):
    common.regular(path)
    if path.stat().st_size > 64 * 1024:
        raise ValueError('JSON metadata exceeds 64 KiB: ' + str(path))
    return json.loads(path.read_text(), object_pairs_hook=unique_keys)


def verify_release(root):
    """Verify and admit only the sealed public ARMv6 binary/provenance pair."""
    if root.is_symlink() or not root.is_dir():
        raise ValueError('release directory must be an ordinary directory')
    manifest = read_json(root / MANIFEST)
    fields = {'schema', 'target_id', 'make_package_id', 'output', 'builder_adapter',
              'deployment_adapter', 'source_identity', 'bundle_sha256', 'files'}
    if not isinstance(manifest, dict) or set(manifest) != fields:
        raise ValueError('unexpected release manifest fields')
    expected = {
        'schema': 'conduit.release/host-bundle@1', 'target_id': TARGET,
        'make_package_id': 'conduit-host-raspberry-pi@1', 'output': 'native-bundle',
        'builder_adapter': 'conduit-host-raspberry-pi/build-raspios-native@1',
        'deployment_adapter': 'conduit-host-raspberry-pi/install-raspios-package@1',
    }
    if any(manifest.get(key) != value for key, value in expected.items()):
        raise ValueError('release is not the exact Raspberry Pi B+ ARMv6 target')
    source = manifest.get('source_identity')
    if not isinstance(source, str) or not source or len(source) > 256 or any(ord(c) < 32 for c in source):
        raise ValueError('invalid release source identity')
    files = manifest.get('files')
    if not isinstance(files, list) or len(files) != 2:
        raise ValueError('release must contain exactly executable and build provenance')
    seen = set()
    bundle = hashlib.sha256(b'conduit.release/host-bundle-content@1\0')
    for file in files:
        if not isinstance(file, dict) or set(file) != {'path', 'bytes', 'sha256', 'media_type'}:
            raise ValueError('unexpected release file fields')
        name = file.get('path')
        if name not in EXPECTED_FILES or name in seen:
            raise ValueError('unsafe, duplicate or unexpected release file path')
        seen.add(name)
        size, digest = file.get('bytes'), file.get('sha256')
        if (type(size) is not int or not 0 < size <= MAXIMUM_FILE_BYTES
                or not isinstance(digest, str) or not SHA.fullmatch(digest)
                or file.get('media_type') != EXPECTED_FILES[name]):
            raise ValueError('invalid release file bounds, digest or media type')
        path = root / name
        common.regular(path)
        if path.stat().st_size != size or 'sha256:' + common.digest(path) != digest:
            raise ValueError('release payload checksum mismatch: ' + name)
        bundle.update(name.encode() + b'\0' + digest.encode() + b'\n')
    if seen != set(EXPECTED_FILES) or manifest.get('bundle_sha256') != 'sha256:' + bundle.hexdigest():
        raise ValueError('release bundle content identity mismatch')
    provenance = read_json(root / PROVENANCE)
    provenance_fields = {'schema', 'source_identity', 'rust_toolchain', 'target', 'cpu',
                         'cross_version', 'toolchain_image', 'c_flags', 'physical_execution_proved'}
    if not isinstance(provenance, dict) or set(provenance) != provenance_fields:
        raise ValueError('unexpected provenance fields')
    if (provenance.get('schema') != 'conduit.release/armv6-build@1'
            or provenance.get('source_identity') != source
            or provenance.get('target') != 'arm-unknown-linux-gnueabihf'
            or provenance.get('cpu') != 'arm1176jzf-s'
            or provenance.get('physical_execution_proved') is not False):
        raise ValueError('ARM1176 provenance mismatch')
    with (root / EXECUTABLE).open('rb') as stream:
        elf = stream.read(52)
    if (len(elf) != 52 or elf[:7] != b'\x7fELF\x01\x01\x01'
            or struct.unpack_from('<H', elf, 18)[0] != 40
            or struct.unpack_from('<H', elf, 16)[0] not in (2, 3)
            or struct.unpack_from('<I', elf, 36)[0] & 0xff000400 != 0x05000400):
        raise ValueError('executable is not little-endian ARM EABI5 hard-float ELF32')
    return manifest


def public_key(path):
    common.regular(path)
    if path.stat().st_size > 1024:
        raise ValueError('public key file bound')
    public = path.read_text().strip()
    if '\n' in public or not public.startswith('ssh-ed25519 '):
        raise ValueError('expected one Ed25519 public key, never a private key')
    subprocess.run(['ssh-keygen', '-l', '-f', str(path)], stdout=subprocess.DEVNULL, check=True)
    return public


def cloud_config(user, hostname, public):
    # Singular user extends the distro default user, retaining Pi OS groups.
    return f'''#cloud-config
hostname: {json.dumps(hostname)}
manage_etc_hosts: true
timezone: America/Los_Angeles
locale: en_US.UTF-8
keyboard:
  layout: us
user:
  name: {json.dumps(user)}
  shell: /bin/bash
  lock_passwd: true
  sudo: ALL=(ALL) NOPASSWD:ALL
  ssh_authorized_keys:
    - {json.dumps(public)}
ssh_pwauth: false
package_update: false
runcmd:
  - [systemctl, enable, --now, ssh]
  - [systemctl, enable, --now, avahi-daemon]
  - [systemctl, set-default, multi-user.target]
  - [sh, /boot/firmware/conduit-bplus/bplus-first-boot.sh, {json.dumps(user)}]
'''


def prepare(args):
    # Admit release content and public-key type before creating any image file.
    manifest = verify_release(args.release_dir)
    public = public_key(args.ssh_public_key)
    if not re.fullmatch(r'[a-z][a-z0-9-]{0,61}[a-z0-9]', args.hostname):
        raise ValueError('hostname must be a single lowercase DNS label')
    if not re.fullmatch(r'[a-z_][a-z0-9_-]{0,30}', args.user) or args.user == 'root':
        raise ValueError('expected a non-root Linux user name')
    common.regular(args.base)
    if common.digest(args.base) != SOURCE_SHA256:
        raise ValueError('official compressed image checksum mismatch')
    sidecars = args.output.with_suffix('.bootstrap')
    checksum = Path(str(args.output) + '.sha256')
    for path in [args.output, sidecars, checksum]:
        if path.exists() or path.is_symlink():
            raise ValueError('refusing to overwrite existing output: ' + str(path))
    tools = {}
    for name in ['mtype', 'mcopy', 'mmd']:
        tool = str(args.mtools_dir / name) if args.mtools_dir else shutil.which(name)
        if not tool or not Path(tool).is_file() or not os.access(tool, os.X_OK):
            raise ValueError('mtools unavailable: ' + name)
        tools[name] = tool
    args.output.parent.mkdir(parents=True, exist_ok=True)
    if shutil.disk_usage(args.output.parent).free < 4 * 1024**3:
        raise ValueError('at least 4 GiB free is required for image preparation')
    sidecars.mkdir()
    payload = sidecars / 'payload'
    payload.mkdir()
    for name in [MANIFEST, EXECUTABLE, PROVENANCE]:
        shutil.copyfile(args.release_dir / name, payload / name)
    # Freeze and reverify the admitted snapshot before touching image bytes.
    verify_release(payload)
    installer = Path(__file__).with_name('bplus-first-boot.sh')
    common.regular(installer)
    shutil.copyfile(installer, payload / installer.name)
    instructions = Path(__file__).with_name('README-BPLUS.md')
    common.regular(instructions)
    shutil.copyfile(instructions, payload / instructions.name)
    sums = ''.join(common.digest(path) + '  ' + path.name + '\n'
                   for path in sorted(payload.iterdir()))
    (payload / 'SHA256SUMS').write_text(sums)
    with args.output.open('xb') as output, lzma.open(args.base, 'rb') as source:
        shutil.copyfileobj(source, output)
    with args.output.open('rb') as image:
        mbr = image.read(512)
    if len(mbr) != 512 or mbr[510:512] != b'\x55\xaa' or mbr[450] not in (0x0b, 0x0c):
        raise ValueError('expected the official FAT boot partition')
    start, sectors = struct.unpack_from('<II', mbr, 454)
    offset = start * 512
    if not start or not sectors or (start + sectors) * 512 > args.output.stat().st_size:
        raise ValueError('invalid FAT partition extent')
    image_arg = str(args.output.resolve()) + '@@' + str(offset)
    subprocess.run([tools['mtype'], '-i', image_arg, '::bcm2708-rpi-b-plus.dtb'],
                   stdout=subprocess.DEVNULL, check=True)
    # This pinned image must actually carry Raspberry Pi OS cloud-init files.
    for name in ['user-data', 'meta-data', 'network-config']:
        subprocess.run([tools['mtype'], '-i', image_arg, '::' + name],
                       stdout=subprocess.DEVNULL, check=True)
    subprocess.run([tools['mmd'], '-i', image_arg, '::conduit-bplus'], check=True)
    for path in payload.iterdir():
        target = '::conduit-bplus/' + path.name
        subprocess.run([tools['mcopy'], '-i', image_arg, str(path), target], check=True)
        if hashlib.sha256(subprocess.check_output([tools['mtype'], '-i', image_arg, target])).hexdigest() != common.digest(path):
            raise ValueError('FAT payload verification failed: ' + path.name)
    files = {
        'user-data': cloud_config(args.user, args.hostname, public),
        'meta-data': f'instance-id: {uuid.uuid4()}\nlocal-hostname: {json.dumps(args.hostname)}\n',
        'network-config': 'network:\n  version: 2\n  renderer: NetworkManager\n  ethernets:\n    eth0:\n      dhcp4: true\n      optional: false\n',
    }
    for name, text in files.items():
        path = sidecars / name
        path.write_text(text)
        subprocess.run([tools['mcopy'], '-o', '-i', image_arg, str(path), '::' + name], check=True)
        if subprocess.check_output([tools['mtype'], '-i', image_arg, '::' + name]) != path.read_bytes():
            raise ValueError('FAT cloud-init verification failed: ' + name)
    image_digest = common.digest(args.output)
    receipt = {
        'schema': 'conduit.raspberry-pi/linux-bplus-bootstrap@1',
        'source_url': SOURCE, 'source_xz_sha256': SOURCE_SHA256,
        'image_sha256': image_digest, 'image_bytes': args.output.stat().st_size,
        'hostname': args.hostname, 'user': args.user, 'target_id': TARGET,
        'release_bundle_sha256': manifest['bundle_sha256'],
        'release_source_identity': manifest['source_identity'],
        'release_manifest_sha256': common.digest(payload / MANIFEST),
        'ssh_public_key_sha256': hashlib.sha256(public.encode()).hexdigest(),
        'fat_boot_offset': offset, 'fat_payload_roundtrip_verified': True,
        'private_key_included': False, 'host_identity_preseeded': False,
        'body_invitation_preseeded': False, 'conduit_release_preloaded': True,
        'conduit_host_initialized': False, 'physical_boot_verified': False,
        'minipitft_linux_adapter_proved': False, 'camera_linux_adapter_proved': False,
    }
    (sidecars / 'receipt.json').write_text(json.dumps(receipt, indent=2) + '\n')
    with checksum.open('x') as output:
        output.write(image_digest + '  ' + args.output.name + '\n')
    print(json.dumps(receipt))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ['base', 'output', 'ssh-public-key', 'release-dir']:
        parser.add_argument('--' + name, type=Path, required=True)
    parser.add_argument('--mtools-dir', type=Path)
    parser.add_argument('--hostname', default='conduit-bplus')
    parser.add_argument('--user', default='dancxjo')
    prepare(parser.parse_args())
