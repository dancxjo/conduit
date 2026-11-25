import { acquireBrowserWallpaper } from "./browser-wallpaper-acquisition.mjs";

const PROFILE = "media/image-rgba8@1";
const ACCESS = "conduit.resource/image-content@1";

/** Bind the prepared image bytes to the exact references exposed by the Host. */
export async function prepareBrowserWallpaperDeck({ fetch, latestDate, signal }) {
  const prepared = await acquireBrowserWallpaper({ fetch, latestDate, signal });
  if (!prepared.photos) return Object.freeze({ reason: prepared.reason });

  const images = new Map();
  const metadata = prepared.photos.map(photo => {
    const { identity, version, width, height, pixels } = photo;
    const reference = Object.freeze({
      identity, version, width, height, bytes: pixels.length,
      contentProfile: PROFILE, accessClass: ACCESS,
    });
    images.set(identity, { reference, pixels });
    return Object.freeze({
      reference,
      title: photo.title,
      caption: "",
      attribution: photo.attribution,
      license: "License not supplied by provider",
      source: `NASA APOD/${photo.date}`,
      imageFrame: `apod/${photo.date}`,
    });
  });

  return Object.freeze({
    metadata: Object.freeze(metadata),
    copyPixels(reference, destination) {
      const image = images.get(reference?.identity);
      if (!image || !(destination instanceof Uint8ClampedArray)) return false;
      const exact = image.reference;
      if (reference.version !== exact.version || reference.width !== exact.width
          || reference.height !== exact.height || reference.bytes !== exact.bytes
          || reference.contentProfile !== exact.contentProfile || reference.accessClass !== exact.accessClass
          || destination.length !== exact.bytes) return false;
      destination.set(image.pixels);
      return true;
    },
    release() {
      images.clear();
    },
  });
}
