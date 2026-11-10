// Which open tabs are shown in which pane of the split view. Pure functions so
// the rules are easy to test; App keeps a PaneState in React state.

export type LayoutKind =
  | "1"
  | "2v"
  | "2h"
  | "3v"
  | "4"
  | "win-2-left-wide"
  | "win-2-right-wide"
  | "win-3-left-tall"
  | "win-3-right-tall"
  | "win-3-center-focus"
  | `${number}x${number}`
  | string;

export interface LayoutZone {
  slotIndex: number;
  label: string;
  colStart: number;
  colSpan: number;
  rowStart: number;
  rowSpan: number;
}

export interface LayoutDefinition {
  kind: LayoutKind;
  label: string;
  slots: number;
  rows: number;
  cols: number;
  areas: string;
  defaultCols: number[];
  defaultRows: number[];
  zones: LayoutZone[];
}

export const SNAP_LAYOUTS: LayoutDefinition[] = [
  // 1. Dua kolom seimbang 50/50
  {
    kind: "2v",
    label: "Dua kolom seimbang",
    slots: 2,
    cols: 2,
    rows: 1,
    areas: '"p0 p1"',
    defaultCols: [50, 50],
    defaultRows: [100],
    zones: [
      { slotIndex: 0, label: "Kiri (50%)", colStart: 1, colSpan: 1, rowStart: 1, rowSpan: 1 },
      { slotIndex: 1, label: "Kanan (50%)", colStart: 2, colSpan: 1, rowStart: 1, rowSpan: 1 },
    ],
  },
  // 2. Kiri penuh, kanan terbagi atas & bawah
  {
    kind: "win-3-left-tall",
    label: "Kiri penuh, kanan atas & bawah",
    slots: 3,
    cols: 2,
    rows: 2,
    areas: '"p0 p1" "p0 p2"',
    defaultCols: [50, 50],
    defaultRows: [50, 50],
    zones: [
      { slotIndex: 0, label: "Kiri Penuh", colStart: 1, colSpan: 1, rowStart: 1, rowSpan: 2 },
      { slotIndex: 1, label: "Kanan Atas", colStart: 2, colSpan: 1, rowStart: 1, rowSpan: 1 },
      { slotIndex: 2, label: "Kanan Bawah", colStart: 2, colSpan: 1, rowStart: 2, rowSpan: 1 },
    ],
  },
  // 3. Empat kuadran 2x2
  {
    kind: "4",
    label: "Empat kuadran",
    slots: 4,
    cols: 2,
    rows: 2,
    areas: '"p0 p1" "p2 p3"',
    defaultCols: [50, 50],
    defaultRows: [50, 50],
    zones: [
      { slotIndex: 0, label: "Kiri Atas", colStart: 1, colSpan: 1, rowStart: 1, rowSpan: 1 },
      { slotIndex: 1, label: "Kanan Atas", colStart: 2, colSpan: 1, rowStart: 1, rowSpan: 1 },
      { slotIndex: 2, label: "Kiri Bawah", colStart: 1, colSpan: 1, rowStart: 2, rowSpan: 1 },
      { slotIndex: 3, label: "Kanan Bawah", colStart: 2, colSpan: 1, rowStart: 2, rowSpan: 1 },
    ],
  },
  // 4. Dua kolom (kanan lebar 67%)
  {
    kind: "win-2-right-wide",
    label: "Dua kolom (kanan lebar)",
    slots: 2,
    cols: 2,
    rows: 1,
    areas: '"p0 p1"',
    defaultCols: [33, 67],
    defaultRows: [100],
    zones: [
      { slotIndex: 0, label: "Kiri Sempit (33%)", colStart: 1, colSpan: 1, rowStart: 1, rowSpan: 1 },
      { slotIndex: 1, label: "Kanan Lebar (67%)", colStart: 2, colSpan: 1, rowStart: 1, rowSpan: 1 },
    ],
  },
  // 5. Dua kolom (kiri lebar 67%)
  {
    kind: "win-2-left-wide",
    label: "Dua kolom (kiri lebar)",
    slots: 2,
    cols: 2,
    rows: 1,
    areas: '"p0 p1"',
    defaultCols: [67, 33],
    defaultRows: [100],
    zones: [
      { slotIndex: 0, label: "Kiri Lebar (67%)", colStart: 1, colSpan: 1, rowStart: 1, rowSpan: 1 },
      { slotIndex: 1, label: "Kanan Sempit (33%)", colStart: 2, colSpan: 1, rowStart: 1, rowSpan: 1 },
    ],
  },
  // 6. Tiga kolom seimbang 33/33/33
  {
    kind: "3v",
    label: "Tiga kolom seimbang",
    slots: 3,
    cols: 3,
    rows: 1,
    areas: '"p0 p1 p2"',
    defaultCols: [33.333, 33.333, 33.334],
    defaultRows: [100],
    zones: [
      { slotIndex: 0, label: "Kolom Kiri", colStart: 1, colSpan: 1, rowStart: 1, rowSpan: 1 },
      { slotIndex: 1, label: "Kolom Tengah", colStart: 2, colSpan: 1, rowStart: 1, rowSpan: 1 },
      { slotIndex: 2, label: "Kolom Kanan", colStart: 3, colSpan: 1, rowStart: 1, rowSpan: 1 },
    ],
  },
  // 7. Kanan penuh, kiri terbagi atas & bawah
  {
    kind: "win-3-right-tall",
    label: "Kanan penuh, kiri atas & bawah",
    slots: 3,
    cols: 2,
    rows: 2,
    areas: '"p0 p2" "p1 p2"',
    defaultCols: [50, 50],
    defaultRows: [50, 50],
    zones: [
      { slotIndex: 0, label: "Kiri Atas", colStart: 1, colSpan: 1, rowStart: 1, rowSpan: 1 },
      { slotIndex: 1, label: "Kiri Bawah", colStart: 1, colSpan: 1, rowStart: 2, rowSpan: 1 },
      { slotIndex: 2, label: "Kanan Penuh", colStart: 2, colSpan: 1, rowStart: 1, rowSpan: 2 },
    ],
  },
  // 8. Tiga kolom fokus tengah 25/50/25
  {
    kind: "win-3-center-focus",
    label: "Tiga kolom fokus tengah",
    slots: 3,
    cols: 3,
    rows: 1,
    areas: '"p0 p1 p2"',
    defaultCols: [25, 50, 25],
    defaultRows: [100],
    zones: [
      { slotIndex: 0, label: "Samping Kiri (25%)", colStart: 1, colSpan: 1, rowStart: 1, rowSpan: 1 },
      { slotIndex: 1, label: "Fokus Tengah (50%)", colStart: 2, colSpan: 1, rowStart: 1, rowSpan: 1 },
      { slotIndex: 2, label: "Samping Kanan (25%)", colStart: 3, colSpan: 1, rowStart: 1, rowSpan: 1 },
    ],
  },
  // 9. Dua baris seimbang 50/50
  {
    kind: "2h",
    label: "Dua baris seimbang",
    slots: 2,
    cols: 1,
    rows: 2,
    areas: '"p0" "p1"',
    defaultCols: [100],
    defaultRows: [50, 50],
    zones: [
      { slotIndex: 0, label: "Baris Atas", colStart: 1, colSpan: 1, rowStart: 1, rowSpan: 1 },
      { slotIndex: 1, label: "Baris Bawah", colStart: 1, colSpan: 1, rowStart: 2, rowSpan: 1 },
    ],
  },
];

export const LAYOUTS: { kind: LayoutKind; label: string; slots: number }[] = [
  { kind: "1", label: "Satu panel", slots: 1 },
  { kind: "2v", label: "Dua kolom", slots: 2 },
  { kind: "3v", label: "Tiga kolom", slots: 3 },
  { kind: "2h", label: "Dua baris", slots: 2 },
  { kind: "4", label: "Empat panel", slots: 4 },
];

export function getLayoutDefinition(layout: LayoutKind): LayoutDefinition {
  const preset = SNAP_LAYOUTS.find((l) => l.kind === layout);
  if (preset) return preset;

  if (layout === "1") {
    return {
      kind: "1",
      label: "Satu panel",
      slots: 1,
      rows: 1,
      cols: 1,
      areas: '"p0"',
      defaultCols: [100],
      defaultRows: [100],
      zones: [{ slotIndex: 0, label: "Layar Penuh", colStart: 1, colSpan: 1, rowStart: 1, rowSpan: 1 }],
    };
  }

  const match = /^(\d+)x(\d+)$/.exec(layout.trim());
  if (match) {
    const rows = Math.max(1, Math.min(10, parseInt(match[1], 10)));
    const cols = Math.max(1, Math.min(10, parseInt(match[2], 10)));
    const slots = rows * cols;
    const zones: LayoutZone[] = [];
    for (let r = 0; r < rows; r++) {
      for (let c = 0; c < cols; c++) {
        const slotIndex = r * cols + c;
        zones.push({
          slotIndex,
          label: `Panel ${slotIndex + 1}`,
          colStart: c + 1,
          colSpan: 1,
          rowStart: r + 1,
          rowSpan: 1,
        });
      }
    }
    return {
      kind: layout,
      label: `${rows}×${cols} panel`,
      slots,
      rows,
      cols,
      areas: gridTemplateAreasFor(rows, cols),
      defaultCols: defaultTrackSizes(cols),
      defaultRows: defaultTrackSizes(rows),
      zones,
    };
  }

  // Fallback ke 1 panel jika tidak dikenal
  return {
    kind: "1",
    label: "Satu panel",
    slots: 1,
    rows: 1,
    cols: 1,
    areas: '"p0"',
    defaultCols: [100],
    defaultRows: [100],
    zones: [{ slotIndex: 0, label: "Layar Penuh", colStart: 1, colSpan: 1, rowStart: 1, rowSpan: 1 }],
  };
}

export function layoutDimensions(layout: LayoutKind): {
  rows: number;
  cols: number;
  slots: number;
} {
  const def = getLayoutDefinition(layout);
  return {
    rows: def.rows,
    cols: def.cols,
    slots: def.slots,
  };
}

export function gridTemplateAreasFor(rows: number, cols: number): string {
  const rowStrings: string[] = [];
  for (let r = 0; r < rows; r++) {
    const rowSlots: string[] = [];
    for (let c = 0; c < cols; c++) {
      rowSlots.push(`p${r * cols + c}`);
    }
    rowStrings.push(`"${rowSlots.join(" ")}"`);
  }
  return rowStrings.join(" ");
}

export function defaultTrackSizes(count: number): number[] {
  if (count <= 0) return [];
  const pct = 100 / count;
  return Array(count).fill(pct);
}

export function resizeTrack(sizes: number[], index: number, deltaPct: number, minPct = 10): number[] {
  if (index < 0 || index >= sizes.length - 1) return sizes;
  const currentLeft = sizes[index];
  const currentRight = sizes[index + 1];
  const totalPair = currentLeft + currentRight;

  let newLeft = currentLeft + deltaPct;
  let newRight = currentRight - deltaPct;

  if (newLeft < minPct) {
    newLeft = minPct;
    newRight = totalPair - minPct;
  } else if (newRight < minPct) {
    newRight = minPct;
    newLeft = totalPair - minPct;
  }

  const result = [...sizes];
  result[index] = newLeft;
  result[index + 1] = newRight;
  return result;
}

export interface PaneState {
  layout: LayoutKind;
  /** Tab id shown in each pane, null for an empty pane. */
  slots: (string | null)[];
  /** Index of the pane that receives new tabs, snippets and shortcuts. */
  focused: number;
}

export function slotCount(layout: LayoutKind): number {
  return layoutDimensions(layout).slots;
}

export function emptyPanes(layout: LayoutKind): PaneState {
  return { layout, slots: Array<string | null>(slotCount(layout)).fill(null), focused: 0 };
}

export function activeTabId(state: PaneState): string | null {
  return state.slots[state.focused] ?? null;
}

export function focusSlot(state: PaneState, index: number): PaneState {
  if (index < 0 || index >= state.slots.length || index === state.focused) return state;
  return { ...state, focused: index };
}

/** Focus the pane already showing the tab, or put the tab into the focused pane. */
export function showTab(state: PaneState, tabId: string): PaneState {
  const existing = state.slots.indexOf(tabId);
  if (existing !== -1) return { ...state, focused: existing };
  const slots = [...state.slots];
  slots[state.focused] = tabId;
  return { ...state, slots };
}

/** Change the split; the focused tab stays visible and new panes get hidden tabs. */
export function setLayout(state: PaneState, layout: LayoutKind, tabIds: string[]): PaneState {
  const count = slotCount(layout);
  const slots = state.slots.slice(0, count);
  while (slots.length < count) slots.push(null);
  let focused = state.focused;
  if (focused >= count) {
    slots[count - 1] = state.slots[focused];
    focused = count - 1;
  }
  const hidden = tabIds.filter((id) => !slots.includes(id));
  for (let i = 0; i < slots.length && hidden.length > 0; i += 1) {
    if (slots[i] === null) slots[i] = hidden.shift() ?? null;
  }
  return { layout, slots, focused };
}

/**
 * Change layout specifically assigning the active tab to targetSlot.
 * Remaining slots are filled with previously visible tabs, then hidden tabs.
 */
export function setLayoutWithSlot(
  state: PaneState,
  layout: LayoutKind,
  targetSlot: number,
  allTabIds: string[],
): PaneState {
  const count = slotCount(layout);
  const clampedTarget = Math.max(0, Math.min(count - 1, targetSlot));
  const activeTab = activeTabId(state);

  const slots: (string | null)[] = Array(count).fill(null);
  slots[clampedTarget] = activeTab;

  // Tabs previously visible (excluding activeTab) to preserve where possible
  const otherPrevVisible = state.slots.filter(
    (id): id is string => id !== null && id !== activeTab,
  );
  // Hidden tabs that were not visible
  const hiddenTabs = allTabIds.filter(
    (id) => id !== activeTab && !otherPrevVisible.includes(id),
  );
  const pool = [...otherPrevVisible, ...hiddenTabs];

  for (let i = 0; i < count; i++) {
    if (i !== clampedTarget && pool.length > 0) {
      slots[i] = pool.shift() ?? null;
    }
  }

  return {
    layout,
    slots,
    focused: clampedTarget,
  };
}

/** Called before a tab closes. `allTabIds` is the tab order including the closing tab. */
export function removeTab(state: PaneState, tabId: string, allTabIds: string[]): PaneState {
  if (!state.slots.includes(tabId)) return state;
  const index = allTabIds.indexOf(tabId);
  const hidden = allTabIds.filter((id) => id !== tabId && !state.slots.includes(id));
  // Prefer the right-hand neighbour in the tab bar, then the left one.
  const replacement =
    hidden.find((id) => allTabIds.indexOf(id) > index) ?? hidden[hidden.length - 1] ?? null;
  return { ...state, slots: state.slots.map((id) => (id === tabId ? replacement : id)) };
}

export const DEFAULT_FONT_SIZE = 14;
export const MIN_FONT_SIZE = 8;
export const MAX_FONT_SIZE = 28;

export function clampFontSize(size: number): number {
  if (!Number.isFinite(size)) return DEFAULT_FONT_SIZE;
  return Math.max(MIN_FONT_SIZE, Math.min(MAX_FONT_SIZE, Math.round(size)));
}

export function stepFontSize(current: number, delta: number): number {
  return clampFontSize(clampFontSize(current) + delta);
}

export interface BookmarkUsageEntry {
  count: number;
  lastUsed: number;
}

export type BookmarkUsageMap = Record<number, BookmarkUsageEntry>;

export interface PresetPath {
  name: string;
  path: string;
  description: string;
}

export const COMMON_SERVER_PATHS: PresetPath[] = [
  { name: "Home (~)", path: "~", description: "Folder utama user" },
  { name: "Web Root (/var/www)", path: "/var/www", description: "Folder aplikasi web" },
  { name: "Public HTML (/var/www/html)", path: "/var/www/html", description: "Document root standar" },
  { name: "Docker & Opt (/opt)", path: "/opt", description: "Aplikasi & container" },
  { name: "Services (/srv)", path: "/srv", description: "Data layanan server" },
  { name: "Konfigurasi (/etc)", path: "/etc", description: "Konfigurasi sistem & aplikasi" },
  { name: "Nginx Config (/etc/nginx)", path: "/etc/nginx", description: "Virtual host & proxy" },
  { name: "System Logs (/var/log)", path: "/var/log", description: "Log sistem & layanan" },
];

export function recordBookmarkUse(
  usage: BookmarkUsageMap,
  bookmarkId: number,
  now: number = Date.now(),
): BookmarkUsageMap {
  const prev = usage[bookmarkId];
  return {
    ...usage,
    [bookmarkId]: {
      count: (prev?.count ?? 0) + 1,
      lastUsed: now,
    },
  };
}

export function rankFrequentBookmarks<T extends { id: number; name: string }>(
  bookmarks: T[],
  usage: BookmarkUsageMap,
): (T & { useCount: number; lastUsed: number })[] {
  return bookmarks
    .map((bm) => ({
      ...bm,
      useCount: usage[bm.id]?.count ?? 0,
      lastUsed: usage[bm.id]?.lastUsed ?? 0,
    }))
    .sort((a, b) => {
      if (b.useCount !== a.useCount) return b.useCount - a.useCount;
      if (b.lastUsed !== a.lastUsed) return b.lastUsed - a.lastUsed;
      return a.name.localeCompare(b.name);
    });
}


