// Compatibility entrance only. Zero-Body bootstrap and retained Body truth
// are owned by the Workspace application; Crèche no longer runs a parallel
// lifecycle, durability, provisioning, or graduation application.

export async function startApplication() {
  const workspace = new URL("../workspace/", document.baseURI);
  globalThis.location.replace(workspace.href);
}
