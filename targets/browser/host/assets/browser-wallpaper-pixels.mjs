// Prepare the same RGBA representation used by the native wallpaper Host.
const MAXIMUM_SOURCE_EDGE = 2048;
const MAXIMUM_WIDTH = 1024;
const MAXIMUM_HEIGHT = 600;
const JPEG_FRAMES = new Set([0xc0, 0xc1, 0xc2]);
const hex = bytes => Array.from(new Uint8Array(bytes), byte => byte.toString(16).padStart(2, "0")).join("");

function jpegExtent(bytes, view) {
  if (view.getUint16(0) !== 0xffd8) return null;
  for (let offset = 2; offset + 4 <= bytes.length;) {
    if (bytes[offset++] !== 0xff) return null;
    while (bytes[offset] === 0xff) offset++;
    const marker = bytes[offset++];
    if (marker === 0xda || marker === 0xd9 || offset + 2 > bytes.length) return null;
    const size = view.getUint16(offset);
    if (size < 2 || size > bytes.length - offset) return null;
    if (JPEG_FRAMES.has(marker)) {
      if (size < 8) return null;
      return { height: view.getUint16(offset + 3), width: view.getUint16(offset + 5), mime: "image/jpeg" };
    }
    offset += size;
  }
  return null;
}

function rasterExtent(bytes) {
  if (bytes.length < 24) return null;
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  if (view.getUint32(0) !== 0x89504e47) return jpegExtent(bytes, view);
  if (view.getUint32(4) !== 0x0d0a1a0a || view.getUint32(8) !== 13
      || view.getUint32(12) !== 0x49484452) return null;
  return { width: view.getUint32(16), height: view.getUint32(20), mime: "image/png" };
}

export async function prepareWallpaperPixels(photo, bytes, signal) {
  const extent = rasterExtent(bytes);
  if (!extent || !extent.width || !extent.height
      || extent.width > MAXIMUM_SOURCE_EDGE || extent.height > MAXIMUM_SOURCE_EDGE) return null;
  signal.throwIfAborted();
  const bitmap = await createImageBitmap(new Blob([bytes], { type: extent.mime }), {
    imageOrientation: "none",
  });
  try {
    signal.throwIfAborted();
    if (bitmap.width !== extent.width || bitmap.height !== extent.height) return null;
    const scale = Math.min(1, MAXIMUM_WIDTH / bitmap.width, MAXIMUM_HEIGHT / bitmap.height);
    const width = Math.max(1, Math.floor(bitmap.width * scale));
    const height = Math.max(1, Math.floor(bitmap.height * scale));
    const canvas = new OffscreenCanvas(width, height);
    const context = canvas.getContext("2d", { willReadFrequently: true });
    if (!context) return null;
    context.drawImage(bitmap, 0, 0, width, height);
    const pixels = context.getImageData(0, 0, width, height).data;
    for (let alpha = 3; alpha < pixels.length; alpha += 4) {
      if (pixels[alpha] !== 255) return null;
    }
    const identity = hex(await crypto.subtle.digest("SHA-256", bytes));
    const version = hex(await crypto.subtle.digest("SHA-256", pixels));
    signal.throwIfAborted();
    return { ...photo, identity, version, width, height, pixels };
  } finally {
    bitmap.close();
  }
}
