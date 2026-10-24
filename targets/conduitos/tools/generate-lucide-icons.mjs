// Internal preparation tool; consumes the repository's pinned Chromium and SVGs.
// Runtime uses the committed, fixed-size masks and never parses an SVG.
import { createRequire } from 'node:module';
import { readFile, writeFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
const require = createRequire(new URL('../../../proof/browser/package.json', import.meta.url));
const { chromium } = require('playwright');
const assets = new URL('../assets/icons/', import.meta.url);
const source = new URL('../../../mechanisms/implementations/bounded-lucide/svg/', import.meta.url);
const mapping = JSON.parse(await readFile(new URL('lucide-map.json', assets)));
const browser = await chromium.launch({ headless: true });
try {
  const page = await browser.newPage();
  const masks = [];
  const inputs = {};
  for (const name of Object.values(mapping)) {
    const svg = await readFile(new URL(`${name}.svg`, source), 'utf8');
    inputs[name] = createHash('sha256').update(svg).digest('hex');
    const bytes = await page.evaluate(async svg => {
      const image = new Image();
      image.src = `data:image/svg+xml,${encodeURIComponent(svg)}`;
      await image.decode();
      const canvas = document.createElement('canvas');
      canvas.width = canvas.height = 192;
      const context = canvas.getContext('2d');
      context.drawImage(image, 0, 0, 192, 192);
      const rgba = context.getImageData(0, 0, 192, 192).data;
      const bits = new Uint8Array(192 * 192 / 8);
      for (let i = 0; i < 192 * 192; i++) if (rgba[i * 4 + 3] >= 128) bits[i >> 3] |= 1 << (i & 7);
      return [...bits];
    }, svg);
    masks.push(Buffer.from(bytes));
  }
  const data = Buffer.concat(masks);
  const receipt = `${JSON.stringify({
    upstream: 'b7b6ecf1316d0af64c97a6b0392abe5e816a8e30', chromium: browser.version(),
    edge: 192, threshold: 128, order: Object.keys(mapping), inputs,
    sha256: createHash('sha256').update(data).digest('hex'), bytes: data.length,
  }, null, 2)}\n`;
  if (process.argv.includes('--check')) {
    const previous = await readFile(new URL('lucide-192.bin', assets));
    const previousReceipt = await readFile(new URL('lucide-receipt.json', assets), 'utf8');
    if (!previous.equals(data) || previousReceipt !== receipt) throw new Error('Native Lucide masks or provenance changed');
    console.log('23 native Lucide masks and raster provenance reproduce exactly');
  } else {
    await writeFile(new URL('lucide-192.bin', assets), data);
    await writeFile(new URL('lucide-receipt.json', assets), receipt);
  }
} finally { await browser.close(); }
