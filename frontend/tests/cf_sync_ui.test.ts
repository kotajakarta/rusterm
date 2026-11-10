import assert from "node:assert/strict";
import { test } from "node:test";
import {
  formatSyncStats,
  formatSyncTime,
  getDefaultDeviceName,
  validateSyncPin,
} from "../src/syncLogic.ts";

test("validateSyncPin: enforces minimum 4 characters", () => {
  assert.equal(validateSyncPin(""), "PIN enkripsi tidak boleh kosong.");
  assert.equal(validateSyncPin("123"), "PIN enkripsi minimal 4 karakter.");
  assert.equal(validateSyncPin("1234"), null);
  assert.equal(validateSyncPin("secret-pass-99"), null);
});

test("formatSyncStats: builds readable diff report", () => {
  const emptyStats = { servers_added: 0, servers_updated: 0, bookmarks_added: 0, snippets_added: 0 };
  assert.equal(formatSyncStats(emptyStats), "Semua data sudah sinkron (tidak ada entri baru).");

  const diffStats = { servers_added: 2, servers_updated: 1, bookmarks_added: 3, snippets_added: 0 };
  assert.equal(
    formatSyncStats(diffStats),
    "+2 server ditambahkan, 1 server diperbarui, +3 bookmark ditambahkan"
  );
});

test("formatSyncTime: formats epoch seconds gracefully", () => {
  const nowSec = Math.floor(Date.now() / 1000);
  assert.equal(formatSyncTime(nowSec - 10), "Baru saja");
  assert.equal(formatSyncTime(nowSec - 120), "2 menit yang lalu");
  assert.equal(formatSyncTime(nowSec - 7200), "2 jam yang lalu");
  assert.ok(formatSyncTime(0).length > 0);
});

test("getDefaultDeviceName: returns fallback if not Android", () => {
  assert.equal(typeof getDefaultDeviceName(), "string");
  assert.ok(getDefaultDeviceName().length > 0);
});
