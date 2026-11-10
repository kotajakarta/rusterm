import { useState } from "react";

/** Boolean UI preference remembered in this browser (falls back to memory if storage is blocked). */
export function usePersistentFlag(key: string, initial: boolean): [boolean, (value: boolean) => void] {
  const [value, setValue] = useState<boolean>(() => {
    try {
      const raw = localStorage.getItem(key);
      return raw === null ? initial : raw === "1";
    } catch {
      return initial;
    }
  });
  const update = (next: boolean) => {
    setValue(next);
    try {
      localStorage.setItem(key, next ? "1" : "0");
    } catch {
      // Not persisted; the in-memory value still applies.
    }
  };
  return [value, update];
}

export function readString(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

export function writeString(key: string, value: string): void {
  try {
    localStorage.setItem(key, value);
  } catch {
    // Preference simply isn't remembered.
  }
}
