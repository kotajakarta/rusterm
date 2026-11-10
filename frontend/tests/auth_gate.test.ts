import assert from "node:assert/strict";
import { test } from "node:test";

import type { AuthStatus } from "../src/types.ts";

export type Gate = "checking" | "setup" | "locked" | "force-change-pin" | "unlocked";

export function resolveGate(status: AuthStatus): Gate {
  if (!status.initialized) {
    return "setup";
  }
  if (status.session_valid) {
    return status.must_change_pin ? "force-change-pin" : "unlocked";
  }
  return "locked";
}

export function validateNewPinInput(pin: string, confirmPin: string): string | null {
  if (pin.length < 4) return "PIN minimal 4 digit angka.";
  if (pin === "7821724") return "PIN tidak boleh menggunakan PIN default lama (7821724).";
  if (pin !== confirmPin) return "Konfirmasi PIN tidak cocok.";
  return null;
}

test("resolveGate: routes uninitialized vault to setup gate", () => {
  const status: AuthStatus = {
    unlocked: false,
    session_valid: false,
    initialized: false,
    must_change_pin: false,
  };
  assert.equal(resolveGate(status), "setup");
});

test("resolveGate: routes locked initialized vault to locked gate", () => {
  const status: AuthStatus = {
    unlocked: false,
    session_valid: false,
    initialized: true,
    must_change_pin: false,
  };
  assert.equal(resolveGate(status), "locked");
});

test("resolveGate: routes session needing pin change to force-change-pin gate", () => {
  const status: AuthStatus = {
    unlocked: true,
    session_valid: true,
    initialized: true,
    must_change_pin: true,
  };
  assert.equal(resolveGate(status), "force-change-pin");
});

test("resolveGate: routes valid session without must_change_pin to unlocked gate", () => {
  const status: AuthStatus = {
    unlocked: true,
    session_valid: true,
    initialized: true,
    must_change_pin: false,
  };
  assert.equal(resolveGate(status), "unlocked");
});

test("validateNewPinInput: enforces min 4 digits, mismatch, and rejects legacy 7821724", () => {
  assert.match(validateNewPinInput("12", "12") ?? "", /minimal 4 digit/);
  assert.match(validateNewPinInput("123456", "654321") ?? "", /tidak cocok/);
  assert.match(validateNewPinInput("7821724", "7821724") ?? "", /PIN default lama/);
  assert.equal(validateNewPinInput("987654", "987654"), null);
});
