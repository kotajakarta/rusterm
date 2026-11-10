import { useEffect, useRef, useState } from "react";
import { api } from "../../api";
import type { FileEntry } from "../../types";
import {
  ChevronLeftIcon,
  ChevronRightIcon,
  DownloadIcon,
  ExternalLinkIcon,
  Maximize2Icon,
  Minimize2Icon,
  XIcon
} from "./FileIcons";
import { formatDate, formatSize, joinPath, previewKind } from "./fileUtils";

interface ViewerProps {
  serverId: number;
  dirPath: string;
  entry: FileEntry;
  siblings: FileEntry[];
  onSelect: (e: FileEntry) => void;
  onClose: () => void;
}

const RATES = [0.5, 0.75, 1, 1.25, 1.5, 2];

export function FileViewer({
  serverId,
  dirPath,
  entry,
  siblings,
  onSelect,
  onClose
}: ViewerProps) {
  const kind = previewKind(entry);
  const fullPath = entry.path || joinPath(dirPath, entry.name);
  const viewUrl = api.filesViewUrl(serverId, fullPath);
  const downloadUrl = api.filesDownloadUrl(serverId, fullPath);

  const idx = siblings.findIndex((s) => s.name === entry.name);
  const hasPrev = idx > 0;
  const hasNext = idx >= 0 && idx < siblings.length - 1;
  const goPrev = () => {
    if (hasPrev) onSelect(siblings[idx - 1]);
  };
  const goNext = () => {
    if (hasNext) onSelect(siblings[idx + 1]);
  };

  const [actualSize, setActualSize] = useState(false);
  const [imgMeta, setImgMeta] = useState<{ w: number; h: number } | null>(null);
  const [rate, setRate] = useState(1);
  const [mediaError, setMediaError] = useState<string | null>(null);
  const videoRef = useRef<HTMLVideoElement>(null);

  useEffect(() => {
    setActualSize(false);
    setImgMeta(null);
    setMediaError(null);
  }, [entry.name, entry.mtime]);

  useEffect(() => {
    if (videoRef.current) videoRef.current.playbackRate = rate;
  }, [rate, entry.name]);

  useEffect(() => {
    const onKey = (ev: KeyboardEvent) => {
      const tag = (ev.target as HTMLElement | null)?.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;
      if (ev.key === "Escape") {
        ev.preventDefault();
        onClose();
        return;
      }
      if (ev.key === "ArrowLeft" && !ev.altKey) {
        if (kind === "video" && ev.shiftKey && videoRef.current) {
          videoRef.current.currentTime = Math.max(0, videoRef.current.currentTime - 5);
          ev.preventDefault();
          return;
        }
        if (hasPrev) {
          ev.preventDefault();
          goPrev();
        }
      } else if (ev.key === "ArrowRight" && !ev.altKey) {
        if (kind === "video" && ev.shiftKey && videoRef.current) {
          videoRef.current.currentTime += 5;
          ev.preventDefault();
          return;
        }
        if (hasNext) {
          ev.preventDefault();
          goNext();
        }
      } else if (ev.key === " " && kind === "video" && videoRef.current && tag !== "VIDEO") {
        ev.preventDefault();
        if (videoRef.current.paused) videoRef.current.play().catch(() => {});
        else videoRef.current.pause();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  return (
    <div
      className="fm-modal-backdrop fm-viewer-backdrop"
      onMouseDown={(ev) => {
        if (ev.target === ev.currentTarget) onClose();
      }}
    >
      <div className="fm-modal fm-viewer-modal" role="dialog" aria-modal="true">
        <div className="fm-viewer-header">
          <div className="fm-viewer-title">
            <span className="fm-viewer-name" title={fullPath}>
              {entry.name}
            </span>
            <span className="fm-viewer-meta">
              {formatSize(entry.size)}
              {imgMeta ? ` · ${imgMeta.w}×${imgMeta.h}` : ""}
              {siblings.length > 1 ? ` · ${idx + 1} / ${siblings.length}` : ""}
            </span>
          </div>
          <div className="fm-viewer-toolbar">
            {siblings.length > 1 && (
              <>
                <button
                  type="button"
                  className="fm-btn fm-btn-sm"
                  disabled={!hasPrev}
                  onClick={goPrev}
                  title="Previous (←)"
                >
                  <ChevronLeftIcon size={14} />
                </button>
                <button
                  type="button"
                  className="fm-btn fm-btn-sm"
                  disabled={!hasNext}
                  onClick={goNext}
                  title="Next (→)"
                >
                  <ChevronRightIcon size={14} />
                </button>
              </>
            )}
            {kind === "image" && (
              <button
                type="button"
                className="fm-btn fm-btn-sm"
                onClick={() => setActualSize((v) => !v)}
                title={actualSize ? "Fit to window" : "Actual size (1:1)"}
              >
                {actualSize ? <Minimize2Icon size={14} /> : <Maximize2Icon size={14} />}
                <span>{actualSize ? "Fit" : "1:1"}</span>
              </button>
            )}
            {kind === "video" && (
              <>
                <select
                  className="fm-select fm-select-sm"
                  value={rate}
                  onChange={(e) => setRate(Number(e.target.value))}
                  title="Playback speed"
                >
                  {RATES.map((r) => (
                    <option key={r} value={r}>
                      {r}×
                    </option>
                  ))}
                </select>
                {typeof document !== "undefined" && "pictureInPictureEnabled" in document && (
                  <button
                    type="button"
                    className="fm-btn fm-btn-sm"
                    onClick={() => {
                      const v = videoRef.current;
                      if (!v) return;
                      if (document.pictureInPictureElement) {
                        document.exitPictureInPicture().catch(() => {});
                      } else {
                        v.requestPictureInPicture?.().catch(() => {});
                      }
                    }}
                  >
                    PiP
                  </button>
                )}
              </>
            )}
            <button
              type="button"
              className="fm-btn fm-btn-sm"
              onClick={() => window.open(viewUrl, "_blank", "noopener")}
              title="Open in new tab"
            >
              <ExternalLinkIcon size={14} />
            </button>
            <button
              type="button"
              className="fm-btn fm-btn-sm"
              onClick={() => {
                window.location.href = downloadUrl;
              }}
              title="Download"
            >
              <DownloadIcon size={14} />
              <span>Download</span>
            </button>
            <button
              type="button"
              className="fm-btn fm-btn-sm"
              onClick={onClose}
              title="Close (Esc)"
            >
              <XIcon size={14} />
            </button>
          </div>
        </div>

        <div className={`fm-viewer-stage ${actualSize ? "actual" : ""}`}>
          {siblings.length > 1 && (
            <button
              type="button"
              className="fm-viewer-nav prev"
              disabled={!hasPrev}
              onClick={goPrev}
              title="Previous (←)"
            >
              <ChevronLeftIcon size={22} />
            </button>
          )}

          {kind === "image" && (
            <img
              key={viewUrl}
              src={viewUrl}
              alt={entry.name}
              className="fm-viewer-img"
              onDoubleClick={() => setActualSize((v) => !v)}
              onLoad={(ev) => {
                const el = ev.currentTarget;
                setImgMeta({ w: el.naturalWidth, h: el.naturalHeight });
              }}
              onError={() => setMediaError("Browser tidak dapat merender gambar ini.")}
            />
          )}

          {kind === "video" && (
            <video
              key={viewUrl}
              ref={videoRef}
              src={viewUrl}
              className="fm-viewer-video"
              controls
              autoPlay
              playsInline
              preload="metadata"
              onLoadedMetadata={(ev) => {
                const v = ev.currentTarget;
                v.playbackRate = rate;
                if (v.videoWidth && v.videoHeight) {
                  setImgMeta({ w: v.videoWidth, h: v.videoHeight });
                }
              }}
              onError={() =>
                setMediaError(
                  "Codec atau container video tidak didukung oleh browser ini — gunakan tombol Download."
                )
              }
            />
          )}

          {kind === "audio" && (
            <div className="fm-viewer-audio">
              <div className="fm-viewer-audio-title">{entry.name}</div>
              <div className="fm-viewer-audio-sub">
                {formatSize(entry.size)} · Modified {formatDate(entry.mtime)}
              </div>
              <audio
                key={viewUrl}
                src={viewUrl}
                controls
                autoPlay
                preload="metadata"
                style={{ width: "100%" }}
                onError={() => setMediaError("Format audio tidak didukung oleh browser ini.")}
              />
            </div>
          )}

          {kind === "pdf" && (
            <iframe key={viewUrl} src={viewUrl} title={entry.name} className="fm-viewer-pdf" />
          )}

          {mediaError && <div className="fm-viewer-error">{mediaError}</div>}

          {siblings.length > 1 && (
            <button
              type="button"
              className="fm-viewer-nav next"
              disabled={!hasNext}
              onClick={goNext}
              title="Next (→)"
            >
              <ChevronRightIcon size={22} />
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
