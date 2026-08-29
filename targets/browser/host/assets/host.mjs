import { initializeBrowserHost } from "/browser-host-bootstrap.mjs";

const { hostId, bootId, runtime: api } = await initializeBrowserHost();

document.querySelector("#host-id").textContent = hostId;
document.querySelector("#boot-id").textContent = bootId;
document.querySelector("#identity").hidden = false;
document.querySelector("#status").textContent = "Current and independently initialized";
globalThis.__conduitBrowserHost = Object.freeze({ hostId, bootId, runtime: api });
const { createBrowserMediaHost } = await import("/media-host.mjs");
const media = createBrowserMediaHost({ api, hostId, bootId });
document.querySelector("#media").hidden = false;
document.querySelector("#camera").addEventListener("click", () => media.acquire("camera"));
document.querySelector("#microphone").addEventListener("click", () => media.acquire("microphone"));
const { createBrowserDeviceBase } = await import("/device-base.mjs");
const devices = createBrowserDeviceBase({ api, hostId, bootId });
const { createBrowserUsbDeviceBase } = await import("/usb-device-base.mjs");
const usbDevices = createBrowserUsbDeviceBase({ api, hostId, bootId });
document.querySelector("#devices").hidden = false;
document.querySelector("#serial").addEventListener("click", async () => {
  try {
    globalThis.__conduitSerialResource = await devices.acquireSerial();
  } catch {
    // The adapter has already retained and published the exact refusal.
  }
});
document.querySelector("#usb").addEventListener("click", async () => {
  try {
    globalThis.__conduitUsbResource = await usbDevices.acquireUsb();
  } catch {
    // The adapter has already retained and published the exact refusal.
  }
});
globalThis.__conduitBrowserHost = Object.freeze({
  hostId,
  bootId,
  runtime: api,
  media,
  devices,
  usbDevices,
});
