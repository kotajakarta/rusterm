import type { Bookmark } from "./types";

/** Same normalisation the backend applies when a folder is bookmarked (drop trailing `/`). */
export function normalizeBookmarkPath(path: string): string {
  return path.length > 1 && path.endsWith("/") ? path.replace(/\/+$/, "") || "/" : path;
}

/** File-manager bookmarks are the sidebar paths, grouped per server id. */
export function fileBookmarkMap(bookmarks: Bookmark[]): Record<string, string[]> {
  const map: Record<string, string[]> = {};
  for (const bm of bookmarks) {
    const key = String(bm.server_id);
    const path = normalizeBookmarkPath(bm.path);
    const list = map[key] ?? (map[key] = []);
    if (!list.includes(path)) list.push(path);
  }
  for (const key of Object.keys(map)) map[key].sort();
  return map;
}

/** The sidebar path saved for `path` on `serverId`, if any. */
export function findPathBookmark(bookmarks: Bookmark[], serverId: number, path: string): Bookmark | undefined {
  const target = normalizeBookmarkPath(path);
  return bookmarks.find((b) => b.server_id === serverId && normalizeBookmarkPath(b.path) === target);
}

/** Short label for a folder: its last segment, or `Root (/)`. */
export function folderTitle(path: string): string {
  if (path === "/") return "Root (/)";
  return path.split("/").filter(Boolean).pop() || path;
}
