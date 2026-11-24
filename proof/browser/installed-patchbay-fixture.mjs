import { expect } from "@playwright/test";
import { execFileSync, spawn } from "node:child_process";
import { once } from "node:events";
import { mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";

export async function installPatchbayBody(bundle, evidence) {
  const state = mkdtempSync(path.join(tmpdir(), "cpb-"));
  if (Buffer.byteLength(path.join(state, "control.sock")) >= 108) {
    throw new Error("The temporary directory is too long for the installed owner socket.");
  }
  const product = path.join(bundle, "conduit-linux-x86_64");
  const command = (...args) => JSON.parse(execFileSync(product, args, {
    encoding: "utf8", timeout: 15_000,
  }));
  execFileSync("sh", [path.join(bundle, "install-linux-x86_64.sh"), "--state-dir", state, "--no-start"]);
  execFileSync(product, ["host", "service", "configure",
    path.join(bundle, "linux-ambient.host.conduit"), "--image",
    path.join(bundle, "linux-ambient.image.json"), "--state-dir", state]);

  const errors = [];
  let service;
  const status = () => command("body", "status", "--state-dir", state, "--json");
  const startService = async () => {
    service = spawn(product, ["host", "service", "run", "--state-dir", state], {
      stdio: ["ignore", "ignore", "pipe"],
    });
    service.stderr.on("data", data => errors.push(data.toString()));
    await expect.poll(() => {
      if (service.exitCode !== null) throw new Error(errors.join(""));
      try {
        const receipt = JSON.parse(readFileSync(path.join(state, "runtime.json"), "utf8"));
        return receipt.process_id;
      } catch {
        return null;
      }
    }, { timeout: 15_000 }).toBe(service.pid);
  };
  const stopService = async () => {
    if (service?.exitCode === null && service.signalCode === null) {
      const closed = once(service, "exit");
      service.kill();
      await closed;
    }
  };
  const startPlay = () => command("body", "start", "--state-dir", state, "--maximum-millis", "60000");
  try {
    await startService();
    const birth = execFileSync(product, ["body", "birth", "--screen-free", "--state-dir", state], {
      encoding: "utf8", timeout: 30_000,
      input: ["focus creche.name", "edit value Installed Commons proof", "activate",
        "focus creche.plot.2", "edit value true", "activate",
        "focus creche.plot.3", "edit value true", "activate",
        "focus creche.birth", "activate", "quit", ""].join("\n"),
    });
    expect(birth).toContain("Body retained by this installed Host:");
    const retained = JSON.parse(readFileSync(path.join(state, "body/workset-sources.json"), "utf8"));
    expect(retained.entries.map(source => source.entry)).toEqual(["ambient-resident", "patchbay"]);
    if (!status().realization?.play?.active_play_id) startPlay();
    await expect.poll(() => status().realization?.play?.active_play_id).toMatch(/^body-play\//);
    const first = status();
    expect(first.biography.body.workset.plots).toHaveLength(2);
    await evidence.attach("first-installed-play", {
      contentType: "application/json",
      body: JSON.stringify({ body: first.biography.body_id, host: first.host.host_id,
        boot: first.host.boot_id, play: first.realization.play.active_play_id, state }),
    });
    await stopService();
    await startService();
    expect(status().biography.body_id).toBe(first.biography.body_id);
    expect(status().host.host_id).toBe(first.host.host_id);
    expect(status().host.boot_id).not.toBe(first.host.boot_id);
    const [ambient, patchbay] = first.biography.body.workset.plots;
    return { command, state, status, startPlay, stopService, errors, first,
      ambient: ambient.checked_plot_id, patchbay: patchbay.checked_plot_id };
  } catch (error) {
    await stopService();
    throw new Error(`Installed Body proof stopped in ${state}. ${errors.join("")}`, { cause: error });
  }
}
