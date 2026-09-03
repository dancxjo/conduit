import { browserHostOperationLimits, createBrowserHostOperations } from "./browser-host-operations.mjs";

const MAXIMUM_LOCATION_SEQUENCE = 0xffff_ffff;

export function createBookRouting({ host, applicationId, isRunning, currentPage, render, onFailure }) {
  const operations = createBrowserHostOperations({
    hostId: host.hostId,
    bootId: host.bootId,
    applicationId,
    applicationGeneration: 1,
    authorityGeneration: 1,
  });
  let routes = [];
  let sequence = 0;

  const isProductRoot = () => location.pathname === new URL(".", document.baseURI).pathname
    || location.pathname === new URL("index.html", document.baseURI).pathname;
  const indexForLocation = () => {
    if (isProductRoot()) return 0;
    const legacyRoutes = new Map([
      [new URL("meet-one-gear/", document.baseURI).pathname, 0],
      [new URL("same-face-different-implementation/", document.baseURI).pathname, 1],
    ]);
    const index = legacyRoutes.get(location.pathname) ?? routes.indexOf(location.pathname);
    if (index === -1) throw new Error("this Tour page does not exist");
    return index;
  };
  const move = async (index, mode) => {
    sequence = sequence === MAXIMUM_LOCATION_SEQUENCE ? 1 : sequence + 1;
    const outcome = await operations.moveLocation({
      contract: browserHostOperationLimits.contract,
      kind: "location",
      operationId: `book/location-${sequence}`,
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
      throw new Error(`Tour location ${mode} refused (${outcome.disposition})`);
    }
  };

  addEventListener("popstate", () => {
    (async () => {
      const index = indexForLocation();
      if (isRunning()) await move(currentPage(), "replace");
      else await render(index);
    })().catch(onFailure);
  });

  return Object.freeze({
    admitPages(markdownPages) {
      routes = markdownPages.map((markdown) => {
        const title = markdown.match(/^# (.+)$/m)?.[1];
        if (!title) throw new Error("a Tour page has no title");
        const slug = title.toLowerCase().normalize("NFKD").replace(/\p{M}/gu, "")
          .replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
        if (!slug) throw new Error("a Tour page has no route identity");
        return new URL(`${slug}/`, document.baseURI).pathname;
      });
      return Object.freeze({ index: indexForLocation(), normalize: isProductRoot() });
    },
    move,
  });
}
