import type { DirEntry } from "./types";

export interface BreadcrumbSegment {
  label: string;
  path: string;
}

export interface PermissionBits {
  owner: [boolean, boolean, boolean];
  group: [boolean, boolean, boolean];
  other: [boolean, boolean, boolean];
}

export type SortField = "name" | "size" | "mtime";

export interface FilterSortOptions {
  query: string;
  showHidden: boolean;
  sortBy: SortField;
  sortAsc: boolean;
}

export const QUICK_TERMINAL_COMMANDS: readonly { label: string; command: string }[] = [
  { label: "ls -la", command: "ls -la\r" },
  { label: "git status", command: "git status\r" },
  { label: "docker ps", command: "docker ps\r" },
  { label: "systemctl", command: "systemctl list-units --type=service --state=running\r" },
  { label: "df -h", command: "df -h\r" },
  { label: "free -m", command: "free -m\r" },
  { label: "ps aux", command: "ps aux --sort=-%mem | head -n 15\r" },
  { label: "clear", command: "clear\r" },
];

export function splitBreadcrumbs(rawPath: string): BreadcrumbSegment[] {
  const trimmed = rawPath.trim() || "/";
  if (trimmed === "/") {
    return [{ label: "/", path: "/" }];
  }
  if (trimmed === "~") {
    return [{ label: "~", path: "~" }];
  }
  if (trimmed.startsWith("~/")) {
    const parts = trimmed.slice(2).split("/").filter(Boolean);
    const segments: BreadcrumbSegment[] = [{ label: "~", path: "~" }];
    let current = "~";
    for (const part of parts) {
      current = `${current}/${part}`;
      segments.push({ label: part, path: current });
    }
    return segments;
  }
  const parts = trimmed.split("/").filter(Boolean);
  const segments: BreadcrumbSegment[] = [{ label: "/", path: "/" }];
  let current = "";
  for (const part of parts) {
    current = `${current}/${part}`;
    segments.push({ label: part, path: current });
  }
  return segments;
}

export function parentDirPath(rawPath: string): string {
  const trimmed = rawPath.trim().replace(/\/+$/, "") || "/";
  if (trimmed === "/" || trimmed === "~") return trimmed;
  const idx = trimmed.lastIndexOf("/");
  if (idx <= 0) return "/";
  return trimmed.slice(0, idx);
}

export function joinRemotePath(dir: string, name: string): string {
  const cleanName = name.replace(/^\/+/, "");
  const cleanDir = dir.trim().replace(/\/+$/, "");
  if (!cleanDir || cleanDir === "/") return `/${cleanName}`;
  return `${cleanDir}/${cleanName}`;
}

function normalizeOctal3(mode: string): string {
  const digits = mode.replace(/[^0-7]/g, "");
  if (!digits) return "644";
  return digits.slice(-3).padStart(3, "0");
}

function digitToTriplet(digit: number): [boolean, boolean, boolean] {
  return [(digit & 4) !== 0, (digit & 2) !== 0, (digit & 1) !== 0];
}

function tripletToDigit(triplet: [boolean, boolean, boolean]): number {
  return (triplet[0] ? 4 : 0) + (triplet[1] ? 2 : 0) + (triplet[2] ? 1 : 0);
}

function tripletToRwx(triplet: [boolean, boolean, boolean]): string {
  return `${triplet[0] ? "r" : "-"}${triplet[1] ? "w" : "-"}${triplet[2] ? "x" : "-"}`;
}

export function parseModeBits(mode: string): PermissionBits {
  const norm = normalizeOctal3(mode);
  const o = parseInt(norm[0] ?? "6", 8) || 0;
  const g = parseInt(norm[1] ?? "4", 8) || 0;
  const x = parseInt(norm[2] ?? "4", 8) || 0;
  return {
    owner: digitToTriplet(o),
    group: digitToTriplet(g),
    other: digitToTriplet(x),
  };
}

export function bitsToMode(bits: PermissionBits): string {
  return `0${tripletToDigit(bits.owner)}${tripletToDigit(bits.group)}${tripletToDigit(bits.other)}`;
}

export function modeToRwx(mode: string, isDir: boolean): string {
  if (!mode) return isDir ? "drwxr-xr-x" : "-rw-r--r--";
  const bits = parseModeBits(mode);
  return `${isDir ? "d" : "-"}${tripletToRwx(bits.owner)}${tripletToRwx(bits.group)}${tripletToRwx(bits.other)}`;
}

export function filterAndSortEntries(entries: DirEntry[], opts: FilterSortOptions): DirEntry[] {
  const q = opts.query.trim().toLowerCase();
  const filtered = entries.filter((entry) => {
    if (!opts.showHidden && entry.name.startsWith(".")) return false;
    if (q && !entry.name.toLowerCase().includes(q)) return false;
    return true;
  });

  const dirFactor = opts.sortAsc ? 1 : -1;
  return [...filtered].sort((a, b) => {
    if (a.is_dir !== b.is_dir) return a.is_dir ? -1 : 1;
    if (opts.sortBy === "size") {
      const diff = (a.size ?? 0) - (b.size ?? 0);
      if (diff !== 0) return diff * dirFactor;
      return a.name.toLowerCase().localeCompare(b.name.toLowerCase());
    }
    if (opts.sortBy === "mtime") {
      const diff = (a.mtime ?? 0) - (b.mtime ?? 0);
      if (diff !== 0) return diff * dirFactor;
      return a.name.toLowerCase().localeCompare(b.name.toLowerCase());
    }
    return a.name.toLowerCase().localeCompare(b.name.toLowerCase()) * dirFactor;
  });
}

export function formatFileTime(mtime: number | null | undefined): string {
  if (!mtime) return "—";
  const d = new Date(mtime * 1000);
  if (Number.isNaN(d.getTime())) return "—";
  return d.toLocaleString("id-ID", {
    day: "2-digit",
    month: "short",
    year: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}
