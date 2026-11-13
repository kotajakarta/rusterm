export interface BufferSearchMatch {
  row: number;
  col: number;
  length: number;
}

/** Find all case-insensitive substring occurrences across terminal buffer lines. */
export function findBufferMatches(lines: readonly string[], query: string): BufferSearchMatch[] {
  if (!query) return [];
  const needle = query.toLowerCase();
  const qLen = needle.length;
  const results: BufferSearchMatch[] = [];

  for (let row = 0; row < lines.length; row++) {
    const hay = lines[row].toLowerCase();
    let from = 0;
    while (from <= hay.length - qLen) {
      const idx = hay.indexOf(needle, from);
      if (idx === -1) break;
      results.push({ row, col: idx, length: qLen });
      from = idx + Math.max(1, qLen);
    }
  }
  return results;
}

/** Step forward (+1) or backward (-1) through match indices with wrap-around. */
export function stepMatchIndex(current: number, total: number, delta: 1 | -1): number {
  if (total <= 0) return -1;
  if (current < 0 || current >= total) {
    return delta > 0 ? 0 : total - 1;
  }
  return (current + delta + total) % total;
}
