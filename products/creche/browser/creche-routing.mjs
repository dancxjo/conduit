import { browserHostOperationLimits, createBrowserHostOperations } from "../../../targets/browser/host/assets/browser-host-operations.mjs";

const MAXIMUM_LOCATION_SEQUENCE = 0xffff_ffff;

export function createCrecheRouting({ host, applicationId, steps, onPopState, onFailure }) {
  const operations = createBrowserHostOperations({
    hostId: host.hostId,
    bootId: host.bootId,
    applicationId,
    applicationGeneration: 1,
    authorityGeneration: 1,
  });
  const productRoot = new URL(".", document.baseURI);
  const routes = steps.map(({ slug }) => new URL(`${slug}/`, productRoot).pathname);
  let sequence = 0;
  let artifactSequence = 0;

  const indexForLocation = () => {
    if (location.pathname === productRoot.pathname
      || location.pathname === new URL("index.html", productRoot).pathname) return 0;
    const index = routes.indexOf(location.pathname);
    if (index === -1) throw new Error("this Crèche step does not exist");
    return index;
  };

  addEventListener("popstate", () => {
    try { onPopState(indexForLocation()); }
    catch (error) { onFailure(error); }
  });

  return Object.freeze({
    current: indexForLocation,
    isProductRoot() {
      return location.pathname === productRoot.pathname
        || location.pathname === new URL("index.html", productRoot).pathname;
    },
    async move(index, mode) {
      if (!Number.isInteger(index) || index < 0 || index >= routes.length) {
        throw new Error("Crèche route is outside the admitted workflow");
      }
      sequence = sequence === MAXIMUM_LOCATION_SEQUENCE ? 1 : sequence + 1;
      const outcome = await operations.moveLocation({
        contract: browserHostOperationLimits.contract,
        kind: "location",
        operationId: `creche/location-${sequence}`,
        hostId: host.hostId,
        bootId: host.bootId,
        applicationId,
        applicationGeneration: 1,
        authorityGeneration: 1,
        presentationRevision: sequence,
        mode,
        path: routes[index],
      });
      if (outcome.disposition !== "completed" || outcome.path !== routes[index]) {
        throw new Error(`Crèche location ${mode} refused (${outcome.disposition})`);
      }
    },
    async handoffArtifact(artifact) {
      artifactSequence = artifactSequence === MAXIMUM_LOCATION_SEQUENCE ? 1 : artifactSequence + 1;
      return operations.handoffArtifact({
        contract: browserHostOperationLimits.contract,
        kind: "artifact-handoff",
        operationId: `creche/artifact-${artifactSequence}`,
        hostId: host.hostId,
        bootId: host.bootId,
        applicationId,
        applicationGeneration: 1,
        authorityGeneration: 1,
        userActivation: true,
        artifactId: artifact.artifact_id,
        bytes: artifact.payload,
        maximumBytes: artifact.maximum_bytes,
        filename: artifact.filename,
        mediaType: artifact.media_type,
      });
    },
  });
}
