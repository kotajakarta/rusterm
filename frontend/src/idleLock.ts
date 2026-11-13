export const IDLE_LOCK_KEY = "rusterm.idleLockMinutes";
export const IDLE_LOCK_OPTIONS = [0, 5, 15, 30, 60] as const;
export type IdleLockMinutes = (typeof IDLE_LOCK_OPTIONS)[number];
export const DEFAULT_IDLE_LOCK_MINUTES: IdleLockMinutes = 30;

/** Normalize a stored or user-supplied value into a valid IdleLockMinutes option. */
export function normalizeIdleLockMinutes(value: unknown): IdleLockMinutes {
  if (value === null || value === undefined || value === "") {
    return DEFAULT_IDLE_LOCK_MINUTES;
  }
  const num = Number(value);
  if (!Number.isFinite(num)) return DEFAULT_IDLE_LOCK_MINUTES;
  for (const opt of IDLE_LOCK_OPTIONS) {
    if (opt === num) return opt;
  }
  return DEFAULT_IDLE_LOCK_MINUTES;
}

/** Return true when idle timeout is enabled (> 0) and elapsed time meets or exceeds threshold. */
export function isIdleExpired(lastActiveMs: number, nowMs: number, timeoutMinutes: number): boolean {
  if (!Number.isFinite(timeoutMinutes) || timeoutMinutes <= 0) return false;
  if (!Number.isFinite(lastActiveMs) || !Number.isFinite(nowMs) || nowMs < lastActiveMs) return false;
  return nowMs - lastActiveMs >= timeoutMinutes * 60_000;
}

/** Format human-readable Indonesian label for an idle lock option. */
export function formatIdleLockLabel(minutes: IdleLockMinutes): string {
  if (minutes === 0) return "Nonaktif";
  return `${minutes} menit`;
}
