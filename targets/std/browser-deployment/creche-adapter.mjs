import { createExistingComputerAdapter, EXISTING_COMPUTER_BOUNDS, EXISTING_COMPUTER_MODES } from "../../../creche-existing-computer.mjs";

const FAMILY = Object.freeze({ id: "conduit-target-family/hosted-computer@1", label: "Hosted computers" });

const PLATFORM_VARIANTS = Object.freeze([
  Object.freeze({ os: "linux", architecture: "x86_64", status: "supported", reason: "reviewed native build and launch adapters are available" }),
  Object.freeze({ os: "linux", architecture: "aarch64", status: "planned", reason: "no reviewed hosted launch adapter is installed" }),
  Object.freeze({ os: "windows", architecture: "x86_64", status: "planned", reason: "no reviewed Windows build and launch adapters are installed" }),
  Object.freeze({ os: "macos", architecture: "aarch64", status: "planned", reason: "no reviewed macOS build and launch adapters are installed" }),
]);

const PROFILES = Object.freeze([profile()]);

export const STD_EXISTING_COMPUTER_CONTRIBUTIONS = Object.freeze(PROFILES.map((targetProfile) => Object.freeze({
  schema: "conduit.creche/physical-host-target-entry@1",
  family: FAMILY,
  target: targetProfile.target,
  intentions: EXISTING_COMPUTER_MODES,
  fabrication_strategies: Object.freeze([
    Object.freeze({ id: "reviewed-generic-release-download", label: "Reviewed generic native release" }),
  ]),
  carriers: Object.freeze({
    deployment: Object.freeze([]),
    installation: Object.freeze([
      Object.freeze({ id: "conduit-carrier/browser-release-download@1", label: "Download Body-bound native spore" }),
    ]),
    attachment: Object.freeze([]),
    observation: Object.freeze([]),
  }),
  bounds: EXISTING_COMPUTER_BOUNDS,
  expected_join_contract: "conduit.host/native-spawn-observation@1",
  target_profile: targetProfile.declaration,
  createAdapter: ({ host }) => createExistingComputerAdapter({ host, profile: targetProfile }),
})));

function profile() {
  const target = Object.freeze({
    id: "std/x86_64/computer",
    label: "Hosted computer · Linux · x86_64",
    model_id: "conduit.host/hosted-computer@1",
    profile_id: "hosted-linux-x86_64",
  });
  return Object.freeze({
    target,
    target_id: target.id,
    manifest_path: new URL("../../../artifacts/hosted-linux-x86_64.json", import.meta.url).href,
    package_id: "hosted-native@1",
    output: "native-bundle",
    builder_adapter: "conduit-host-hosted/build-native@1",
    deployment_adapter: "conduit-host-hosted/launch@1",
    os: "linux",
    architecture: "x86_64",
    browser_carrier: false,
    declaration: Object.freeze({
      schema: "conduit.host/creche-existing-computer-profile@1",
      os: "linux",
      architecture: "x86_64",
      machine: "computer",
      role_profile: null,
      platform_variants: PLATFORM_VARIANTS,
      package_id: "hosted-native@1",
      output: "native-bundle",
      acquisition: "reviewed generic release download",
      implemented_carriers: ["conduit-carrier/browser-release-download@1"],
      unavailable_carriers: ["explicit-helper", "ssh", "package-manager", "container", "already-running"],
    }),
  });
}
