import assert from "node:assert/strict";
import { test } from "node:test";

import {
  DEFAULT_SIDEBAR_WIDTH,
  MAX_SIDEBAR_WIDTH,
  MINI_COLLAPSE_THRESHOLD,
  MIN_SIDEBAR_WIDTH,
  clampSidebarWidth,
  resolveSidebarDrag,
} from "../src/sidebarResize.ts";

test("clampSidebarWidth: falls back to default for invalid numbers", () => {
  assert.equal(clampSidebarWidth(Number.NaN), DEFAULT_SIDEBAR_WIDTH);
  assert.equal(clampSidebarWidth(Number.POSITIVE_INFINITY), DEFAULT_SIDEBAR_WIDTH);
});

test("clampSidebarWidth: clamps within min and max bounds", () => {
  assert.equal(clampSidebarWidth(100), MIN_SIDEBAR_WIDTH);
  assert.equal(clampSidebarWidth(360), 360);
  assert.equal(clampSidebarWidth(900), MAX_SIDEBAR_WIDTH);
});

test("clampSidebarWidth: respects 50% viewport cap when viewportWidth is given", () => {
  assert.equal(clampSidebarWidth(500, 800), 400);
  assert.equal(clampSidebarWidth(500, 1440), 500);
  assert.equal(clampSidebarWidth(300, 400), MIN_SIDEBAR_WIDTH);
});

test("resolveSidebarDrag: collapses to mini below threshold and clamps above", () => {
  assert.deepEqual(resolveSidebarDrag(MINI_COLLAPSE_THRESHOLD - 1, 1280), { mini: true });
  assert.deepEqual(resolveSidebarDrag(80, 1280), { mini: true });
  assert.deepEqual(resolveSidebarDrag(MINI_COLLAPSE_THRESHOLD, 1280), {
    mini: false,
    width: MIN_SIDEBAR_WIDTH,
  });
  assert.deepEqual(resolveSidebarDrag(380, 1280), {
    mini: false,
    width: 380,
  });
  assert.deepEqual(resolveSidebarDrag(750, 1280), {
    mini: false,
    width: MAX_SIDEBAR_WIDTH,
  });
});
