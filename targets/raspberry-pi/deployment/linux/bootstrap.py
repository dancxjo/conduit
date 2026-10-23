#!/usr/bin/env python3
"""Prepare an Ethernet/SSH Pi OS Lite bootstrap, not a ready appliance Host."""
import argparse
import hashlib
import json
import lzma
from pathlib import Path
import re
import shutil
import stat
import struct
import subprocess
import uuid

SOURCE = 'https://downloads.raspberrypi.com/raspios_lite_arm64/images/raspios_lite_arm64-2026-10-06/2026-10-06-raspios-trixie-arm64-lite.img.xz'
SOURCE_SHA256 = '483db18a48da399b5b7022ffae9a07bc6d99daab30e593aedac09b69ff422843'


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def regular(path):
    if path.is_symlink() or not stat.S_ISREG(path.stat().st_mode):
        raise ValueError('expected an ordinary file: ' + str(path))


def prepare(args):
    regular(args.base)
    regular(args.ssh_public_key)
    if digest(args.base) != SOURCE_SHA256:
        raise ValueError('official compressed image checksum mismatch')
    if args.ssh_public_key.stat().st_size > 1024:
        raise ValueError('public key file bound')
    public = args.ssh_public_key.read_text().strip()
    if len(public) > 1024 or '\n' in public or not public.startswith('ssh-ed25519 '):
        raise ValueError('expected one Ed25519 public key, never a private key')
    # ssh-keygen checks the actual key encoding before it enters boot media.
    subprocess.run(['ssh-keygen', '-l', '-f', str(args.ssh_public_key)],
                   stdout=subprocess.DEVNULL, check=True)
    if not re.fullmatch(r'[a-z][a-z0-9-]{0,61}[a-z0-9]', args.hostname):
        raise ValueError('hostname must be a single lowercase DNS label')
    if not re.fullmatch(r'[a-z_][a-z0-9_-]{0,30}', args.user):
        raise ValueError('invalid Linux user name')
    sidecars = args.output.with_suffix('.bootstrap')
    if sidecars.exists() or sidecars.is_symlink():
        raise ValueError('refusing to overwrite existing bootstrap sidecars')
    if args.output.exists() or args.output.is_symlink():
        raise ValueError('refusing to overwrite existing output')
    args.output.parent.mkdir(parents=True, exist_ok=True)
    if shutil.disk_usage(args.output.parent).free < 4 * 1024**3:
        raise ValueError('at least 4 GiB free is required for bootstrap preparation')
    tools = {}
    for name in ['mtype', 'mcopy']:
        value = str(args.mtools_dir / name) if args.mtools_dir else shutil.which(name)
        if not value or not Path(value).is_file():
            raise ValueError('mtools unavailable: ' + name)
        tools[name] = value
    # Exclusive creation prevents accidentally writing a block device or an
    # existing image. This command never flashes removable or internal media.
    with args.output.open('xb') as output, lzma.open(args.base, 'rb') as source:
        shutil.copyfileobj(source, output)
    with args.output.open('rb') as image:
        mbr = image.read(512)
    if mbr[510:512] != b'\x55\xaa' or mbr[450] not in (0x0b, 0x0c):
        raise ValueError('expected the official FAT boot partition')
    start, sectors = struct.unpack_from('<II', mbr, 454)
    offset = start * 512
    if offset == 0 or (start + sectors) * 512 > args.output.stat().st_size:
        raise ValueError('invalid FAT partition extent')
    image_arg = str(args.output.resolve()) + '@@' + str(offset)
    # Confirm this image actually carries the chosen board's device tree.
    subprocess.run([tools['mtype'], '-i', image_arg, '::bcm2710-rpi-3-b-plus.dtb'],
                   stdout=subprocess.DEVNULL, check=True)
    cloud = f'''#cloud-config
hostname: {args.hostname}
manage_etc_hosts: true
timezone: America/Los_Angeles
locale: en_US.UTF-8
keyboard:
  layout: us
user:
  name: {args.user}
  shell: /bin/bash
  lock_passwd: true
  ssh_authorized_keys:
    - {json.dumps(public)}
ssh_pwauth: false
package_update: true
packages:
  - libsdl2-2.0-0
  - libgbm1
  - libegl1
  - libgl1-mesa-dri
  - avahi-daemon
runcmd:
  - [systemctl, enable, --now, ssh]
  - [systemctl, enable, --now, avahi-daemon]
  - [systemctl, set-default, multi-user.target]
  - [systemctl, mask, display-manager.service]
  - [install, -d, -o, {args.user}, -g, {args.user}, -m, '0700', /var/lib/conduit]
'''
    # This identifies cloud-init's image instance, not a Conduit Host or Body.
    instance = str(uuid.uuid4())
    sidecars.mkdir(exist_ok=False)
    network = 'network:\n  version: 2\n  renderer: NetworkManager\n  ethernets:\n    eth0:\n      dhcp4: true\n      optional: false\n'
    files = {'user-data': cloud, 'network-config': network,
             'meta-data': f'instance-id: {instance}\nlocal-hostname: {args.hostname}\n'}
    for name, text in files.items():
        path = sidecars / name
        path.write_text(text)
        subprocess.run([tools['mcopy'], '-o', '-i', image_arg, str(path), '::' + name], check=True)
        actual = subprocess.check_output([tools['mtype'], '-i', image_arg, '::' + name])
        if actual != path.read_bytes():
            raise ValueError('FAT verification failed: ' + name)
    receipt = {
        'schema': 'conduit.raspberry-pi/linux-bootstrap@1',
        'source_url': SOURCE, 'source_xz_sha256': SOURCE_SHA256,
        'image_sha256': digest(args.output), 'image_bytes': args.output.stat().st_size,
        'hostname': args.hostname, 'user': args.user,
        'ssh_public_key_sha256': hashlib.sha256(public.encode()).hexdigest(),
        'fat_boot_offset': offset, 'cloud_init_fat_roundtrip_verified': True,
        'ethernet': 'eth0 DHCP; required for first-boot package installation', 'wifi_credentials_included': False,
        'private_key_included': False, 'desktop': False,
        'conduit_host_initialized': False, 'graphics_client_installed': False,
        'physical_boot_verified': False, 'appliance_image_complete': False,
        'first_boot_packages': 'distribution repositories; Ethernet required',
    }
    (sidecars / 'receipt.json').write_text(json.dumps(receipt, indent=2) + '\n')
    print(json.dumps(receipt))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--base', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--ssh-public-key', type=Path, required=True)
    parser.add_argument('--hostname', default='climate-pi')
    parser.add_argument('--user', default='dancxjo')
    parser.add_argument('--mtools-dir', type=Path)
    prepare(parser.parse_args())
