import { createExistingComputerAdapter, EXISTING_COMPUTER_BOUNDS, EXISTING_COMPUTER_MODES } from "../../../creche-existing-computer.mjs";

const FAMILY = Object.freeze({ id: "conduit-target-family/hosted-linux@1", label: "Linux computers" });

const PROFILES = Object.freeze([
  profile("workstation", "Hosted Linux workstation"),
  profile("server", "Hosted Linux server"),
]);

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

function profile(machine, label) {
  const target = Object.freeze({
    id: `std/x86_64/${machine}`,
    label,
    model_id: `conduit.host/hosted-linux-${machine}@1`,
    profile_id: `linux-x86_64-${machine}`,
  });
  return Object.freeze({
    target,
    target_id: target.id,
    manifest_path: new URL(`../../../artifacts/hosted-linux-${machine}.json`, import.meta.url).href,
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
      machine,
      package_id: "hosted-native@1",
      output: "native-bundle",
      acquisition: "reviewed generic release download",
      implemented_carriers: ["conduit-carrier/browser-release-download@1"],
      unavailable_carriers: ["explicit-helper", "ssh", "package-manager", "container", "already-running"],
    }),
  });
}
