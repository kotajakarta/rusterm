import { useCallback, useEffect, useId, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { readString, writeString } from "../storage";
import { TOOLS_TABS, TOOLS_TAB_KEY, computeTrayPosition, parseToolsTab, type ToolsTab, type TrayPosition } from "../toolsTray";
import { Icon } from "./Icon";

const TAB_LABELS: Record<ToolsTab, string> = {
  snippets: "Snippets",
  cloudflare: "Cloudflare",
  tailscale: "Tailscale",
};

interface ToolsTrayProps {
  snippets: ReactNode;
  cloudflare: ReactNode;
  tailscale: ReactNode;
  mini: boolean;
}

/**
 * Sidebar "Tools" button + upward popover holding Snippets, Cloudflare and Tailscale.
 * All three panels stay mounted (inactive ones are `hidden`) so their data and selections
 * survive closing the popover or switching tabs. The popover is portalled to <body> so the
 * panels' own dialogs are not clipped by the sidebar's containment/overflow.
 */
export function ToolsTray({ snippets, cloudflare, tailscale, mini }: ToolsTrayProps): JSX.Element {
  const [open, setOpen] = useState(false);
  const [tab, setTab] = useState<ToolsTab>(() => parseToolsTab(readString(TOOLS_TAB_KEY)));
  const [pos, setPos] = useState<TrayPosition | null>(null);
  const buttonRef = useRef<HTMLButtonElement>(null);
  const trayRef = useRef<HTMLDivElement>(null);
  const baseId = useId();
  const trayId = `${baseId}-tray`;

  const reposition = useCallback(() => {
    const btn = buttonRef.current;
    if (!btn) return;
    const rect = btn.getBoundingClientRect();
    const sidebarLeft = btn.closest(".sidebar")?.getBoundingClientRect().left ?? 0;
    setPos(computeTrayPosition({ top: rect.top }, sidebarLeft, { width: window.innerWidth, height: window.innerHeight }));
  }, []);

  useLayoutEffect(() => {
    if (open) reposition();
  }, [open, mini, reposition]);

  useEffect(() => {
    if (!open) return;
    const onPointerDown = (e: PointerEvent) => {
      const target = e.target as Node;
      // Panel dialogs render inside the tray subtree, so clicks there keep the tray open.
      if (trayRef.current?.contains(target) || buttonRef.current?.contains(target)) return;
      setOpen(false);
    };
    // Bubble phase: an open Modal handles Escape in capture phase and stops propagation first.
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      setOpen(false);
      buttonRef.current?.focus();
    };
    window.addEventListener("pointerdown", onPointerDown);
    window.addEventListener("keydown", onKeyDown);
    window.addEventListener("resize", reposition);
    return () => {
      window.removeEventListener("pointerdown", onPointerDown);
      window.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("resize", reposition);
    };
  }, [open, reposition]);

  const selectTab = (next: ToolsTab) => {
    setTab(next);
    writeString(TOOLS_TAB_KEY, next);
  };

  const panels: Record<ToolsTab, ReactNode> = { snippets, cloudflare, tailscale };

  return (
    <>
      <button
        ref={buttonRef}
        type="button"
        className={`sidebar-tools-btn${open ? " is-open" : ""}`}
        onClick={() => setOpen((v) => !v)}
        aria-expanded={open}
        aria-controls={trayId}
        aria-haspopup="dialog"
        title={open ? "Tutup Tools" : "Tools: Snippets, Cloudflare, Tailscale"}
        aria-label="Tools: Snippets, Cloudflare, Tailscale"
      >
        <Icon name="up" size={14} />
        {!mini && <span className="btn-label">Tools</span>}
      </button>
      {createPortal(
        <div
          ref={trayRef}
          id={trayId}
          className="tools-tray"
          role="dialog"
          aria-label="Tools"
          hidden={!open}
          style={pos ? { left: pos.left, bottom: pos.bottom, width: pos.width, maxHeight: pos.maxHeight } : undefined}
        >
          <div className="tools-tray-tabs" role="tablist" aria-label="Pilih tools">
            {TOOLS_TABS.map((t) => (
              <button
                key={t}
                type="button"
                role="tab"
                id={`${baseId}-tab-${t}`}
                aria-selected={tab === t}
                aria-controls={`${baseId}-panel-${t}`}
                className={`tools-tray-tab${tab === t ? " is-active" : ""}`}
                onClick={() => selectTab(t)}
              >
                {TAB_LABELS[t]}
              </button>
            ))}
          </div>
          {TOOLS_TABS.map((t) => (
            <div
              key={t}
              role="tabpanel"
              id={`${baseId}-panel-${t}`}
              aria-labelledby={`${baseId}-tab-${t}`}
              className="tools-tray-body"
              hidden={tab !== t}
            >
              {panels[t]}
            </div>
          ))}
        </div>,
        document.body,
      )}
    </>
  );
}
