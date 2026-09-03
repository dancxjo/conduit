import { createExistingComputerAdapter, EXISTING_COMPUTER_BOUNDS, EXISTING_COMPUTER_MODES } from "../../../creche-existing-computer.mjs";
import { createBrowserConfigurationOutfitter, prepareCheckedBrowserSpore } from "../../../creche-browser-configuration.mjs";
import { acquireHostRelease } from "../../../creche-release-bundle.mjs";
import { buildBrowserBundleImage } from "./browser-bundle.mjs";

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
  builder_adapter: "conduit-host-browser/bind-prebuilt@1",
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
      Object.freeze({ id: "conduit-carrier/browser-release-download@1", label: "Download Body-bound browser ZIP" }),
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
  createAdapter: ({ host, presentationFor }) => {
    const existing = createExistingComputerAdapter({ host, profile: PROFILE });
    let outfitter;
    return Object.freeze({
      ...existing,
      createOptions({ mode, onChange }) {
        if (!outfitter) outfitter = createBrowserConfigurationOutfitter({ host, presentationFor, onChange });
        const note = existing.createOptions({ mode });
        const root = document.createElement("div");
        root.append(note, outfitter.render());
        return root;
      },
      configuration: () => Object.freeze({ required: true, checked: outfitter?.checked() ?? null }),
      async obtain({ mode, signal }) {
        const checked = outfitter?.checked();
        if (!checked) throw new Error("review the browser Host configuration before BUILD");
        const distribution = await acquireHostRelease({
          ...PROFILE,
          builder_adapter: "conduit-host-browser/build-wasm@1",
        }, signal);
        const release = await buildBrowserBundleImage({ checked, distribution });
        return Object.freeze({
          resultKind: "installation",
          private: Object.freeze({ release, distribution }),
          evidence: Object.freeze({
            schema: "conduit.browser/bundle-build-receipt@1",
            mode,
            result_kind: "installation",
            target_id: PROFILE.target_id,
            profile_id: checked.profile_id,
            source_configuration_id: checked.configuration_id,
            build_id: release.manifest.browser_build_id,
            image_id: release.manifest.browser_image_id,
            image_content_digest: release.manifest.bundle_sha256,
            bundle_sha256: release.manifest.bundle_sha256,
            distribution_id: release.manifest.distribution_id,
            distribution_sha256: release.manifest.distribution_sha256,
            builder_adapter: release.manifest.builder_adapter,
            compiler_started: false,
            network_fetches: [new URL(PROFILE.manifest_path).pathname, ...distribution.payloads.map((item) => item.path)],
            does_not_prove: ["body-binding", "start", "boot-observation", "join", "membership"],
          }),
        });
      },
      async bind(args) {
        const checked = outfitter?.checked();
        if (!checked) throw new Error("review the browser Host configuration before Body binding");
        return existing.bind({
          ...args,
          prepareSpore: ({ imageDigest, nowMillis, entropy }) => prepareCheckedBrowserSpore({
            host, checked, selection: outfitter.selection(), imageDigest, nowMillis, entropy,
          }),
        });
      },
    });
  },
});
