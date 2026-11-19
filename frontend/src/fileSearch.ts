import type { FileReplaceTarget, FileSearchFileResult } from "./types";

export const DEFAULT_SEARCH_EXCLUDES: readonly string[] = [
  ".git",
  "node_modules",
  "dist",
  "build",
  "target",
  ".next",
  ".nuxt",
  "vendor",
  "__pycache__",
  ".venv",
  "venv",
  ".idea",
  ".vscode",
  "coverage",
  ".cache",
  "*.min.js",
  "*.min.css",
  "*.lock",
  "package-lock.json",
  "Cargo.lock",
  "pnpm-lock.yaml",
  "yarn.lock"
];

export function parsePatternList(raw: string): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const part of raw.split(/[,\n]/)) {
    const trimmed = part.trim();
    if (!trimmed || seen.has(trimmed)) continue;
    seen.add(trimmed);
    out.push(trimmed);
  }
  return out;
}

export interface MatchLineParts {
  before: string;
  match: string;
  after: string;
}

export function splitMatchLine(text: string, start: number, end: number): MatchLineParts {
  const safeStart = Math.max(0, Math.min(start, text.length));
  const safeEnd = Math.max(safeStart, Math.min(end, text.length));
  return {
    before: text.slice(0, safeStart),
    match: text.slice(safeStart, safeEnd),
    after: text.slice(safeEnd)
  };
}

export function buildReplaceTargets(
  files: FileSearchFileResult[],
  selectedPaths: Set<string>
): FileReplaceTarget[] {
  const targets: FileReplaceTarget[] = [];
  for (const f of files) {
    if (selectedPaths.has(f.path)) {
      targets.push({ path: f.path, expected_mtime: f.mtime });
    }
  }
  return targets;
}

export function countSelectedMatches(
  files: FileSearchFileResult[],
  selectedPaths: Set<string>
): number {
  let total = 0;
  for (const f of files) {
    if (selectedPaths.has(f.path)) {
      total += f.match_count;
    }
  }
  return total;
}
