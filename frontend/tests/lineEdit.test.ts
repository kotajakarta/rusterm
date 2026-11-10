import assert from "node:assert/strict";
import { test } from "node:test";

import { deleteCommandSequence, deleteSelectionSequence, moveCursorSequence, selectAllRange } from "../src/lineEdit.ts";
import type { BufferLike, CellLike, LineLike } from "../src/lineEdit.ts";

const LEFT = "\x1b[D";
const RIGHT = "\x1b[C";
const BS = "\x7f";

/**
 * Fake buffer: each row is a string padded with never-written cells. A "中" takes
 * two cells (the second one empty with width 0), like xterm stores wide characters.
 */
function buffer(cols: number, rows: string[], wrapped: number[], cursor: { x: number; y: number }): BufferLike {
  const lines: LineLike[] = rows.map((text, y) => {
    const cells: CellLike[] = [];
    for (const ch of text) {
      if (ch === "中") {
        cells.push({ getChars: () => ch, getWidth: () => 2 });
        cells.push({ getChars: () => "", getWidth: () => 0 });
      } else {
        cells.push({ getChars: () => ch, getWidth: () => 1 });
      }
    }
    while (cells.length < cols) cells.push({ getChars: () => "", getWidth: () => 1 });
    return { isWrapped: wrapped.includes(y), length: cols, getCell: (x: number) => cells[x] };
  });
  return { cursorX: cursor.x, cursorY: cursor.y, baseY: 0, getLine: (y: number) => lines[y] };
}

test("click on the input line moves the cursor by characters", () => {
  const buf = buffer(20, ["old output", "$ echo hello"], [], { x: 12, y: 1 });
  assert.equal(moveCursorSequence(buf, { x: 7, y: 1 }, false), LEFT.repeat(5));
  assert.equal(moveCursorSequence(buf, { x: 12, y: 1 }, false), "");
});

test("click past the end of the text clamps to the text end", () => {
  const buf = buffer(20, ["$ echo hello"], [], { x: 4, y: 0 });
  assert.equal(moveCursorSequence(buf, { x: 18, y: 0 }, false), RIGHT.repeat(8));
});

test("click outside the input line does nothing", () => {
  const buf = buffer(20, ["old output", "$ echo hello"], [], { x: 12, y: 1 });
  assert.equal(moveCursorSequence(buf, { x: 3, y: 0 }, false), null);
});

test("click follows wrapped rows of a long command", () => {
  const buf = buffer(10, ["$ abcdefgh", "ijklmn"], [1], { x: 6, y: 1 });
  // From after "n" (index 16) back to before "c" (index 4).
  assert.equal(moveCursorSequence(buf, { x: 4, y: 0 }, false), LEFT.repeat(12));
  // The cursor on the first row reaches the wrapped row too.
  const top = buffer(10, ["$ abcdefgh", "ijklmn"], [1], { x: 2, y: 0 });
  assert.equal(moveCursorSequence(top, { x: 2, y: 1 }, false), RIGHT.repeat(10));
});

test("a full row continues the command even when readline's redraw dropped the wrap flag", () => {
  // Same screen as above, but row 1 is not marked wrapped.
  const bottom = buffer(10, ["$ abcdefgh", "ijklmn"], [], { x: 6, y: 1 });
  assert.equal(moveCursorSequence(bottom, { x: 4, y: 0 }, false), LEFT.repeat(12));
  assert.deepEqual(selectAllRange(bottom), { column: 2, row: 0, length: 14 });
  const top = buffer(10, ["$ abcdefgh", "ijklmn"], [], { x: 2, y: 0 });
  assert.equal(moveCursorSequence(top, { x: 2, y: 1 }, false), RIGHT.repeat(10));
});

test("a full-width output row above the prompt is not part of the command", () => {
  const buf = buffer(10, ["0123456789", "$ ls -l"], [], { x: 7, y: 1 });
  assert.equal(moveCursorSequence(buf, { x: 3, y: 0 }, false), null);
  assert.deepEqual(selectAllRange(buf), { column: 2, row: 1, length: 5 });
});

test("wide characters count as one arrow press", () => {
  const buf = buffer(20, ["$ 中中ab"], [], { x: 8, y: 0 });
  // Cells: "$"," ","中",_,"中",_,"a","b" — clicking before the first 中 is 4 characters back.
  assert.equal(moveCursorSequence(buf, { x: 2, y: 0 }, false), LEFT.repeat(4));
});

test("application cursor mode uses SS3 arrows", () => {
  const buf = buffer(20, ["$ ab"], [], { x: 4, y: 0 });
  assert.equal(moveCursorSequence(buf, { x: 2, y: 0 }, true), "\x1bOD\x1bOD");
});

test("deleting a selection moves to its end and erases it", () => {
  const buf = buffer(20, ["$ echo hello world"], [], { x: 18, y: 0 });
  // Selects "hello " (cells 7..13).
  const range = { start: { x: 7, y: 0 }, end: { x: 13, y: 0 } };
  assert.equal(deleteSelectionSequence(buf, range, false), LEFT.repeat(5) + BS.repeat(6));
});

test("selection after the cursor is reached by moving right", () => {
  const buf = buffer(20, ["$ echo hello"], [], { x: 2, y: 0 });
  const range = { start: { x: 7, y: 0 }, end: { x: 12, y: 0 } };
  assert.equal(deleteSelectionSequence(buf, range, false), RIGHT.repeat(10) + BS.repeat(5));
});

test("selection running past the text end only erases the text", () => {
  const buf = buffer(20, ["$ echo hi"], [], { x: 9, y: 0 });
  const range = { start: { x: 7, y: 0 }, end: { x: 20, y: 0 } };
  assert.equal(deleteSelectionSequence(buf, range, false), BS.repeat(2));
});

test("selection outside the input line is left to the terminal", () => {
  const buf = buffer(20, ["old output", "$ echo hi"], [], { x: 9, y: 1 });
  assert.equal(deleteSelectionSequence(buf, { start: { x: 0, y: 0 }, end: { x: 3, y: 0 } }, false), null);
  assert.equal(deleteSelectionSequence(buf, { start: { x: 2, y: 0 }, end: { x: 4, y: 1 } }, false), null);
});

test("selection of blank cells only is left to the terminal", () => {
  const buf = buffer(20, ["$ echo hi"], [], { x: 9, y: 0 });
  assert.equal(deleteSelectionSequence(buf, { start: { x: 12, y: 0 }, end: { x: 15, y: 0 } }, false), null);
});

test("select all covers the command after the prompt", () => {
  const buf = buffer(30, ["[user@host ~]$ ls -la /tmp"], [], { x: 26, y: 0 });
  assert.deepEqual(selectAllRange(buf), { column: 15, row: 0, length: 11 });
});

test("select all spans wrapped rows", () => {
  const buf = buffer(10, ["# abcdefgh", "ijk"], [1], { x: 3, y: 1 });
  assert.deepEqual(selectAllRange(buf), { column: 2, row: 0, length: 11 });
});

test("select all ignores prompt markers typed after the cursor", () => {
  // No marker before the cursor: the whole line is selected.
  const buf = buffer(20, ["user: echo $ x"], [], { x: 0, y: 0 });
  assert.deepEqual(selectAllRange(buf), { column: 0, row: 0, length: 14 });
});

test("select all with an empty command selects nothing", () => {
  const buf = buffer(20, ["old", "$ "], [], { x: 2, y: 1 });
  assert.equal(selectAllRange(buf), null);
});

test("select all then delete erases exactly the command", () => {
  const buf = buffer(30, ["[user@host ~]$ ls -la /tmp"], [], { x: 20, y: 0 });
  const sel = selectAllRange(buf);
  assert.ok(sel);
  const range = { start: { x: sel.column, y: sel.row }, end: { x: sel.column + sel.length, y: sel.row } };
  assert.equal(deleteSelectionSequence(buf, range, false), RIGHT.repeat(6) + BS.repeat(11));
});

test("deleting the whole command reads the current screen", () => {
  const buf = buffer(30, ["[user@host ~]$ ls -la /tmp"], [], { x: 20, y: 0 });
  assert.equal(deleteCommandSequence(buf, false), RIGHT.repeat(6) + BS.repeat(11));
  // A character echoed after Ctrl+A drew its selection is erased too.
  const later = buffer(30, ["[user@host ~]$ ls -la /tmpX"], [], { x: 27, y: 0 });
  assert.equal(deleteCommandSequence(later, false), BS.repeat(12));
  assert.equal(deleteCommandSequence(buffer(20, ["$ "], [], { x: 2, y: 0 }), false), null);
});
