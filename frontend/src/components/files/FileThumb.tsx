import { useEffect, useRef, useState } from "react";
import { api } from "../../api";
import type { FileEntry } from "../../types";
import { EntryIcon, PlayIcon } from "./FileIcons";
import { joinPath, thumbKind } from "./fileUtils";

// Capture one video frame at a टाइम so a grid of 20 videos doesn't open 20
// parallel SFTP range streams and choke the connection.
const MAX_VIDEO_JOBS = 2;
let activeJobs = 0;
const jobQueue: Array<() => void> = [];

function runVideoJob<T>(fn: () => Promise<T>): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const start = () => {
      activeJobs++;
      fn()
        .then(resolve, reject)
        .finally(() => {
          activeJobs--;
          jobQueue.shift()?.();
        });
    };
    if (activeJobs < MAX_VIDEO_JOBS) start();
    else jobQueue.push(start);
  });
}

// Cache captured frames by (server, path, mtime) so toggling list↔grid or
// scrolling doesn't re-decode the same video.
const frameCache = new Map<string, string | false>();

function captureBrowserFrame(url: string, cacheKey: string): Promise<string | null> {
  const cached = frameCache.get(cacheKey);
  if (cached !== undefined) return Promise.resolve(cached || null);

  return runVideoJob(
    () =>
      new Promise<string | null>((resolve) => {
        const video = document.createElement("video");
        let settled = false;
        const finish = (result: string | null) => {
          if (settled) return;
          settled = true;
          clearTimeout(timer);
          video.removeAttribute("src");
          video.load();
          frameCache.set(cacheKey, result ?? false);
          resolve(result);
        };
        const timer = setTimeout(() => finish(null), 10000);

        video.muted = true;
        video.playsInline = true;
        video.preload = "metadata";
        video.crossOrigin = "use-credentials";

        video.onloadedmetadata = () => {
          const d = Number.isFinite(video.duration) && video.duration > 0 ? video.duration : 0;
          video.currentTime = d > 2 ? Math.min(1, d * 0.1) : 0.05;
        };
        video.onseeked = () => {
          try {
            const vw = video.videoWidth;
            const vh = video.videoHeight;
            if (!vw || !vh) return finish(null);
            const scale = Math.min(1, 200 / Math.max(vw, vh));
            const canvas = document.createElement("canvas");
            canvas.width = Math.max(1, Math.round(vw * scale));
            canvas.height = Math.max(1, Math.round(vh * scale));
            const ctx = canvas.getContext("2d");
            if (!ctx) return finish(null);
            ctx.drawImage(video, 0, 0, canvas.width, canvas.height);
            finish(canvas.toDataURL("image/jpeg", 0.78));
          } catch {
            finish(null);
          }
        };
        video.onerror = () => finish(null);
        video.src = url;
      })
  );
}

interface Props {
  serverId: number;
  dirPath: string;
  entry: FileEntry;
  iconSize?: number;
}

export function FileThumb({ serverId, dirPath, entry, iconSize = 36 }: Props) {
  const kind = thumbKind(entry);
  const fullPath = entry.path || joinPath(dirPath, entry.name);
  const version = `${entry.size}:${entry.mtime}`;
  const cacheKey = `${serverId}:${fullPath}:${version}`;

  const [src, setSrc] = useState<string | null>(() => {
    if (!kind) return null;
    if (kind === "image") return api.filesThumbUrl(serverId, fullPath, version);
    const cached = frameCache.get(cacheKey);
    return typeof cached === "string" ? cached : null;
  });
  const [failed, setFailed] = useState(() => frameCache.get(cacheKey) === false);
  const boxRef = useRef<HTMLSpanElement>(null);

  useEffect(() => {
    if (kind === "image") {
      setFailed(false);
      setSrc(api.filesThumbUrl(serverId, fullPath, version));
      return;
    }
    if (kind !== "video") return;

    const cached = frameCache.get(cacheKey);
    if (cached !== undefined) {
      setSrc(cached || null);
      setFailed(cached === false);
      return;
    }
    setSrc(null);
    setFailed(false);

    const el = boxRef.current;
    if (!el) return;
    let cancelled = false;
    const io = new IntersectionObserver(
      (entries) => {
        if (!entries.some((e) => e.isIntersecting)) return;
        io.disconnect();
        const url = api.filesViewUrl(serverId, fullPath);
        captureBrowserFrame(url, cacheKey).then((dataUrl) => {
          if (cancelled) return;
          if (dataUrl) setSrc(dataUrl);
          else setFailed(true);
        });
      },
      { rootMargin: "120px" }
    );
    io.observe(el);
    return () => {
      cancelled = true;
      io.disconnect();
    };
  }, [kind, serverId, fullPath, version, cacheKey]);

  if (!kind || failed) {
    return <EntryIcon entry={entry} size={iconSize} />;
  }

  return (
    <span ref={boxRef} className="fm-thumb">
      {src ? (
        <img
          src={src}
          alt=""
          loading="lazy"
          decoding="async"
          draggable={false}
          onError={() => {
            if (kind === "video") {
              captureBrowserFrame(api.filesViewUrl(serverId, fullPath), cacheKey).then(
                (dataUrl) => {
                  if (dataUrl) setSrc(dataUrl);
                  else setFailed(true);
                }
              );
            } else {
              setFailed(true);
            }
          }}
        />
      ) : (
        <EntryIcon entry={entry} size={iconSize} />
      )}
      {kind === "video" && (
        <span className="fm-thumb-badge">
          <PlayIcon size={10} />
        </span>
      )}
    </span>
  );
}
