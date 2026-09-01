#!/usr/bin/env node
import { createHash } from "node:crypto";
import { copyFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { basename, join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const PRODUCTS = Object.freeze({
  "conduitos/x86_64/pc": Object.freeze({ architecture: "x86_64", machine: "q35", output: "conduitos-x86_64-pc.iso", manifest: "conduitos-x86_64-pc-release.json", builder: "conduit-host-conduitos/build-x86_64@1", deployment: "conduit-host-conduitos/boot-x86_64@1" }),
  "conduitos/aarch64/virt": Object.freeze({ architecture: "aarch64", machine: "virt", output: "conduitos-aarch64-virt.iso", manifest: "conduitos-aarch64-virt-release.json", builder: "conduit-host-conduitos/build-aarch64@1", deployment: "conduit-host-conduitos/boot-aarch64@1" }),
});

export async function sealConduitOsCrecheRelease({ buildRoot, output }) {
  await mkdir(output, { recursive: true });
  const releases = [];
  for (const [targetId, product] of Object.entries(PRODUCTS)) {
    const directory = join(buildRoot, product.architecture);
    const build = JSON.parse(await readFile(join(directory, "build-manifest.json"), "utf8"));
    requireBuild(build, targetId, product);
    const sourceImage = join(directory, build.image.file);
    const bytes = await readFile(sourceImage);
    const digest = `sha256:${createHash("sha256").update(bytes).digest("hex")}`;
    if (digest !== build.image.sha256 || build.image_id !== `image:${digest}`) throw new Error(`stale ${targetId} IMAGE content identity`);
    await copyFile(sourceImage, join(output, product.output));
    const release = {
      schema: "conduit.conduitos/creche-release@1",
      target_id: targetId,
      architecture: product.architecture,
      machine: product.machine,
      artifact_role: "product-host",
      fabrication_package_id: "conduitos-image@1",
      output: "disk-image",
      builder_adapter: product.builder,
      deployment_adapter: product.deployment,
      boot_mechanism: "uefi-limine-hybrid-iso",
      profile_id: build.profile_id,
      build_id: build.build_id,
      image_id: build.image_id,
      source_identity: build.source_identity,
      toolchain_identity: build.toolchain_identity,
      boot_assets: build.boot_assets,
      artifact: { role: build.image.role, format: "hybrid-iso", path: product.output, bytes: bytes.byteLength, sha256: digest },
      expected_offers: build.resolved_build.host_operations,
      bounds: build.resolved_build.bounds,
      boot_claimed: false,
      physical_proof_claimed: false,
    };
    await writeFile(join(output, product.manifest), `${JSON.stringify(release, null, 2)}\n`);
    releases.push(release);
  }
  return Object.freeze(releases);
}

function requireBuild(build, targetId, product) {
  if (build?.schema !== "conduit.host/target-build-manifest@3" || build.target !== targetId || build.artifact_role !== "product-host") throw new Error(`${targetId} BUILD is not an exact product Host manifest`);
  if (build.image?.role !== "final-bootable-image" || basename(build.image?.file ?? "") !== build.image?.file) throw new Error(`${targetId} BUILD omitted its final bootable IMAGE`);
  if (build.boot_assets?.architecture !== product.architecture || build.boot_assets?.machine !== product.machine) throw new Error(`${targetId} BUILD has the wrong architecture or machine`);
  if (typeof build.boot_assets?.firmware !== "string" || typeof build.boot_assets?.boot_entry !== "string") throw new Error(`${targetId} BUILD omitted firmware or bootloader identity`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const [buildRoot, output] = process.argv.slice(2);
  if (!buildRoot || !output) throw new Error("usage: seal-creche-release.mjs BUILD_ROOT OUTPUT");
  await sealConduitOsCrecheRelease({ buildRoot, output });
}
