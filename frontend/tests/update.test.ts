import assert from "node:assert/strict";
import { test } from "node:test";

import type { UpdateCheckResult, UpdateProgressResult } from "../src/types.ts";
import { formatUpdateBytes, getUpdateBadge, getUpdateStatusText } from "../src/updateLogic.ts";

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

test("getUpdateStatusText: formats progress states accurately", () => {
  const downloading: UpdateProgressResult = {
    state: "downloading",
    progress_percent: 50,
    downloaded_bytes: 10 * 1024 * 1024,
    total_bytes: 20 * 1024 * 1024,
    message: "",
  };
  assert.match(getUpdateStatusText(downloading), /Mengunduh \(10.0 MB \/ 20.0 MB - 50%\)/);

  const applying: UpdateProgressResult = {
    state: "applying",
    progress_percent: 100,
    downloaded_bytes: 20 * 1024 * 1024,
    total_bytes: 20 * 1024 * 1024,
    message: "Memasang binary...",
  };
  assert.equal(getUpdateStatusText(applying), "Memasang binary...");

  const completed: UpdateProgressResult = {
    state: "completed",
    progress_percent: 100,
    downloaded_bytes: 20 * 1024 * 1024,
    total_bytes: 20 * 1024 * 1024,
    message: "Selesai!",
  };
  assert.equal(getUpdateStatusText(completed), "Selesai!");

  const error: UpdateProgressResult = {
    state: "error",
    progress_percent: 0,
    downloaded_bytes: 0,
    total_bytes: 0,
    message: "Koneksi terputus",
  };
  assert.equal(getUpdateStatusText(error), "Gagal: Koneksi terputus");
});
