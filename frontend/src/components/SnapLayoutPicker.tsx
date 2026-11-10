import { useEffect, useRef } from "react";
import { SNAP_LAYOUTS, type LayoutKind } from "../layout";
import { Icon } from "./Icon";

interface SnapLayoutPickerProps {
  currentLayout: LayoutKind;
  onSelectSnap: (layout: LayoutKind, targetSlot: number) => void;
  onClose: () => void;
  onOpenCustomGrid: () => void;
}

export function SnapLayoutPicker({
  currentLayout,
  onSelectSnap,
  onClose,
  onOpenCustomGrid,
}: SnapLayoutPickerProps) {
  const popoverRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const onPointerDown = (e: PointerEvent) => {
      if (popoverRef.current && !popoverRef.current.contains(e.target as Node)) {
        onClose();
      }
    };
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        onClose();
      }
    };
    window.addEventListener("pointerdown", onPointerDown);
    window.addEventListener("keydown", onKeyDown);
    return () => {
      window.removeEventListener("pointerdown", onPointerDown);
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [onClose]);

  return (
    <div
      ref={popoverRef}
      className="snap-picker-popover"
      role="dialog"
      aria-label="Snap Layouts Windows"
    >
      <div className="snap-picker-header">
        <div className="snap-picker-title-group">
          <Icon name="layoutSnap" size={15} />
          <span className="snap-picker-title">Snap Layouts</span>
        </div>
        <button
          type="button"
          className="snap-custom-btn"
          onClick={onOpenCustomGrid}
          title="Buka pemilih kisi matriks kustom (RxC)"
        >
          <span>Kisi Kustom</span>
          <Icon name="layoutCustom" size={13} />
        </button>
      </div>

      <div className="snap-cards-grid" role="group" aria-label="Pilihan template tata letak">
        {SNAP_LAYOUTS.map((layout) => {
          const isCurrent = currentLayout === layout.kind;
          return (
            <div
              key={layout.kind}
              className={`snap-card${isCurrent ? " is-active" : ""}`}
              style={{
                gridTemplateAreas: layout.areas,
                gridTemplateColumns: layout.defaultCols.map((c) => `${c}%`).join(" "),
                gridTemplateRows: layout.defaultRows.map((r) => `${r}%`).join(" "),
              }}
              title={layout.label}
              aria-label={layout.label}
            >
              {layout.zones.map((zone) => (
                <button
                  key={zone.slotIndex}
                  type="button"
                  className="snap-zone"
                  style={{ gridArea: `p${zone.slotIndex}` }}
                  onClick={() => onSelectSnap(layout.kind, zone.slotIndex)}
                  title={`${layout.label}: ${zone.label}`}
                  aria-label={`${layout.label}: ${zone.label}`}
                >
                  <span className="snap-zone-dot" />
                </button>
              ))}
            </div>
          );
        })}
      </div>
      <div className="snap-picker-footer">
        <span className="snap-picker-tip">Klik salah satu zona untuk menempatkan terminal aktif ke posisi tersebut.</span>
      </div>
    </div>
  );
}
