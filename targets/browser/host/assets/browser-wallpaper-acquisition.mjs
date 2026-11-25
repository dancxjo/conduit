import { prepareWallpaperPixels } from "./browser-wallpaper-pixels.mjs";

// APOD Basic preparation for the Browser Host. Only the admitted Host should
// invoke this adapter, before Play. The Mask consumes retained resources.
const LIST = "https://science.nasa.gov/wp-json/wp/v2/apod-basic?per_page=12&_fields=date,title,media_type,hdurl,copyright";
const ASSET_ORIGIN = "https://assets.science.nasa.gov";
const ASSET_PATH = /^\/dynamicimage\/assets\/science\/[A-Za-z0-9_/-]+\.(?:jpe?g|png)$/i;
const ATTEMPTS = 12;
const DECK_LENGTH = 4;
const MAXIMUM_LIST_BYTES = 256 * 1024;
const MAXIMUM_IMAGE_BYTES = 4 * 1024 * 1024;
const encoder = new TextEncoder();

class WallpaperPreparationRefusal extends Error {
  constructor(reason) {
    super(reason);
    this.reason = reason;
  }
}

const unavailable = () => new WallpaperPreparationRefusal("provider-unavailable");

async function readBounded(fetch, url, limit, signal) {
  signal.throwIfAborted();
  const response = await fetch(url, {
    signal, credentials: "omit", redirect: "error", referrerPolicy: "no-referrer",
  });
  if (response.status === 429) throw new WallpaperPreparationRefusal("provider-rate-limited");
  if (!response.ok || !response.body) throw unavailable();

  const reader = response.body.getReader();
  try {
    const bytes = new Uint8Array(limit);
    let length = 0;
    for (;;) {
      signal.throwIfAborted();
      const { value, done } = await reader.read();
      if (done) break;
      if (value.length > limit - length) throw unavailable();
      bytes.set(value, length);
      length += value.length;
    }
    signal.throwIfAborted();
    return bytes.slice(0, length);
  } finally {
    await reader.cancel().catch(() => {});
    reader.releaseLock();
  }
}

function candidate(entry, latestDate) {
  if (entry?.media_type !== "image" || typeof entry.date !== "string"
      || !/^\d{4}-\d{2}-\d{2}$/.test(entry.date) || entry.date > latestDate
      || typeof entry.title !== "string" || !entry.title.trim()
      || encoder.encode(entry.title).length > 512
      || typeof entry.hdurl !== "string" || entry.hdurl.length > 2048) {
    return null;
  }
  const credit = entry.copyright ?? "Creator credit not supplied by NASA APOD";
  if (typeof credit !== "string" || encoder.encode(credit).length > 256) return null;
  const url = URL.parse(entry.hdurl);
  if (!url || url.origin !== ASSET_ORIGIN || url.username || url.password
      || !ASSET_PATH.test(url.pathname) || url.hash) return null;
  url.search = "?w=1024&h=1024&fit=clip";
  return { date: entry.date, title: entry.title, attribution: credit, url: url.href };
}

/**
 * Prepare a finite deck using the selected Host's bounded decoder. The decoder
 * returns an owned photo with canonical identity, or null on refusal. It must
 * not publish resources. The caller can admit custody of the complete deck.
 */
export async function acquireBrowserWallpaper({ fetch, preparePhoto = prepareWallpaperPixels, latestDate, signal }) {
  try {
    const list = await readBounded(fetch, LIST, MAXIMUM_LIST_BYTES, signal);
    const entries = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(list));
    if (!Array.isArray(entries) || entries.length > ATTEMPTS) throw unavailable();
    const candidates = entries.map(entry => candidate(entry, latestDate)).filter(Boolean)
      .sort((a, b) => b.date.localeCompare(a.date));
    const photos = [];
    const identities = new Set();
    for (const photo of candidates) {
      let bytes;
      try {
        bytes = await readBounded(fetch, photo.url, MAXIMUM_IMAGE_BYTES, signal);
      } catch (error) {
        signal.throwIfAborted();
        if (error?.reason === "provider-rate-limited") throw error;
        continue;
      }
      let prepared;
      try {
        prepared = await preparePhoto(photo, bytes, signal);
      } catch {
        signal.throwIfAborted();
        continue;
      }
      signal.throwIfAborted();
      if (prepared === null || identities.has(prepared.identity)) continue;
      identities.add(prepared.identity);
      photos.push(prepared);
      if (photos.length === DECK_LENGTH) return { photos };
    }
    throw unavailable();
  } catch (error) {
    signal.throwIfAborted();
    return { reason: error instanceof WallpaperPreparationRefusal ? error.reason : "provider-unavailable" };
  }
}
