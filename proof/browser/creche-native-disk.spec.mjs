import { createHash } from "node:crypto";
import { expect, test } from "@playwright/test";
import {
  bindBodyProvisionedMedia,
  NATIVE_MEDIA_PROVISION_BYTES,
  readBodyProvisionedMedia,
} from "../../products/creche/browser/creche-native-disk.mjs";

test("two Bodies produce distinct native disk images with recoverable provisioning", async () => {
  const image = new Uint8Array(2048);
  image[510] = 0x55;
  image[511] = 0xaa;
  const first = await bindBodyProvisionedMedia({ prepared: prepared("one"), imageBytes: image, filename: "one.img", format: "img", mediaType: "application/x-raw-disk-image" });
  const second = await bindBodyProvisionedMedia({ prepared: prepared("two"), imageBytes: image, filename: "two.img", format: "img", mediaType: "application/x-raw-disk-image" });

  expect(first.bytes.byteLength).toBe(image.byteLength + NATIVE_MEDIA_PROVISION_BYTES);
  expect(Array.from(first.bytes.subarray(0, image.byteLength))).toEqual(Array.from(image));
  expect(first.bytes[510]).toBe(0x55);
  expect(first.bytes[511]).toBe(0xaa);
  expect(first.content_digest).toBe(`sha256:${createHash("sha256").update(first.bytes).digest("hex")}`);
  expect(second.content_digest).not.toBe(first.content_digest);

  const recovered = readBodyProvisionedMedia(first.bytes);
  expect(recovered.provision).toMatchObject({
    schema: "conduit.spore/native-media-provision@1",
    image_bytes: image.byteLength,
    spore: { spore_id: "spore:one", body_id: "body:one", binding: { invitation_id: "invitation:one" } },
    invitation_provision: { invitation_id: "invitation:one", expires_at_millis: 1_800_000_000_000 },
  });
  expect(Array.from(recovered.image)).toEqual(Array.from(image));
});

test("missing and malformed native media provisioning remain refusals", async () => {
  expect(() => readBodyProvisionedMedia(new Uint8Array(8192))).toThrow(/omitted its Body provision/);
  const native = await bindBodyProvisionedMedia({
    prepared: prepared("stale"),
    imageBytes: new Uint8Array(2048),
    filename: "stale.iso",
    format: "iso",
    mediaType: "application/x-iso9660-image",
  });
  const malformed = new Uint8Array(native.bytes);
  malformed.fill(0xff, malformed.byteLength - NATIVE_MEDIA_PROVISION_BYTES + 28, malformed.byteLength - NATIVE_MEDIA_PROVISION_BYTES + 32);
  expect(() => readBodyProvisionedMedia(malformed)).toThrow(/header is malformed/);
});

function prepared(suffix) {
  const digestByte = suffix === "one" ? "1" : suffix === "two" ? "2" : "3";
  const imageContentDigest = `sha256:${digestByte.repeat(64)}`;
  const invitationId = `invitation:${suffix}`;
  return {
    spore_id: `spore:${suffix}`,
    image_id: `image:${suffix}`,
    image_content_digest: imageContentDigest,
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
      image_content_digest: imageContentDigest,
    },
  };
}
