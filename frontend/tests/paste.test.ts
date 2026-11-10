import assert from "node:assert/strict";
import { test } from "node:test";

import { formatPastedPaths, pickImages } from "../src/paste.ts";

const png = { type: "image/png", name: "a.png" };
const jpeg = { type: "image/jpeg", name: "b.jpg" };
const pdf = { type: "application/pdf", name: "c.pdf" };

test("picks only image files", () => {
  assert.deepEqual(pickImages([png, pdf, jpeg], ""), [png, jpeg]);
  assert.deepEqual(pickImages([pdf], ""), []);
  assert.deepEqual(pickImages([], ""), []);
});

test("clipboard text takes precedence over images", () => {
  assert.deepEqual(pickImages([png], "hello"), []);
  assert.deepEqual(pickImages([png], "  \n"), [png]);
});

test("formats pasted paths", () => {
  assert.equal(formatPastedPaths(["/home/u/.cache/termdeck/paste/p.png"]), "/home/u/.cache/termdeck/paste/p.png");
  assert.equal(formatPastedPaths(["/a/x.png", "/b c/y.png"]), "/a/x.png /b\\ c/y.png");
});
