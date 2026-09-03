import { initializeBrowserHost } from "./browser-host-bootstrap.mjs";

const suspense = document.querySelector("#suspense");
const hostSurface = document.querySelector("#host");

function revealHost() {
  hostSurface.hidden = false;
  suspense.setAttribute("aria-busy", "false");
  requestAnimationFrame(() => {
    hostSurface.classList.add("ready");
    suspense.classList.add("leaving");
    if (matchMedia("(prefers-reduced-motion: reduce)").matches) suspense.hidden = true;
    else suspense.addEventListener("transitionend", () => { suspense.hidden = true; }, { once: true });
  });
}

try {
  const { hostId, bootId, runtime: api } = await initializeBrowserHost();

  document.querySelector("#host-id").textContent = hostId;
  document.querySelector("#boot-id").textContent = bootId;
  document.querySelector("#identity").hidden = false;
  globalThis.__conduitBrowserHost = Object.freeze({ hostId, bootId, runtime: api });
  const { createBrowserMediaHost } = await import("./media-host.mjs");
  const media = createBrowserMediaHost({
    api,
    hostId,
    bootId,
    selectedImplementations: [
      "browser/media-devices-camera@1",
      "browser/media-devices-microphone@1",
    ],
  });
  document.querySelector("#media").hidden = false;
  document.querySelector("#camera").addEventListener("click", () => media.acquire("camera"));
  document.querySelector("#microphone").addEventListener("click", () => media.acquire("microphone"));
  const { createBrowserDeviceBase } = await import("./device-base.mjs");
  const selectedDeviceImplementations = ["browser/webserial@1", "browser/webusb@1"];
  const devices = createBrowserDeviceBase({
    api, hostId, bootId, selectedImplementations: selectedDeviceImplementations,
  });
  const { createBrowserUsbDeviceBase } = await import("./usb-device-base.mjs");
  const usbDevices = createBrowserUsbDeviceBase({
    api, hostId, bootId, selectedImplementations: selectedDeviceImplementations,
  });
  document.querySelector("#devices").hidden = false;
  const serialClose = document.querySelector("#serial-close");
  const usbClose = document.querySelector("#usb-close");
  let serialResource = null;
  let usbResource = null;
  document.querySelector("#serial").addEventListener("click", async () => {
    try {
      serialResource = await devices.acquireSerial();
      globalThis.__conduitSerialResource = serialResource;
      serialClose.disabled = false;
    } catch {
      // The adapter has already retained and published the exact refusal.
    }
  });
  serialClose.addEventListener("click", async () => {
    if (!serialResource) return;
    try {
      await serialResource.close();
    } catch {
      // The adapter has already retained and published the exact close failure.
    } finally {
      serialClose.disabled = true;
    }
  });
  document.querySelector("#usb").addEventListener("click", async () => {
    try {
      usbResource = await usbDevices.acquireUsb();
      globalThis.__conduitUsbResource = usbResource;
      usbClose.disabled = false;
    } catch {
      // The adapter has already retained and published the exact refusal.
    }
  });
  usbClose.addEventListener("click", async () => {
    if (!usbResource) return;
    try {
      await usbResource.close();
    } catch {
      // The adapter has already retained and published the exact close failure.
    } finally {
      usbClose.disabled = true;
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
  revealHost();
} catch (error) {
  suspense.classList.add("failed");
  suspense.setAttribute("aria-busy", "false");
  suspense.querySelector("h1").textContent = "This Host couldn’t wake up";
  document.querySelector("#starting-status").textContent = error instanceof Error ? error.message : String(error);
}
