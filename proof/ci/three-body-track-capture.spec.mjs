import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import test from "node:test";

const script = resolve("tools/ci/capture-three-body-track.mjs");
const commit = "a".repeat(40);

function capture(root, mode, source, media = "-") {
  const sourcePath = join(root, `${mode}.json`);
  const output = join(root, mode);
  writeFileSync(sourcePath, JSON.stringify(source));
  execFileSync(process.execPath, [script, mode, commit, sourcePath, media, output]);
  return JSON.parse(readFileSync(join(output, "track.json"), "utf8"));
}

function native() {
  return {
    schema: "conduit.conduitos/product-journey-proof@4", base_commit: commit,
    open_effects: 0, profile_id: "profile/native", build_id: "build/native", image_id: "image/native",
    body_id: "body/native", host_id: "host/native", boot_id: "boot/native", part_id: "part/native",
    born_sign_id: "sign/native/born", fulfilled_sign_id: "sign/native/fulfilled", wake_id: "wake/native",
    plan_id: "plan/native", active_play_id: "play/native", input_sign_id: "sign/native/input",
    result_sign_id: "sign/native/result", result: "HELLO", inspector_presentation_id: "presentation/native",
    inspector_manifestation_id: "manifestation/native", workset: { patchbay_presenters_replanned: true },
    usb_line_id: "line/native", usb_line_plan_id: "plan/native/distributed",
    usb_line_peer_host_id: "host/native/peer", usb_line_peer_boot_id: "boot/native/peer",
    usb_line_membership: "admitted-present", usb_line_final_membership: "admitted-offline",
    resize_invalidated_manifestation_id: "manifestation/native/stale",
    resize_current_manifestation_id: "manifestation/native/repaired",
    resize_input_refused_while_invalidated: true, body_retained_after_lull: true,
    body_fulfilled: true, remained_alive: true,
  };
}

function browser() {
  const born = { Born: { sign_id: "sign/browser/born" } };
  const fulfilled = { Fulfilled: { sign_id: "sign/browser/fulfilled" } };
  const playing = { lifecycle: "Playing", plans: [{ plan_id: "plan/browser", active_play_id: "play/browser" }] };
  return {
    schema: "conduit.browser/body-journey@1", git_commit: commit,
    current: { body_id: "body/browser", host_id: "host/browser", boot_id: "boot/browser", workload_revision: 1 },
    state: { state: "Fulfilled", terminal: { presentation_id: "presentation/browser" } },
    evidence: { evidence: { body: { events: [born, fulfilled], workset: { forms: [{ checked_form_id: "checked/browser" }] } },
      wakes: [playing] } },
    checkpoints: {
      joined: { evidence: { membership: { revision: 3, parts: [
        { current: { host_id: "host/browser", boot_id: "boot/browser" } },
        { current: { host_id: "host/browser/peer", boot_id: "boot/browser/peer" } },
      ] } } },
      refused: { playback: { refusal: { code: "ExecutionLineUnavailable" } },
        evidence: { evidence: { wakes: [{ lifecycle: "Failed", sign_ids: ["sign/browser/fault"] }] } } },
      repaired: { evidence: { evidence: { wakes: [playing] } } },
    },
  };
}

function generative() {
  return {
    body_journey: {
      schema: "conduit.evidence/orifina-body-journey@1", body_id: "body/generative",
      host_ids: ["host/generative", "host/generative/peer"],
      boot_ids: ["boot/generative", "boot/generative/peer"],
      plan_ids: ["plan/generative/first", "plan/generative/repaired"],
      play_ids: ["play/generative/first", "play/generative/repaired"],
      workload_revision: 1, fault_reason: "provider-unavailable", repaired: true, fulfilled: true,
      biography: { body: { events: [
        { Born: { sign_id: "sign/generative/born" } },
        { Fulfilled: { sign_id: "sign/generative/fulfilled" } },
      ] }, wakes: [{ lifecycle: "Failed" }, { lifecycle: "Lulled" }], membership: { revision: 3 } },
    },
    local_model: { presenter_requests: [{ source_presentation_identity: "presentation/generative",
      manifestation: { manifestation_identity: "manifestation/generative", disposition: "Produced",
        provider_identity: "provider/ollama", model_identity: "model/gemma3-270m" } }] },
  };
}

test("each Body producer emits its own bounded semantic track", () => {
  const root = mkdtempSync(join(tmpdir(), "conduit-three-body-capture-"));
  try {
    const media = join(root, "browser.png");
    writeFileSync(media, "browser pixels");
    const tracks = [capture(root, "native", native()), capture(root, "browser", browser(), media),
      capture(root, "generative", generative())];
    assert.deepEqual(tracks.map(track => track.track_id),
      ["native-graphical", "browser-graphical", "hosted-generative"]);
    assert.ok(tracks.every(track => track.steps.length === 13));
    assert.equal(tracks[0].line_ids[0], "line/native");
    assert.equal(tracks[1].steps[8].provenance.sign_id, "sign/browser/fault");
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("a neighboring receipt cannot be relabeled as completed Body evidence", () => {
  const root = mkdtempSync(join(tmpdir(), "conduit-three-body-refusal-"));
  try {
    const source = native();
    source.usb_line_membership = "not-requested";
    const sourcePath = join(root, "native.json");
    writeFileSync(sourcePath, JSON.stringify(source));
    const result = spawnSync(process.execPath, [script, "native", commit, sourcePath, "-", join(root, "out")],
      { encoding: "utf8" });
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /peer was not admitted and present/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
