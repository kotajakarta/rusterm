import type { BackupStats } from "./types";

export function validateSyncPin(pin: string): string | null {
  const trimmed = pin.trim();
  if (!trimmed) {
    return "PIN enkripsi tidak boleh kosong.";
  }
  if (trimmed.length < 4) {
    return "PIN enkripsi minimal 4 karakter.";
  }
  return null;
}

export function formatSyncStats(stats: BackupStats): string {
  const parts: string[] = [];
  if (stats.servers_added > 0) {
    parts.push(`+${stats.servers_added} server ditambahkan`);
  }
  if (stats.servers_updated > 0) {
    parts.push(`${stats.servers_updated} server diperbarui`);
  }
  if (stats.bookmarks_added > 0) {
    parts.push(`+${stats.bookmarks_added} bookmark ditambahkan`);
  }
  if (stats.snippets_added > 0) {
    parts.push(`+${stats.snippets_added} snippet ditambahkan`);
  }

  if (parts.length === 0) {
    return "Semua data sudah sinkron (tidak ada entri baru).";
  }
  return parts.join(", ");
}

export function formatSyncTime(epochSeconds: number): string {
  if (!epochSeconds || epochSeconds <= 0) {
    return "Belum pernah disinkronkan";
  }
  const nowSec = Math.floor(Date.now() / 1000);
  const diffSec = nowSec - epochSeconds;

  if (diffSec < 30) {
    return "Baru saja";
  }
  if (diffSec < 3600) {
    const mins = Math.max(1, Math.floor(diffSec / 60));
    return `${mins} menit yang lalu`;
  }
  if (diffSec < 86400) {
    const hours = Math.floor(diffSec / 3600);
    return `${hours} jam yang lalu`;
  }
  const date = new Date(epochSeconds * 1000);
  return date.toLocaleDateString("id-ID", {
    day: "numeric",
    month: "short",
    year: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

export function getDefaultDeviceName(): string {
  if (typeof navigator !== "undefined" && /Android/i.test(navigator.userAgent)) {
    return "Android Device";
  }
  return "Linux Desktop";
}
