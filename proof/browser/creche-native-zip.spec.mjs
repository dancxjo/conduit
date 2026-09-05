import { createHash } from "node:crypto";
import { expect, test } from "@playwright/test";
import {
  createBodyBoundZip,
  readBodyBoundZip,
} from "../../products/creche/browser/creche-native-zip.mjs";

test("two Bodies produce distinct ordinary ZIP packages with recoverable provisioning", async () => {
  const release = Object.freeze({
    payloads: Object.freeze([
      Object.freeze({ path: "runtime.wasm", bytes: new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]), media_type: "application/wasm" }),
      Object.freeze({ path: "index.html", bytes: new TextEncoder().encode("<!doctype html><title>Conduit</title>") }),
    ]),
  });
  const first = await createBodyBoundZip({ prepared: prepared("one"), release, filename: "one-browser.zip" });
  const second = await createBodyBoundZip({ prepared: prepared("two"), release, filename: "two-browser.zip" });

  expect(new TextDecoder().decode(first.bytes.subarray(0, 4))).toBe("PK\u0003\u0004");
  expect(first.content_digest).toBe(`sha256:${createHash("sha256").update(first.bytes).digest("hex")}`);
  expect(second.content_digest).not.toBe(first.content_digest);
  expect(Array.from(second.bytes)).not.toEqual(Array.from(first.bytes));

  const firstPackage = readBodyBoundZip(first.bytes);
  const secondPackage = readBodyBoundZip(second.bytes);
  expect(Array.from(firstPackage.entries.keys())).toEqual(["runtime.wasm", "index.html", "conduit-spore.json"]);
  expect(firstPackage.modes.get("runtime.wasm")).toBe(0o100644);
  expect(firstPackage.provision).toMatchObject({
    schema: "conduit.spore/native-package-provision@1",
    spore: { spore_id: "spore:one", body_id: "body:one", binding: { invitation_id: "invitation:one" } },
    invitation_provision: { invitation_id: "invitation:one", expires_at_millis: 1_800_000_000_000 },
  });
  expect(secondPackage.provision.spore.body_id).toBe("body:two");
  expect(secondPackage.provision.invitation_provision.secret).not.toEqual(firstPackage.provision.invitation_provision.secret);
});

test("ZIP corruption and malformed provisioning remain refusals", async () => {
  const native = await createBodyBoundZip({
    prepared: prepared("stale"),
    release: { payloads: [{ path: "runtime.wasm", bytes: new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]) }] },
    filename: "stale-browser.zip",
  });
  const corrupted = new Uint8Array(native.bytes);
  corrupted[31 + "runtime.wasm".length] ^= 0xff;
  expect(() => readBodyBoundZip(corrupted)).toThrow(/CRC-32/);

  const wrongInvitation = prepared("wrong");
  wrongInvitation.spore_manifest.binding.invitation_id = "invitation:somebody-else";
  const malformed = await createBodyBoundZip({
    prepared: wrongInvitation,
    release: { payloads: [{ path: "host", bytes: new Uint8Array([1]) }] },
    filename: "wrong-host.zip",
  });
  expect(() => readBodyBoundZip(malformed.bytes)).toThrow(/lost its exact identities/);
});

function prepared(suffix) {
  const imageContentDigest = `sha256:${suffix === "one" ? "1" : suffix === "two" ? "2" : "3".repeat(64)}`;
  const normalizedDigest = imageContentDigest.length === 71
    ? imageContentDigest
    : `sha256:${imageContentDigest.slice(7).padEnd(64, imageContentDigest.at(-1))}`;
  const invitationId = `invitation:${suffix}`;
  return {
    spore_id: `spore:${suffix}`,
    image_id: `image:${suffix}`,
    image_content_digest: normalizedDigest,
    invitation_id: invitationId,
    invitation_nonce: Array(32).fill(suffix.charCodeAt(0)),
    invitation_expires_at_millis: 1_800_000_000_000,
    invitation_secret: Array(32).fill(suffix.charCodeAt(suffix.length - 1)),
    spore_manifest: {
      schema: "conduit.body/spore-manifest@2",
      spore_id: `spore:${suffix}`,
      body_id: `body:${suffix}`,
      binding: { mode: "self-joining", invitation_id: invitationId },
      image_id: `image:${suffix}`,
      image_content_digest: normalizedDigest,
    },
  };
}
