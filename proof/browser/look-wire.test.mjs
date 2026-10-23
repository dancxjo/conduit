import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import { decodeLook, decodeVisualLook } from "../../targets/browser/host/assets/application-look.mjs";

const original = () => Buffer.from(fs.readFileSync(new URL("../conformance/face-look/phosphor-look-v2.hex", import.meta.url), "utf8").replace(/\s/gu, ""), "hex");
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
  assert.equal(visual.typography, "editorial");
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
  assert.throws(() => read(changeU16("version", 1)), /look-visual-contract/u);
});

test("foreign visual identity and injected semantic fields refuse", () => {
  const identity = original(), at = identity.indexOf(Buffer.from("conduit.look/phosphor@1"));
  assert(at > 0); identity[at] = 120;
  assert.throws(() => read(identity), /look-visual-contract/u);
  const action = original(), field = action.lastIndexOf(Buffer.from("spoken"));
  assert(field > 0); action.set(Buffer.from("action"), field);
  assert.throws(() => read(action), /look-field-identity/u);
});

function prefixText(bytes, text, at = bytes.lastIndexOf(Buffer.from(text))) {
  assert(at >= 4);
  assert.equal(bytes.readUInt32LE(at - 4), Buffer.byteLength(text));
  const patched = Buffer.concat([bytes.subarray(0, at), Buffer.from([0xef, 0xbb, 0xbf]), bytes.subarray(at)]);
  patched.writeUInt32LE(Buffer.byteLength(text) + 3, at - 4);
  return patched;
}

test("Look field identities cannot hide a leading BOM", () => {
  assert.throws(() => read(prefixText(original(), "spoken")), /look-field-identity/u);
});

test("Look identity text preserves leading BOM instead of creating an alias", () => {
  const bytes = original(), identity = "conduit.look/phosphor@1";
  assert.throws(() => read(prefixText(bytes, identity, bytes.indexOf(Buffer.from(identity)))), /look-visual-contract/u);
});

test("Look byte admission refuses coercion and checks bounds before conversion", () => {
  const bytes = original();
  const over = new Array(65_537);
  Object.defineProperty(over, 0, { get() { throw new Error("overcapacity input read"); } });
  assert.throws(() => read(over), /look-oversized-encoding/u);
  for (const value of [65_537, {}, { length: bytes.length }, new Uint16Array(bytes),
    [...bytes.slice(0, -1), bytes.at(-1) + 256], [...bytes.slice(0, -1), String(bytes.at(-1))]]) {
    assert.throws(() => read(value), /look-malformed-encoding/u);
  }
  const sparse = Array.from(bytes); delete sparse[0];
  assert.throws(() => read(sparse), /look-malformed-encoding/u);
  for (const value of [bytes, Array.from(bytes), Uint8Array.from(bytes).buffer,
    Buffer.concat([Buffer.from([99]), bytes, Buffer.from([99])]).subarray(1, -1)]) {
    assert.equal(read(value).identity, "conduit.look/phosphor@1");
  }
});

test("historical v1 Look wire refuses instead of acquiring implicit typography", () => {
  const historical = Buffer.from(fs.readFileSync(new URL('../conformance/face-look/phosphor-look-v1.hex', import.meta.url), 'utf8').replace(/\s/gu, ''), 'hex');
  assert.throws(() => read(historical), /look-canonical-type-required-retired-theme-wire/u);
});
