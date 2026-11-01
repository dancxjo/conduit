import { parseRp2040Uf2, RP2040_UF2 } from '../browser/uf2.mjs';
export function verifyPayload(bytes) {
  const parsed = parseRp2040Uf2(bytes, 4096);
  if (parsed.endAddress > RP2040_UF2.firmwareEnd) {
    throw new Error('Pico payload overlaps retained credentials or Body bootstrap');
  }
  return { bytes: bytes.byteLength, end_address: parsed.endAddress, reserved_regions_preserved: true };
}
if (process.argv[1] && (await import('node:url')).pathToFileURL(process.argv[1]).href === import.meta.url) {
  const { stat, readFile } = await import('node:fs/promises');
  const file = process.argv[2];
  if (!file || (await stat(file)).size > 4 * 1024 * 1024) throw new Error('Pico UF2 extent exceeds its bound');
  verifyPayload(new Uint8Array(await readFile(file)));
}
