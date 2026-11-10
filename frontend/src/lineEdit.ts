// Editor-style editing of the command line at a shell prompt. The text belongs to
// the remote line editor (readline and the like), so mouse actions are turned
// into the keys it understands: arrows to move, backspaces to erase.

/** The slice of xterm's IBuffer this module reads; structural so tests can fake it. */
export interface CellLike {
  getChars(): string;
  getWidth(): number;
}

export interface LineLike {
  readonly isWrapped: boolean;
  readonly length: number;
  getCell(x: number): CellLike | undefined;
}

export interface BufferLike {
  readonly cursorX: number;
  readonly cursorY: number;
  readonly baseY: number;
  getLine(y: number): LineLike | undefined;
}

/** A cell boundary in buffer coordinates (0-based, absolute row). */
export interface CellPos {
  x: number;
  y: number;
}

/** Same shape as xterm's getSelectionPosition(): 0-based, end exclusive. */
export interface CellRange {
  start: CellPos;
  end: CellPos;
}

/** Arguments for xterm's select(column, row, length). */
export interface Selection {
  column: number;
  row: number;
  length: number;
}

const BACKSPACE = "\x7f";
/** Characters that end a typical prompt when followed by a space. */
const PROMPT_MARKS = "$#>%❯";
/** Upper bound on rows one command may span, so a wall of full-width output stays cheap. */
const MAX_INPUT_ROWS = 64;

interface Char {
  text: string;
  x: number;
  y: number;
  width: number;
}

/** The cursor's line, following soft wraps, as characters with their cells. */
interface InputLine {
  startRow: number;
  endRow: number;
  cols: number;
  chars: Char[];
  cursor: number;
}

function rowChars(buffer: BufferLike, y: number): Char[] {
  const row = buffer.getLine(y);
  const chars: Char[] = [];
  if (!row) return chars;
  for (let x = 0; x < row.length; x++) {
    const cell = row.getCell(x);
    const text = cell?.getChars() ?? "";
    // Empty cells are never-written padding or the second half of a wide character.
    if (cell && text !== "") chars.push({ text, x, y, width: cell.getWidth() });
  }
  return chars;
}

function isFull(buffer: BufferLike, y: number): boolean {
  const row = buffer.getLine(y);
  return !!row && (row.getCell(row.length - 1)?.getChars() ?? "") !== "";
}

/** Index just after the first prompt mark ("$ ", "# ", ...) ending at or before `limit`, or -1. */
function promptEnd(chars: Char[], limit: number): number {
  for (let i = 0; i + 2 <= limit; i++) {
    if (PROMPT_MARKS.includes(chars[i].text) && chars[i + 1].text === " ") return i + 2;
  }
  return -1;
}

function readInputLine(buffer: BufferLike): InputLine | null {
  const cursorRow = buffer.baseY + buffer.cursorY;
  const first = buffer.getLine(cursorRow);
  if (!first) return null;
  const charsFrom = (startRow: number, endRow: number) => {
    const chars: Char[] = [];
    for (let y = startRow; y <= endRow; y++) chars.push(...rowChars(buffer, y));
    return chars;
  };
  const beforeCursor = (ch: Char) => ch.y < cursorRow || ch.x < buffer.cursorX;

  // A row continues the one above when xterm flagged it as soft-wrapped, or when the
  // row above is filled to the last column: readline redraws a wrapped command with
  // cursor moves, which leaves the flag unset. Climbing stops once the prompt shows,
  // so full-width output printed before the prompt is not taken in.
  let startRow = cursorRow;
  while (startRow > 0 && cursorRow - startRow < MAX_INPUT_ROWS) {
    if (buffer.getLine(startRow)?.isWrapped) {
      startRow--;
      continue;
    }
    const sofar = charsFrom(startRow, cursorRow).filter(beforeCursor);
    if (!isFull(buffer, startRow - 1) || promptEnd(sofar, sofar.length) >= 0) break;
    startRow--;
  }
  let endRow = cursorRow;
  while (endRow - cursorRow < MAX_INPUT_ROWS) {
    const next = buffer.getLine(endRow + 1);
    if (!next || !(next.isWrapped || (isFull(buffer, endRow) && rowChars(buffer, endRow + 1).length > 0))) break;
    endRow++;
  }

  const line: InputLine = { startRow, endRow, cols: first.length, chars: charsFrom(startRow, endRow), cursor: 0 };
  line.cursor = indexAt(line, { x: buffer.cursorX, y: cursorRow });
  return line;
}

/** Number of characters before a cell boundary; boundaries past the text clamp to its end. */
function indexAt(line: InputLine, pos: CellPos): number {
  let index = 0;
  for (const ch of line.chars) {
    if (ch.y > pos.y || (ch.y === pos.y && ch.x >= pos.x)) break;
    index++;
  }
  return index;
}

function contains(line: InputLine, row: number): boolean {
  return row >= line.startRow && row <= line.endRow;
}

function arrows(count: number, applicationCursor: boolean): string {
  const prefix = applicationCursor ? "\x1bO" : "\x1b[";
  return count < 0 ? `${prefix}D`.repeat(-count) : `${prefix}C`.repeat(count);
}

/** Keys that move the cursor to a clicked cell, or null when the click is off the command line. */
export function moveCursorSequence(buffer: BufferLike, target: CellPos, applicationCursor: boolean): string | null {
  const line = readInputLine(buffer);
  if (!line || !contains(line, target.y)) return null;
  return arrows(indexAt(line, target) - line.cursor, applicationCursor);
}

/**
 * Keys that erase a selection on the command line, or null when the selection is
 * elsewhere (or only covers blank cells) and the key should behave normally.
 */
export function deleteSelectionSequence(buffer: BufferLike, range: CellRange, applicationCursor: boolean): string | null {
  const line = readInputLine(buffer);
  if (!line || !contains(line, range.start.y) || !contains(line, range.end.y)) return null;
  const start = indexAt(line, range.start);
  const end = indexAt(line, range.end);
  if (end <= start) return null;
  return arrows(end - line.cursor, applicationCursor) + BACKSPACE.repeat(end - start);
}

/**
 * Where the command being typed starts: after the first prompt mark ("$ ", "# ", ...)
 * before the cursor, or at the line start when there is no mark.
 */
function commandStart(line: InputLine): number {
  return Math.max(promptEnd(line.chars, line.cursor), 0);
}

/**
 * Keys that erase the whole command (the Ctrl+A selection). Read from the screen at
 * the time of the key, so characters echoed after the selection was drawn are included.
 */
export function deleteCommandSequence(buffer: BufferLike, applicationCursor: boolean): string | null {
  const line = readInputLine(buffer);
  if (!line) return null;
  const start = commandStart(line);
  const end = line.chars.length;
  if (end <= start) return null;
  return arrows(end - line.cursor, applicationCursor) + BACKSPACE.repeat(end - start);
}

/** The command being typed, for Ctrl+A; null when there is nothing typed. */
export function selectAllRange(buffer: BufferLike): Selection | null {
  const line = readInputLine(buffer);
  if (!line) return null;
  const start = commandStart(line);
  if (start >= line.chars.length) return null;
  const first = line.chars[start];
  const last = line.chars[line.chars.length - 1];
  const length = (last.y - first.y) * line.cols + last.x + last.width - first.x;
  return { column: first.x, row: first.y, length };
}
