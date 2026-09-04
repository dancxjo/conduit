import { browserHostOperationLimits, createBrowserHostOperations } from "./browser-host-operations.mjs";

const MAXIMUM_LOCATION_SEQUENCE = 0xffff_ffff;
const MAXIMUM_TOUR_PAGES = 16;
const MAXIMUM_STAGES_PER_PAGE = 8;
const IDENTITY = /^[a-z0-9]+(?:-[a-z0-9]+)*$/;

export function parseTourPages(chapters) {
  if (!Array.isArray(chapters) || chapters.length === 0 || chapters.length > MAXIMUM_TOUR_PAGES) {
    throw new Error("Tour page count is outside its admitted bound");
  }
  const identities = new Set();
  const routes = new Set();
  return Object.freeze(chapters.map((source) => {
    const normalized = source.replaceAll("\r\n", "\n");
    const match = /^---\n([\s\S]*?)\n---\n([\s\S]+)$/.exec(normalized);
    if (!match) throw new Error("Tour page metadata is missing");
    const metadata = new Map();
    const stages = [];
    for (const line of match[1].split("\n")) {
      const separator = line.indexOf(":");
      if (separator < 1) throw new Error("Tour page metadata is malformed");
      const key = line.slice(0, separator).trim();
      const value = line.slice(separator + 1).trim();
      if (key === "stage") {
        const [identity, mode, extra] = value.split("|");
        if (extra !== undefined || !/^canonical-form:[a-z][a-z0-9-]*$/.test(identity)
          || !["run", "recursive", "compare", "two-host", "two-host-plan"].includes(mode)
          || stages.length === MAXIMUM_STAGES_PER_PAGE) throw new Error("Tour stage metadata is malformed or over capacity");
        stages.push(Object.freeze({ identity, mode }));
      } else {
        if (!["page", "route", "companion"].includes(key) || metadata.has(key) || !IDENTITY.test(value)) {
          throw new Error("Tour page metadata is malformed");
        }
        metadata.set(key, value);
      }
    }
    const identity = metadata.get("page");
    const route = metadata.get("route");
    const companion = metadata.get("companion");
    const title = match[2].match(/^# (.+)$/m)?.[1];
    if (!identity || !route || !companion || !title || identities.has(identity) || routes.has(route)) {
      throw new Error("Tour page identity, route, companion, or title is invalid or duplicated");
    }
    identities.add(identity);
    routes.add(route);
    return Object.freeze({ identity, route, companion, title, stages: Object.freeze(stages), markdown: match[2] });
  }));
}

export function createTourRouting({ host, applicationId, render, onFailure }) {
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
      await render(index);
    })().catch(onFailure);
  });

  return Object.freeze({
    admitPages(pages) {
      routes = pages.map((page) => new URL(`${page.route}/`, document.baseURI).pathname);
      return Object.freeze({ index: indexForLocation(), normalize: isProductRoot() });
    },
    move,
  });
}
