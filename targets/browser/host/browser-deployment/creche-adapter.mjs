import { createExistingComputerAdapter, EXISTING_COMPUTER_BOUNDS, EXISTING_COMPUTER_MODES } from "../../../creche-existing-computer.mjs";

const TARGET = Object.freeze({
  id: "browser/wasm32/page",
  label: "Browser page Host",
  model_id: "conduit.host/browser-page@1",
  profile_id: "browser-wasm32-page",
});
const PROFILE = Object.freeze({
  target: TARGET,
  target_id: TARGET.id,
  manifest_path: new URL("../../../artifacts/browser-page.json", import.meta.url).href,
  package_id: "browser-wasm@1",
  output: "browser-bundle",
  builder_adapter: "conduit-host-browser/build-wasm@1",
  deployment_adapter: "conduit-host-browser/load@1",
  os: "browser",
  architecture: "wasm32",
  browser_carrier: true,
});

export const BROWSER_EXISTING_COMPUTER_CONTRIBUTION = Object.freeze({
  schema: "conduit.creche/physical-host-target-entry@1",
  family: Object.freeze({ id: "conduit-target-family/browser@1", label: "Browser Hosts" }),
  target: TARGET,
  intentions: EXISTING_COMPUTER_MODES,
  fabrication_strategies: Object.freeze([
    Object.freeze({ id: "reviewed-generic-release-download", label: "Reviewed generic browser bundle" }),
  ]),
  carriers: Object.freeze({
    deployment: Object.freeze([]),
    installation: Object.freeze([
      Object.freeze({ id: "conduit-carrier/browser-release-download@1", label: "Download Body-bound browser spore" }),
      Object.freeze({ id: "conduit-carrier/browser-local-sandbox@1", label: "Load exact browser bundle locally" }),
    ]),
    attachment: Object.freeze([]),
    observation: Object.freeze([
      Object.freeze({ id: "conduit-carrier/browser-local-spawn@1", label: "Observe fresh browser Boot and signed join" }),
    ]),
  }),
  bounds: EXISTING_COMPUTER_BOUNDS,
  expected_join_contract: "conduit.browser/creche-spawn-observation@1",
  target_profile: Object.freeze({
    schema: "conduit.host/creche-existing-computer-profile@1",
    os: "browser",
    architecture: "wasm32",
    machine: "page",
    package_id: "browser-wasm@1",
    output: "browser-bundle",
    acquisition: "reviewed generic release download",
    implemented_carriers: ["conduit-carrier/browser-release-download@1", "conduit-carrier/browser-local-sandbox@1", "conduit-carrier/browser-local-spawn@1"],
    unavailable_carriers: ["already-running"],
  }),
  createAdapter: ({ host }) => createExistingComputerAdapter({ host, profile: PROFILE }),
});
