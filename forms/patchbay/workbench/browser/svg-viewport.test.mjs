import assert from "node:assert/strict";
import test from "node:test";
import { SvgViewportModel } from "./svg-viewport.js";

test("diagram viewBox zoom stays centered on the requested focus", () => {
  const model = new SvgViewportModel([0, 0, 100, 50]);
  model.zoomAt(2, 0.25, 0.75);
  assert.deepEqual(model.current, [12.5, 18.75, 50, 25]);
  assert.equal(model.zoom, 2);
});

test("diagram pan converts viewport pixels into viewBox coordinates", () => {
  const model = new SvgViewportModel([0, 0, 100, 50]);
  model.zoomAt(2);
  model.panPixels(10, -20, 1000, 500);
  assert.deepEqual(model.current, [24.5, 13.5, 50, 25]);
  model.reset();
  assert.deepEqual(model.current, [0, 0, 100, 50]);
});

test("diagram zoom is finitely bounded", () => {
  const model = new SvgViewportModel([0, 0, 100, 50]);
  model.zoomAt(100);
  assert.equal(model.zoom, 8);
  model.zoomAt(0.001);
  assert.equal(model.zoom, 1);
});
