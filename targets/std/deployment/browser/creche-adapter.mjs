import { createExistingComputerAdapter, EXISTING_COMPUTER_BOUNDS, EXISTING_COMPUTER_RENDEZVOUS_MODES } from "../../../creche-existing-computer.mjs";

const FAMILY = Object.freeze({ id: "conduit-target-family/hosted-computer@1", label: "Hosted computers" });

const PLATFORM_VARIANTS = Object.freeze([
  Object.freeze({ os: "linux", architecture: "x86_64", status: "supported", reason: "reviewed native build and launch adapters are available" }),
  Object.freeze({ os: "linux", architecture: "aarch64", status: "planned", reason: "no reviewed hosted launch adapter is installed" }),
  Object.freeze({ os: "windows", architecture: "x86_64", status: "supported", reason: "reviewed native Windows build and launch adapters are available" }),
  Object.freeze({ os: "macos", architecture: "aarch64", status: "supported", reason: "reviewed native macOS build and launch adapters are available" }),
]);

const PROFILES = Object.freeze([
  profile({
    id: "std/x86_64/computer",
    label: "Hosted computer · Linux · x86_64",
    profileId: "hosted-linux-x86_64",
    manifest: "hosted-linux-x86_64.json",
    os: "linux",
    architecture: "x86_64",
    machine: "computer",
  }),
  profile({
    id: "std/x86_64/windows-computer",
    label: "Hosted computer · Windows · x86_64",
    profileId: "hosted-windows-x86_64",
    manifest: "hosted-windows-x86_64.json",
    os: "windows",
    architecture: "x86_64",
    machine: "windows-computer",
  }),
  profile({
    id: "std/aarch64/macos-computer",
    label: "Hosted computer · macOS · arm64",
    profileId: "hosted-macos-aarch64",
    manifest: "hosted-macos-aarch64.json",
    os: "macos",
    architecture: "aarch64",
    machine: "macos-computer",
  }),
]);

export const STD_EXISTING_COMPUTER_CONTRIBUTIONS = Object.freeze(PROFILES.map((targetProfile) => Object.freeze({
  schema: "conduit.creche/physical-host-target-entry@1",
  family: FAMILY,
  target: targetProfile.target,
  intentions: EXISTING_COMPUTER_RENDEZVOUS_MODES,
  fabrication_strategies: Object.freeze([
    Object.freeze({ id: "reviewed-generic-release-download", label: "Reviewed generic native release" }),
  ]),
  carriers: Object.freeze({
    deployment: Object.freeze([]),
    installation: Object.freeze([
      Object.freeze({ id: "conduit-carrier/browser-release-download@1", label: "Download body-bound native ZIP" }),
    ]),
    attachment: Object.freeze([
      Object.freeze({ id: "conduit-carrier/rendezvous-code@1", label: "One-use rendezvous code" }),
    ]),
    observation: Object.freeze([
      Object.freeze({ id: "conduit-line/loopback-websocket@1", label: "Local WebSocket Line" }),
      Object.freeze({ id: "conduit-line/serial-text@1", label: "Browser-attended serial Line" }),
    ]),
  }),
  bounds: EXISTING_COMPUTER_BOUNDS,
  expected_join_contract: "conduit.host/native-spawn-observation@1",
  target_profile: targetProfile.declaration,
  createAdapter: ({ host, prepareSpore = null }) => createExistingComputerAdapter({ host, profile: targetProfile, prepareSpore }),
})));

function profile({ id, label, profileId, manifest, os, architecture, machine }) {
  const target = Object.freeze({
    id,
    label,
    model_id: "conduit.host/hosted-computer@1",
    profile_id: profileId,
  });
  return Object.freeze({
    target,
    target_id: target.id,
    manifest_path: new URL(`../../../artifacts/${manifest}`, import.meta.url).href,
    package_id: "hosted-native@1",
    output: "native-bundle",
    builder_adapter: "conduit-host-hosted/build-native@1",
    deployment_adapter: "conduit-host-hosted/launch@1",
    os,
    architecture,
    browser_carrier: false,
    rendezvous: true,
    declaration: Object.freeze({
      schema: "conduit.host/creche-existing-computer-profile@1",
      os,
      architecture,
      machine,
      role_profile: null,
      platform_variants: PLATFORM_VARIANTS,
      package_id: "hosted-native@1",
      output: "native-bundle",
      acquisition: "reviewed generic release download",
      implemented_carriers: ["conduit-carrier/browser-release-download@1", "conduit-carrier/rendezvous-code@1", "conduit-line/loopback-websocket@1", "conduit-line/serial-text@1"],
      unavailable_carriers: ["explicit-helper", "ssh", "package-manager", "container", "webrtc", "usb", "bluetooth"],
    }),
  });
}
