import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import { decodeLook, decodeVisualLook } from "../../targets/browser/host/assets/application-look.mjs";

const original = () => Buffer.from(fs.readFileSync(new URL("../conformance/face-look/phosphor-look-v1.hex", import.meta.url), "utf8").replace(/\s/gu, ""), "hex");
const reject = code => { throw new Error(code); };
const read = bytes => decodeVisualLook(bytes, reject);

function changeU16(name, value) {
  const bytes = original(), at = bytes.lastIndexOf(Buffer.from(name));
  assert(at > 0);
  const marker = at + name.length;
  assert.equal(bytes[marker], 0);
  assert.equal(bytes.readUInt32LE(marker + 1), 2);
  bytes.writeUInt16LE(value, marker + 5);
  return bytes;
}

test("canonical Rust Look preserves paint and independent modality parameters", () => {
  const look = decodeLook(original());
  assert.equal(look.identity, "conduit.look/phosphor@1");
  assert.equal(look.spoken.payload.verbosity.tag, "brief");
  assert.equal(look.generative.payload.register.tag, "neutral");
  const visual = read(original());
  assert.equal(visual.tokens.background, "#05070b");
  assert.equal(visual.tokens["type-body"], "16px");
  assert.equal(visual.tokens["focus-width"], "3px");
  assert(!Object.hasOwn(visual.tokens, "responsive-breakpoint"));
  assert(!Object.hasOwn(visual.tokens, "responsive-grid-min"));
});

test("retired theme, altered type, truncation, trailing data and oversize refuse", () => {
  for (const bytes of [new Uint8Array([2, 23, 99, 111, 110]), original().subarray(0, -1),
    Buffer.concat([original(), Buffer.from([0])]), new Uint8Array(65537)]) {
    assert.throws(() => read(bytes), /look-/u);
  }
  const forged = original(); forged[0] = 0;
  assert.throws(() => read(forged), /canonical-type-required/u);
});

test("Source metric and version bounds cannot be bypassed with canonical type bytes", () => {
  assert.throws(() => read(changeU16("focus_width_px", 0)), /look-token-bound/u);
  assert.throws(() => read(changeU16("type_body_px", 65)), /look-token-bound/u);
  assert.throws(() => read(changeU16("text_scale_percent", 99)), /look-visual-contract/u);
  assert.throws(() => read(changeU16("version", 2)), /look-visual-contract/u);
});

test("foreign visual identity and injected semantic fields refuse", () => {
  const identity = original(), at = identity.indexOf(Buffer.from("conduit.look/phosphor@1"));
  assert(at > 0); identity[at] = 120;
  assert.throws(() => read(identity), /look-visual-contract/u);
  const action = original(), field = action.lastIndexOf(Buffer.from("spoken"));
  assert(field > 0); action.set(Buffer.from("action"), field);
  assert.throws(() => read(action), /look-field-identity/u);
});
