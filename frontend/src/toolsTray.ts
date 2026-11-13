export type ToolsTab = "snippets" | "cloudflare" | "tailscale";

export const TOOLS_TABS: readonly ToolsTab[] = ["snippets", "cloudflare", "tailscale"];
export const TOOLS_TAB_KEY = "rusterm.toolsTab";

const EDGE_GAP = 8;
const ANCHOR_GAP = 6;
const MAX_WIDTH = 380;
const MIN_HEIGHT = 160;

/** Restore the last tools tab; anything unknown falls back to Snippets. */
export function parseToolsTab(raw: string | null): ToolsTab {
  return TOOLS_TABS.find((t) => t === raw) ?? "snippets";
}

export interface TrayPosition {
  left: number;
  bottom: number;
  width: number;
  maxHeight: number;
}

/** Fixed-position box for the tools popover, opening upward from its toggle button. */
export function computeTrayPosition(
  anchor: { top: number },
  sidebarLeft: number,
  viewport: { width: number; height: number },
): TrayPosition {
  const width = Math.min(MAX_WIDTH, viewport.width - EDGE_GAP * 2);
  const maxHeight = Math.max(MIN_HEIGHT, Math.min(viewport.height * 0.7, anchor.top - EDGE_GAP * 2));
  return {
    left: Math.round(sidebarLeft + EDGE_GAP),
    bottom: Math.round(viewport.height - anchor.top + ANCHOR_GAP),
    width: Math.round(width),
    maxHeight: Math.round(maxHeight),
  };
}
