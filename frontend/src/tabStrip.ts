export interface TabScrollState {
  hasOverflow: boolean;
  canScrollLeft: boolean;
  canScrollRight: boolean;
}

export function computeTabScrollState(
  scrollLeft: number,
  clientWidth: number,
  scrollWidth: number,
): TabScrollState {
  const hasOverflow = scrollWidth - clientWidth > 2;
  if (!hasOverflow) {
    return {
      hasOverflow: false,
      canScrollLeft: false,
      canScrollRight: false,
    };
  }
  return {
    hasOverflow: true,
    canScrollLeft: scrollLeft > 2,
    canScrollRight: scrollLeft + clientWidth < scrollWidth - 2,
  };
}

export interface SearchableTab {
  id: string;
  title: string;
  serverName?: string;
}

export function filterOpenTabs<T extends SearchableTab>(tabs: T[], query: string): T[] {
  const q = query.trim().toLowerCase();
  if (!q) return tabs;
  return tabs.filter((t) => {
    const titleMatch = t.title.toLowerCase().includes(q);
    const serverMatch = t.serverName ? t.serverName.toLowerCase().includes(q) : false;
    return titleMatch || serverMatch;
  });
}
