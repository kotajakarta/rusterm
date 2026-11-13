import assert from "node:assert/strict";
import { test } from "node:test";

import { TOOLS_TABS, TOOLS_TAB_KEY, computeTrayPosition, parseToolsTab } from "../src/toolsTray.ts";

test("TOOLS_TAB_KEY and TOOLS_TABS are stable", () => {
  assert.equal(TOOLS_TAB_KEY, "rusterm.toolsTab");
  assert.deepEqual([...TOOLS_TABS], ["snippets", "cloudflare", "tailscale"]);
});

test("parseToolsTab: accepts known tabs", () => {
  assert.equal(parseToolsTab("snippets"), "snippets");
  assert.equal(parseToolsTab("cloudflare"), "cloudflare");
  assert.equal(parseToolsTab("tailscale"), "tailscale");
});

test("parseToolsTab: falls back to snippets for missing or unknown values", () => {
  assert.equal(parseToolsTab(null), "snippets");
  assert.equal(parseToolsTab(""), "snippets");
  assert.equal(parseToolsTab("x"), "snippets");
  assert.equal(parseToolsTab("Cloudflare"), "snippets");
});

test("computeTrayPosition: anchors above the button on a normal desktop", () => {
  assert.deepEqual(computeTrayPosition({ top: 700 }, 0, { width: 1280, height: 800 }), {
    left: 8,
    bottom: 106,
    width: 380,
    maxHeight: 560,
  });
});

test("computeTrayPosition: follows the sidebar left edge", () => {
  assert.equal(computeTrayPosition({ top: 700 }, 40, { width: 1280, height: 800 }).left, 48);
});

test("computeTrayPosition: clamps width on narrow viewports", () => {
  const pos = computeTrayPosition({ top: 500 }, 0, { width: 300, height: 600 });
  assert.equal(pos.width, 284);
  assert.equal(pos.maxHeight, 420);
});

test("computeTrayPosition: keeps a usable minimum height on short viewports", () => {
  assert.equal(computeTrayPosition({ top: 100 }, 0, { width: 1280, height: 200 }).maxHeight, 160);
});
