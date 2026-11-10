export const SERVER_COLORS: { value: string; label: string }[] = [
  { value: "", label: "Tanpa warna" },
  { value: "#e2574c", label: "Merah (production)" },
  { value: "#e8a33d", label: "Amber (staging)" },
  { value: "#7cbf6b", label: "Hijau (development)" },
  { value: "#4fb3c8", label: "Biru muda" },
  { value: "#8b8fe8", label: "Ungu" },
  { value: "#d77fb7", label: "Merah muda" },
];

export function serverColor(color: string): string {
  return color || "var(--line-strong)";
}

export function formatBytes(bytes: number | null): string {
  if (bytes === null) return "–";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value.toFixed(value >= 10 || unit === 0 ? 0 : 1)} ${units[unit]}`;
}

export function formatUptime(seconds: number | null): string {
  if (seconds === null) return "–";
  const days = Math.floor(seconds / 86400);
  const hours = Math.floor((seconds % 86400) / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  if (days > 0) return `${days} hari ${hours} jam`;
  if (hours > 0) return `${hours} jam ${minutes} menit`;
  return `${minutes} menit`;
}

export function ratio(used: number | null, total: number | null): number | null {
  if (used === null || total === null || total <= 0) return null;
  return Math.min(1, Math.max(0, used / total));
}
