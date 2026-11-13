import assert from "node:assert/strict";
import { test } from "node:test";

import {
  DEFAULT_IDLE_LOCK_MINUTES,
  IDLE_LOCK_OPTIONS,
  formatIdleLockLabel,
  isIdleExpired,
  normalizeIdleLockMinutes,
} from "../src/idleLock.ts";

test("normalizeIdleLockMinutes: accepts valid options and falls back to default", () => {
  for (const opt of IDLE_LOCK_OPTIONS) {
    assert.equal(normalizeIdleLockMinutes(opt), opt);
    assert.equal(normalizeIdleLockMinutes(String(opt)), opt);
  }
  assert.equal(normalizeIdleLockMinutes(null), DEFAULT_IDLE_LOCK_MINUTES);
  assert.equal(normalizeIdleLockMinutes(""), DEFAULT_IDLE_LOCK_MINUTES);
  assert.equal(normalizeIdleLockMinutes("999"), DEFAULT_IDLE_LOCK_MINUTES);
  assert.equal(normalizeIdleLockMinutes(-5), DEFAULT_IDLE_LOCK_MINUTES);
});

test("isIdleExpired: returns false when disabled or before threshold, true at/after threshold", () => {
  const t0 = 1_000_000;
  assert.equal(isIdleExpired(t0, t0 + 999_999_999, 0), false);
  assert.equal(isIdleExpired(t0, t0 - 5000, 15), false);
  assert.equal(isIdleExpired(t0, t0 + 15 * 60_000 - 1, 15), false);
  assert.equal(isIdleExpired(t0, t0 + 15 * 60_000, 15), true);
  assert.equal(isIdleExpired(t0, t0 + 20 * 60_000, 15), true);
});

test("formatIdleLockLabel: formats human-readable Indonesian labels", () => {
  assert.equal(formatIdleLockLabel(0), "Nonaktif");
  assert.equal(formatIdleLockLabel(15), "15 menit");
  assert.equal(formatIdleLockLabel(30), "30 menit");
});
