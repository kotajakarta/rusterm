export const DEFAULT_SIDEBAR_WIDTH = 420;
export const MIN_SIDEBAR_WIDTH = 240;
export const MAX_SIDEBAR_WIDTH = 600;
export const MINI_COLLAPSE_THRESHOLD = 160;
export const SIDEBAR_WIDTH_KEY = "rusterm.sidebarWidth";

export type SidebarDragResult =
  | { mini: true }
  | { mini: false; width: number };

/** Clamp sidebar width within desktop bounds and optional 50% viewport cap. */
export function clampSidebarWidth(width: number, viewportWidth?: number): number {
  if (!Number.isFinite(width)) return DEFAULT_SIDEBAR_WIDTH;
  const viewportCap =
    viewportWidth && Number.isFinite(viewportWidth) && viewportWidth > 0
      ? Math.max(MIN_SIDEBAR_WIDTH, Math.min(MAX_SIDEBAR_WIDTH, Math.floor(viewportWidth * 0.5)))
      : MAX_SIDEBAR_WIDTH;
  return Math.max(MIN_SIDEBAR_WIDTH, Math.min(viewportCap, Math.round(width)));
}

/** Resolve a pointer drag width into either auto-mini collapse or a clamped width. */
export function resolveSidebarDrag(rawWidth: number, viewportWidth?: number): SidebarDragResult {
  if (Number.isFinite(rawWidth) && rawWidth < MINI_COLLAPSE_THRESHOLD) {
    return { mini: true };
  }
  return { mini: false, width: clampSidebarWidth(rawWidth, viewportWidth) };
}
