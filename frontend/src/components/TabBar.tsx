import { useEffect, useRef, useState } from "react";
import { serverColor } from "../format";
import type { Server, Tab } from "../types";
import {
  DEFAULT_FONT_SIZE,
  MAX_FONT_SIZE,
  MIN_FONT_SIZE,
  layoutDimensions,
  type LayoutKind,
} from "../layout";
import { Icon, type IconName } from "./Icon";
import { SnapLayoutPicker } from "./SnapLayoutPicker";

interface TabBarProps {
  tabs: Tab[];
  activeId: string | null;
  /** Tabs currently shown in one of the split panes. */
  visibleIds: Set<string>;
  layout: LayoutKind;
  onLayout: (layout: LayoutKind, targetSlot?: number) => void;
  fontSize: number;
  onStepFontSize: (delta: number) => void;
  onResetFontSize: () => void;
  broadcastMode?: boolean;
  onToggleBroadcast?: () => void;
  workspaceMode?: "terminal" | "files" | "containers";
  onChangeWorkspaceMode?: (mode: "terminal" | "files" | "containers") => void;
  onOpenFiles?: () => void;
  onOpenContainers?: () => void;
  servers: Map<number, Server>;
  onSelect: (id: string) => void;
  onClose: (id: string) => void;
  onOpenPalette: () => void;
  sidebarOpen?: boolean;
  onToggleSidebar?: () => void;
}

const STATUS_LABEL: Record<Tab["status"], string> = {
  connecting: "Menghubungkan",
  connected: "Terhubung",
  closed: "Terputus",
  error: "Gagal",
};

const MAIN_LAYOUTS: { kind: LayoutKind; label: string; icon: IconName }[] = [
  { kind: "1", label: "Full screen (Satu panel)", icon: "layout1" },
  { kind: "2h", label: "Split atas & bawah (Dua baris)", icon: "layout2h" },
];

export function TabBar({
  tabs,
  activeId,
  visibleIds,
  layout,
  onLayout,
  fontSize,
  onStepFontSize,
  onResetFontSize,
  broadcastMode = false,
  onToggleBroadcast,
  workspaceMode = "terminal",
  onChangeWorkspaceMode,
  onOpenFiles,
  onOpenContainers,
  servers,
  onSelect,
  onClose,
  onOpenPalette,
  sidebarOpen,
  onToggleSidebar,
}: TabBarProps) {
  const [showSnapPicker, setShowSnapPicker] = useState(false);
  const [showPicker, setShowPicker] = useState(false);
  const pickerRef = useRef<HTMLDivElement>(null);
  const tabsRef = useRef<HTMLDivElement>(null);
  const currentDims = layoutDimensions(layout);
  const [hoverDims, setHoverDims] = useState<{ rows: number; cols: number } | null>(null);
  const [manualRows, setManualRows] = useState(currentDims.rows);
  const [manualCols, setManualCols] = useState(currentDims.cols);

  useEffect(() => {
    setManualRows(currentDims.rows);
    setManualCols(currentDims.cols);
  }, [currentDims.rows, currentDims.cols]);

  // Keep the active tab in view when the strip scrolls (many tabs on a narrow screen).
  useEffect(() => {
    tabsRef.current
      ?.querySelector<HTMLElement>(".tab.is-active")
      ?.scrollIntoView({ block: "nearest", inline: "nearest" });
  }, [activeId, tabs.length]);

  useEffect(() => {
    if (!showPicker && !showSnapPicker) return;
    const onPointerDown = (e: PointerEvent) => {
      if (pickerRef.current && !pickerRef.current.contains(e.target as Node)) {
        setShowPicker(false);
        setShowSnapPicker(false);
      }
    };
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        setShowPicker(false);
        setShowSnapPicker(false);
      }
    };
    window.addEventListener("pointerdown", onPointerDown);
    window.addEventListener("keydown", onKeyDown);
    return () => {
      window.removeEventListener("pointerdown", onPointerDown);
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [showPicker, showSnapPicker]);

  const displayRows = hoverDims ? hoverDims.rows : currentDims.rows;
  const displayCols = hoverDims ? hoverDims.cols : currentDims.cols;

  return (
    <div className="tabbar">
      {onToggleSidebar && (
        <button
          type="button"
          className={`icon-btn sidebar-toggle-btn${sidebarOpen ? " is-on" : ""}`}
          onClick={onToggleSidebar}
          aria-expanded={sidebarOpen}
          aria-label="Buka atau tutup daftar server"
          title="Daftar server & path"
        >
          <Icon name="menu" size={16} />
        </button>
      )}
      {servers.size > 0 && onChangeWorkspaceMode && (
        <div className="workspace-mode-switch" role="tablist" aria-label="Mode Workspace">
          <button
            type="button"
            role="tab"
            aria-selected={workspaceMode === "terminal"}
            className={`workspace-mode-btn${workspaceMode === "terminal" ? " is-active" : ""}`}
            onClick={() => onChangeWorkspaceMode("terminal")}
            title="Mode Terminal"
          >
            <Icon name="terminal" size={13} />
            <span>Terminal</span>
          </button>
          {onOpenFiles && (
            <button
              type="button"
              role="tab"
              aria-selected={workspaceMode === "files"}
              className={`workspace-mode-btn${workspaceMode === "files" ? " is-active" : ""}`}
              onClick={onOpenFiles}
              title="Buka File Manager (Cockpit) untuk server terminal aktif"
            >
              <Icon name="folder" size={13} />
              <span>File</span>
            </button>
          )}
          {onOpenContainers && (
            <button
              type="button"
              role="tab"
              aria-selected={workspaceMode === "containers"}
              className={`workspace-mode-btn${workspaceMode === "containers" ? " is-active" : ""}`}
              onClick={onOpenContainers}
              title="Buka modul Containers (Podman / Docker) untuk server terminal aktif"
            >
              <Icon name="box" size={13} />
              <span>Containers</span>
            </button>
          )}
        </div>
      )}
      <div className="tabs" role="tablist" aria-label="Terminal terbuka" ref={tabsRef}>
        {tabs.map((tab) => {
          const server = servers.get(tab.serverId);
          return (
            <div
              key={tab.id}
              role="tab"
              tabIndex={0}
              aria-selected={tab.id === activeId}
              className={`tab${tab.id === activeId ? " is-active" : ""}${
                visibleIds.has(tab.id) ? " is-visible" : ""
              } status-${tab.status}`}
              style={{ ["--srv" as string]: serverColor(server?.color ?? "") }}
              onClick={() => onSelect(tab.id)}
              onKeyDown={(e) => (e.key === "Enter" || e.key === " ") && onSelect(tab.id)}
              onAuxClick={(e) => e.button === 1 && onClose(tab.id)}
              title={`${tab.title} (${STATUS_LABEL[tab.status]})`}
            >
              <span className="tab-status" aria-label={STATUS_LABEL[tab.status]} />
              <span className="tab-title">{tab.title}</span>
              <button
                type="button"
                className="tab-close"
                onClick={(e) => {
                  e.stopPropagation();
                  onClose(tab.id);
                }}
                aria-label={`Tutup ${tab.title}`}
              >
                <Icon name="close" size={12} />
              </button>
            </div>
          );
        })}
      </div>
      <div className="font-switch" role="group" aria-label="Ukuran huruf terminal">
        <button
          type="button"
          className="font-step-btn"
          disabled={fontSize <= MIN_FONT_SIZE}
          onClick={() => onStepFontSize(-1)}
          title="Kecilkan ukuran huruf terminal (Alt+-)"
          aria-label="Kecilkan ukuran huruf terminal"
        >
          A−
        </button>
        <button
          type="button"
          className={`font-size-badge${fontSize !== DEFAULT_FONT_SIZE ? " is-changed" : ""}`}
          onClick={onResetFontSize}
          title={`Ukuran huruf ${fontSize}px — klik untuk reset ke ${DEFAULT_FONT_SIZE}px (Alt+0)`}
          aria-label={`Ukuran huruf ${fontSize} piksel, klik untuk reset`}
        >
          {fontSize}px
        </button>
        <button
          type="button"
          className="font-step-btn"
          disabled={fontSize >= MAX_FONT_SIZE}
          onClick={() => onStepFontSize(1)}
          title="Besarkan ukuran huruf terminal (Alt+=)"
          aria-label="Besarkan ukuran huruf terminal"
        >
          A+
        </button>
      </div>
      {visibleIds.size > 1 && onToggleBroadcast && (
        <button
          type="button"
          className={`icon-btn broadcast-toggle-btn${broadcastMode ? " is-on" : ""}`}
          onClick={onToggleBroadcast}
          aria-pressed={broadcastMode}
          title={
            broadcastMode
              ? "Mode Broadcast AKTIF: ketikan dikirim ke semua panel split"
              : "Aktifkan Mode Broadcast (ketik ke semua panel split sekaligus)"
          }
          aria-label="Mode Broadcast ke semua panel"
        >
          <Icon name="broadcast" size={15} />
        </button>
      )}
      <div className="layout-switch" role="radiogroup" aria-label="Tata letak terminal" ref={pickerRef}>
        {MAIN_LAYOUTS.map((item) => (
          <button
            key={item.kind}
            type="button"
            role="radio"
            aria-checked={layout === item.kind}
            className={`icon-btn${layout === item.kind ? " is-on" : ""}`}
            data-layout={item.kind}
            onClick={() => {
              setShowPicker(false);
              setShowSnapPicker(false);
              onLayout(item.kind);
            }}
            title={item.label}
            aria-label={item.label}
          >
            <Icon name={item.icon} size={15} />
          </button>
        ))}

        <button
          type="button"
          role="radio"
          aria-checked={(layout !== "1" && layout !== "2h") || showSnapPicker}
          className={`icon-btn snap-toggle-btn${(layout !== "1" && layout !== "2h") || showSnapPicker ? " is-on" : ""}`}
          onClick={() => {
            setShowSnapPicker((v) => !v);
            setShowPicker(false);
          }}
          title="Snap Layouts & Split lainnya (Alt+S)"
          aria-label="Snap Layouts & Split lainnya"
        >
          <Icon name="layoutSnap" size={15} />
        </button>

        {showSnapPicker && (
          <SnapLayoutPicker
            currentLayout={layout}
            onSelectSnap={(kind, slotIndex) => {
              setShowSnapPicker(false);
              onLayout(kind, slotIndex);
            }}
            onClose={() => setShowSnapPicker(false)}
            onOpenCustomGrid={() => {
              setShowSnapPicker(false);
              setShowPicker(true);
            }}
          />
        )}

        {showPicker && (
          <div className="grid-picker-popover" role="dialog" aria-label="Kustom tata letak terminal">
            <div className="grid-picker-header">
              <span className="grid-picker-title">Kustom Tata Letak</span>
              <span className="grid-picker-badge">
                {displayRows} × {displayCols} ({displayRows * displayCols} panel)
              </span>
            </div>
            <div
              className="grid-matrix"
              role="grid"
              aria-label="Pilih dimensi kotak"
              onMouseLeave={() => setHoverDims(null)}
            >
              {Array.from({ length: 4 }).map((_, r) => (
                <div key={r} className="grid-matrix-row" role="row">
                  {Array.from({ length: 4 }).map((_, c) => {
                    const rowNum = r + 1;
                    const colNum = c + 1;
                    const isHighlighted = rowNum <= displayRows && colNum <= displayCols;
                    return (
                      <button
                        key={c}
                        type="button"
                        role="gridcell"
                        className={`grid-matrix-cell${isHighlighted ? " is-active" : ""}`}
                        onMouseEnter={() => setHoverDims({ rows: rowNum, cols: colNum })}
                        onClick={() => {
                          onLayout(`${rowNum}x${colNum}`);
                          setShowPicker(false);
                        }}
                        title={`${rowNum} × ${colNum}`}
                        aria-label={`${rowNum} baris, ${colNum} kolom`}
                      />
                    );
                  })}
                </div>
              ))}
            </div>
            <div className="grid-picker-manual">
              <div className="grid-input-group">
                <label className="grid-field">
                  <span>Baris</span>
                  <input
                    type="number"
                    min={1}
                    max={4}
                    value={manualRows}
                    onChange={(e) => setManualRows(Math.max(1, Math.min(4, parseInt(e.target.value, 10) || 1)))}
                  />
                </label>
                <span className="grid-multiply">×</span>
                <label className="grid-field">
                  <span>Kolom</span>
                  <input
                    type="number"
                    min={1}
                    max={6}
                    value={manualCols}
                    onChange={(e) => setManualCols(Math.max(1, Math.min(6, parseInt(e.target.value, 10) || 1)))}
                  />
                </label>
              </div>
              <button
                type="button"
                className="grid-apply-btn"
                onClick={() => {
                  onLayout(`${manualRows}x${manualCols}`);
                  setShowPicker(false);
                }}
              >
                Terapkan
              </button>
            </div>
          </div>
        )}
      </div>
      <button type="button" className="palette-btn" onClick={onOpenPalette} title="Lompat ke proyek (Alt+P)">
        <Icon name="search" size={14} />
        <span>Lompat ke proyek</span>
        <kbd>Alt+P</kbd>
      </button>
    </div>
  );
}
