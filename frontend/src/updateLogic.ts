import type { UpdateCheckResult } from "./types";

export function formatUpdateBytes(bytes: number): string {
  if (bytes <= 0) return "0 B";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function getUpdateBadge(check: UpdateCheckResult): { label: string; type: "warning" | "success" | "info" } {
  if (check.has_update) {
    return { label: "Pembaruan Tersedia", type: "warning" };
  }
  return { label: "Versi Terbaru", type: "success" };
}
