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
globalThis.__conduitBrowserHost = Object.freeze({ hostId, bootId, runtime: api, media });
