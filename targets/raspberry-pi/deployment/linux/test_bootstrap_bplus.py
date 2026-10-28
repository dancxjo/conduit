"""Deterministic payload/corruption/refusal proof; never mounts or flashes media."""
import argparse
import contextlib
import hashlib
import io
import json
import lzma
from pathlib import Path
import struct
import subprocess
import tempfile
import types
import unittest
from unittest import mock

import bootstrap_bplus as subject


class BootstrapTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.release = self.root / 'release'
        self.release.mkdir()
        elf = bytearray(52)
        elf[:7] = b'\x7fELF\x01\x01\x01'
        struct.pack_into('<HH', elf, 16, 3, 40)
        struct.pack_into('<I', elf, 36, 0x05000400)
        (self.release / subject.EXECUTABLE).write_bytes(elf)
        (self.release / subject.PROVENANCE).write_text(json.dumps({
            'schema': 'conduit.release/armv6-build@1', 'source_identity': 'git/fixture',
            'target': 'arm-unknown-linux-gnueabihf', 'cpu': 'arm1176jzf-s',
            'rust_toolchain': '1.98.1', 'cross_version': '0.2.5', 'toolchain_image': 'fixture',
            'c_flags': '-march=armv6 -marm -mfpu=vfp -mfloat-abi=hard',
            'physical_execution_proved': False,
        }))
        self.manifest = {
            'schema': 'conduit.release/host-bundle@1', 'target_id': subject.TARGET,
            'make_package_id': 'conduit-host-raspberry-pi@1', 'output': 'native-bundle',
            'builder_adapter': 'conduit-host-raspberry-pi/build-raspios-native@1',
            'deployment_adapter': 'conduit-host-raspberry-pi/install-raspios-package@1',
            'source_identity': 'git/fixture', 'files': [], 'bundle_sha256': '',
        }
        self.seal()

    def tearDown(self):
        self.temporary.cleanup()

    def seal(self):
        files = []
        bundle = hashlib.sha256(b'conduit.release/host-bundle-content@1\0')
        for name, media in subject.EXPECTED_FILES.items():
            data = (self.release / name).read_bytes()
            digest = 'sha256:' + hashlib.sha256(data).hexdigest()
            files.append({'path': name, 'bytes': len(data), 'sha256': digest, 'media_type': media})
            bundle.update(name.encode() + b'\0' + digest.encode() + b'\n')
        self.manifest.update(files=files, bundle_sha256='sha256:' + bundle.hexdigest())
        self.save()

    def save(self):
        (self.release / subject.MANIFEST).write_text(json.dumps(self.manifest))

    def test_exact_sealed_payload_accepts_but_mutated_bytes_refuse(self):
        self.assertEqual(subject.verify_release(self.release), self.manifest)
        with (self.release / subject.EXECUTABLE).open('ab') as stream:
            stream.write(b'changed')
        with self.assertRaisesRegex(ValueError, 'checksum mismatch'):
            subject.verify_release(self.release)

    def test_path_escape_duplicate_and_additional_identity_fields_refuse(self):
        for name in ['../private.key', '/etc/shadow', 'installation.json', subject.PROVENANCE]:
            with self.subTest(name=name):
                self.manifest['files'][0]['path'] = name
                self.save()
                with self.assertRaises(ValueError):
                    subject.verify_release(self.release)
        self.seal()
        self.manifest['host_id'] = 'forbidden-preseed'
        self.save()
        with self.assertRaisesRegex(ValueError, 'unexpected release manifest'):
            subject.verify_release(self.release)

    def test_symlink_and_wrong_target_and_bundle_refuse(self):
        binary = self.release / subject.EXECUTABLE
        actual = self.root / 'elsewhere'
        binary.rename(actual)
        binary.symlink_to(actual)
        with self.assertRaises(ValueError):
            subject.verify_release(self.release)
        binary.unlink()
        actual.rename(binary)
        self.manifest['target_id'] = 'std/aarch64/wrong-board'
        self.save()
        with self.assertRaisesRegex(ValueError, 'exact Raspberry Pi'):
            subject.verify_release(self.release)
        self.manifest['target_id'] = subject.TARGET
        self.manifest['bundle_sha256'] = 'sha256:' + '0' * 64
        self.save()
        with self.assertRaisesRegex(ValueError, 'bundle content'):
            subject.verify_release(self.release)

    def test_resealed_wrong_cpu_or_elf_abi_refuse(self):
        provenance = self.release / subject.PROVENANCE
        value = json.loads(provenance.read_text())
        value['cpu'] = 'cortex-a7'
        provenance.write_text(json.dumps(value))
        self.seal()
        with self.assertRaisesRegex(ValueError, 'provenance'):
            subject.verify_release(self.release)
        value['cpu'] = 'arm1176jzf-s'
        provenance.write_text(json.dumps(value))
        binary = self.release / subject.EXECUTABLE
        data = bytearray(binary.read_bytes())
        struct.pack_into('<I', data, 36, 0x05000200)
        binary.write_bytes(data)
        self.seal()
        with self.assertRaisesRegex(ValueError, 'hard-float ELF32'):
            subject.verify_release(self.release)

    def arguments(self):
        args = argparse.Namespace(release_dir=self.release, output=self.root / 'out.img',
                                  base=self.root / 'base.xz', ssh_public_key=self.root / 'key.pub',
                                  hostname='conduit-bplus', user='dancxjo', mtools_dir=self.root / 'tools')
        args.base.write_bytes(b'not the pinned official image')
        return args

    def test_checksum_refusal_occurs_before_image_creation(self):
        args = self.arguments()
        with mock.patch.object(subject, 'public_key', return_value='ssh-ed25519 fixture'):
            with self.assertRaisesRegex(ValueError, 'compressed image checksum'):
                subject.prepare(args)
        self.assertFalse(args.output.exists())
        self.assertFalse(args.output.with_suffix('.bootstrap').exists())

    def test_small_fixture_image_roundtrips_payload_and_readme_without_host_identity(self):
        args = self.arguments()
        args.mtools_dir.mkdir()
        for name in ['mtype', 'mcopy', 'mmd']:
            tool = args.mtools_dir / name
            tool.write_text('#!/bin/sh\nexit 99\n')
            tool.chmod(0o755)
        image = bytearray(4096)
        image[510:512] = b'\x55\xaa'
        image[450] = 0x0c
        struct.pack_into('<II', image, 454, 1, 7)
        args.base.write_bytes(lzma.compress(image))
        fat = {name: b'base fixture' for name in ['::bcm2708-rpi-b-plus.dtb', '::user-data', '::meta-data', '::network-config']}
        def run(command, **_):
            name = Path(command[0]).name
            if name == 'mtype' and command[-1] not in fat:
                raise subprocess.CalledProcessError(1, command)
            if name == 'mcopy':
                fat[command[-1]] = Path(command[-2]).read_bytes()
            return subprocess.CompletedProcess(command, 0)
        with mock.patch.object(subject, 'SOURCE_SHA256', subject.common.digest(args.base)), \
                mock.patch.object(subject, 'public_key', return_value='ssh-ed25519 fixture'), \
                mock.patch.object(subject.shutil, 'disk_usage', return_value=types.SimpleNamespace(free=10*1024**3)), \
                mock.patch.object(subject.subprocess, 'run', side_effect=run), \
                mock.patch.object(subject.subprocess, 'check_output', side_effect=lambda cmd: fat[cmd[-1]]), \
                contextlib.redirect_stdout(io.StringIO()):
            subject.prepare(args)
        receipt = json.loads((args.output.with_suffix('.bootstrap') / 'receipt.json').read_text())
        self.assertFalse(receipt['host_identity_preseeded'])
        self.assertFalse(receipt['physical_boot_verified'])
        self.assertEqual(receipt['image_sha256'], subject.common.digest(args.output))
        self.assertIn('::conduit-bplus/README-BPLUS.md', fat)
        self.assertIn(b'README-BPLUS.md', fat['::conduit-bplus/SHA256SUMS'])
        cloud = fat['::user-data'].decode()
        self.assertIn('\nuser:\n', cloud)
        self.assertNotIn('\nusers:\n', cloud)
        self.assertIn('avahi-daemon', cloud)
        self.assertIn('bplus-first-boot.sh', cloud)
        self.assertIn('package_update: false', cloud)


if __name__ == '__main__':
    unittest.main()
