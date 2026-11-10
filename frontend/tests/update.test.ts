import assert from "node:assert/strict";
import { test } from "node:test";

import type { UpdateCheckResult, UpdateProgressResult } from "../src/types.ts";
import { formatUpdateBytes, getUpdateBadge } from "../src/updateLogic.ts";

test("formatUpdateBytes: formats byte sizes accurately", () => {
  assert.equal(formatUpdateBytes(0), "0 B");
  assert.equal(formatUpdateBytes(512), "512 B");
  assert.equal(formatUpdateBytes(1024 * 1024), "1.0 MB");
  assert.equal(formatUpdateBytes(25 * 1024 * 1024), "25.0 MB");
});

test("getUpdateBadge: returns update available badge when has_update is true", () => {
  const check: UpdateCheckResult = {
    platform: "desktop",
    current_version: "830a693",
    latest_version: "abcdef1",
    has_update: true,
    release_title: "feat: new update",
    release_notes: "Changelog",
    download_url: null,
  };
  const badge = getUpdateBadge(check);
  assert.equal(badge.label, "Pembaruan Tersedia");
  assert.equal(badge.type, "warning");
});

test("getUpdateBadge: returns up to date badge when has_update is false", () => {
  const check: UpdateCheckResult = {
    platform: "android",
    current_version: "1.0.0",
    latest_version: "1.0.0",
    has_update: false,
    release_title: "v1.0.0",
    release_notes: "Latest",
    download_url: null,
  };
  const badge = getUpdateBadge(check);
  assert.equal(badge.label, "Versi Terbaru");
  assert.equal(badge.type, "success");
});
