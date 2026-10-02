import { readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { assertDigest, copyFile, xtask } from './common.mjs';
import { activateXtensa } from './setup.mjs';

export function embedded(id, directory) {
  if (id.startsWith('esp32-')) {
    const chip = id.slice('esp32-'.length);
    if (chip !== 'c3') activateXtensa();
    xtask('make', 'esp32-firmware', 'build', '--target', chip,
      '--browser-artifact', path.join(directory, `${id}.bin`),
      '--receipt', path.join(directory, 'firmware-build.json'));
    const receipt = JSON.parse(readFileSync(path.join(directory, 'firmware-build.json'), 'utf8'));
    assertDigest(receipt.artifact, receipt.artifact_sha256);
    copyFile(receipt.artifact, path.join(directory, `${id}.elf`));
    const image = JSON.parse(readFileSync(path.join(directory, `${id}.json`), 'utf8'));
    assertDigest(path.join(directory, `${id}.bin`), image.artifact_sha256);
  } else if (id === 'avr') {
    xtask('make', 'avr', 'release', '--output', directory);
    const manifest = JSON.parse(readFileSync(path.join(directory, 'avr-promicro-atmega32u4-5v-16mhz.json'), 'utf8'));
    assertDigest(path.join(directory, manifest.artifact.path), manifest.artifact.sha256);
  } else if (id === 'raspberry-pi') {
    for (const board of ['rpi-b-plus-v1.2', 'rpi-zero-v1', 'rpi-zero-w-v1.1', 'rpi-zero-wh-v1.1']) {
      xtask('make', 'host', 'rpi', '--board', board, 'image');
      const slug = board === 'rpi-b-plus-v1.2' ? 'rpi-b-plus' : board;
      const source = path.join(process.env.CONDUIT_CONDUITOS_TARGET_ROOT || 'target/conduitos', 'armv6');
      for (const name of [`conduitos-${slug}.img`, `${slug}-image.json`]) {
        copyFile(path.join(source, name), path.join(directory, name));
      }
      verifyImageManifest(directory, `${slug}-image.json`);
    }
  } else if (id === 'orange-pi') {
    xtask('make', 'conduitos', 'orange-pi5-image');
    const source = path.join(process.env.CONDUIT_CONDUITOS_TARGET_ROOT || 'target/conduitos', 'aarch64/orange-pi-5');
    for (const name of ['conduitos-orange-pi-5.img', 'orange-pi-5-image.json']) {
      copyFile(path.join(source, name), path.join(directory, name));
    }
    verifyImageManifest(directory, 'orange-pi-5-image.json');
  } else if (id === 'rp2040') {
    xtask('make', 'pico', 'build');
    const root = 'targets/rp2040/firmware/pico-w-signal/target/thumbv6m-none-eabi/release';
    for (const suffix of ['', '.uf2', '.identity.json', '.generated-image.json']) {
      const name = `conduit-pico-w-signal${suffix}`;
      copyFile(path.join(root, name), path.join(directory, name));
    }
    const identity = JSON.parse(readFileSync(path.join(directory, 'conduit-pico-w-signal.identity.json'), 'utf8'));
    assertDigest(path.join(directory, 'conduit-pico-w-signal'), identity.firmware_sha256);
    verifyUf2(path.join(directory, 'conduit-pico-w-signal.uf2'));
  } else throw new Error(`Unknown embedded lane: ${id}`);
  writeFileSync(path.join(directory, 'proof-scope.txt'),
    'Build and deterministic artifact validation only. No physical boot, device interaction, or HIL is claimed.\n');
}

function verifyImageManifest(directory, name) {
  const manifest = JSON.parse(readFileSync(path.join(directory, name), 'utf8'));
  assertDigest(path.join(directory, manifest.artifact.path), manifest.artifact.sha256);
}

function verifyUf2(file) {
  const bytes = readFileSync(file);
  if (!bytes.length || bytes.length % 512) throw new Error('Invalid UF2 length');
  const blocks = bytes.length / 512;
  for (let n = 0; n < blocks; n++) {
    const block = bytes.subarray(n * 512, (n + 1) * 512);
    if (block.readUInt32LE(0) !== 0x0a324655 || block.readUInt32LE(4) !== 0x9e5d5157 ||
        block.readUInt32LE(508) !== 0x0ab16f30 || block.readUInt32LE(20) !== n ||
        block.readUInt32LE(24) !== blocks || block.readUInt32LE(16) > 476) {
      throw new Error(`Invalid UF2 block ${n}`);
    }
  }
}
