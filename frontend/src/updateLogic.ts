import type { UpdateCheckResult, UpdateProgressResult } from "./types";

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

/** Return true when the header should show the release update indicator. */
export function shouldShowHeaderUpdate(check: UpdateCheckResult | null): boolean {
  return Boolean(check?.has_update);
}

export function getUpdateStatusText(progress: UpdateProgressResult): string {
  switch (progress.state) {
    case "downloading": {
      if (progress.total_bytes > 0) {
        return `Mengunduh (${formatUpdateBytes(progress.downloaded_bytes)} / ${formatUpdateBytes(progress.total_bytes)} - ${progress.progress_percent}%)`;
      }
      return `Mengunduh (${formatUpdateBytes(progress.downloaded_bytes)})`;
    }
    case "applying":
      return progress.message || "Memasang pembaruan dan menutup proses lama...";
    case "completed":
      return progress.message || "Pembaruan selesai! Memulai ulang aplikasi...";
    case "error":
      return `Gagal: ${progress.message || "Terjadi kesalahan saat pembaruan"}`;
    default:
      return progress.message || "";
  }
}
