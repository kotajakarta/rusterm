import assert from "node:assert/strict";
import { test } from "node:test";

import { applyModifiers, specialKeySequence, toCtrlChar } from "../src/mobileKeys.ts";

test("toCtrlChar maps letters and punctuation to ASCII control codes", () => {
  assert.equal(toCtrlChar("a"), "\x01");
  assert.equal(toCtrlChar("c"), "\x03");
  assert.equal(toCtrlChar("C"), "\x03");
  assert.equal(toCtrlChar("d"), "\x04");
  assert.equal(toCtrlChar("l"), "\x0c");
  assert.equal(toCtrlChar("z"), "\x1a");
  assert.equal(toCtrlChar("["), "\x1b");
  assert.equal(toCtrlChar("\\"), "\x1c");
  assert.equal(toCtrlChar("]"), "\x1d");
  assert.equal(toCtrlChar("/"), "\x1f");
  assert.equal(toCtrlChar("_"), "\x1f");
});

test("applyModifiers transforms single-key input when sticky Ctrl or Alt is active", () => {
  assert.deepEqual(applyModifiers("c", { ctrl: true, alt: false }), {
    data: "\x03",
    consumed: true,
  });
  assert.deepEqual(applyModifiers("f", { ctrl: false, alt: true }), {
    data: "\x1bf",
    consumed: true,
  });
  assert.deepEqual(applyModifiers("d", { ctrl: true, alt: true }), {
    data: "\x1b\x04",
    consumed: true,
  });
  assert.deepEqual(applyModifiers("hello", { ctrl: false, alt: false }), {
    data: "hello",
    consumed: false,
  });
});

test("specialKeySequence respects application cursor keys mode and modifiers", () => {
  assert.equal(specialKeySequence("Esc"), "\x1b");
  assert.equal(specialKeySequence("Tab"), "\t");
  assert.equal(specialKeySequence("Up", false), "\x1b[A");
  assert.equal(specialKeySequence("Up", true), "\x1bOA");
  assert.equal(specialKeySequence("Down", false), "\x1b[B");
  assert.equal(specialKeySequence("Right", true), "\x1bOC");
  assert.equal(specialKeySequence("Left", false), "\x1b[D");
  assert.equal(specialKeySequence("Home", false), "\x1b[H");
  assert.equal(specialKeySequence("Home", true), "\x1bOH");
  assert.equal(specialKeySequence("End", false), "\x1b[F");
  assert.equal(specialKeySequence("End", true), "\x1bOF");
  assert.equal(specialKeySequence("PgUp"), "\x1b[5~");
  assert.equal(specialKeySequence("PgDn"), "\x1b[6~");
  assert.equal(specialKeySequence("CtrlC"), "\x03");
  assert.equal(specialKeySequence("CtrlD"), "\x04");
  assert.equal(specialKeySequence("CtrlZ"), "\x1a");
  assert.equal(specialKeySequence("CtrlL"), "\x0c");
  assert.equal(specialKeySequence("Hyphen"), "-");
  assert.equal(specialKeySequence("Slash"), "/");
  assert.equal(specialKeySequence("Pipe"), "|");
  assert.equal(specialKeySequence("Tilde"), "~");
  // Ctrl+Left / Ctrl+Right word movement
  assert.equal(specialKeySequence("Left", false, { ctrl: true, alt: false }), "\x1b[1;5D");
  assert.equal(specialKeySequence("Right", false, { ctrl: true, alt: false }), "\x1b[1;5C");
});
