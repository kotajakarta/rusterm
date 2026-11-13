import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api } from "../../api";
import { fileBookmarkMap } from "../../fileBookmarks";
import type {
  Bookmark,
  FileRemote,
  Server,
  TransferJob,
  TransferScan
} from "../../types";
import { ArrowLeftRightIcon, PanelRightIcon, ServerIcon, XIcon } from "./FileIcons";
import { type ClipboardState, FilePane, type PaneHandle } from "./FilePane";
import { joinPath } from "./fileUtils";
import { DirectTransferModal, TransferList } from "./RemoteSync";

interface FilesViewProps {
  servers: Server[];
  activeServerId: number | null;
  onSelectServer: (id: number) => void;
  onOpenTerminal?: (serverId: number, path: string) => void;
  /** Sidebar paths; the file manager shows them as its bookmarks. */
  bookmarks: Bookmark[];
  onToggleBookmark: (serverId: number, path: string) => void;
  targetRequest?: { serverId: number; path?: string; seq: number } | null;
}

export function FilesView({
  servers,
  activeServerId,
  onSelectServer,
  onOpenTerminal,
  bookmarks: sidebarBookmarks,
  onToggleBookmark,
  targetRequest
}: FilesViewProps) {
  const [split, setSplit] = useState<boolean>(() => {
    try {
      return localStorage.getItem("td_files_split") === "1";
    } catch {
      return false;
    }
  });
  const [showDetails, setShowDetails] = useState<boolean>(() => {
    try {
      return localStorage.getItem("td_files_details") === "1";
    } catch {
      return false;
    }
  });
  const [activePane, setActivePane] = useState<"left" | "right">("left");

  const defaultLeftServer = activeServerId ?? servers[0]?.id ?? 0;
  const [leftServerId, setLeftServerId] = useState<number>(defaultLeftServer);
  const [rightServerId, setRightServerId] = useState<number>(() => {
    const saved = Number(localStorage.getItem("td_files_right_srv") || 0);
    if (saved && servers.some((s) => s.id === saved)) return saved;
    const alt = servers.find((s) => s.id !== defaultLeftServer);
    return alt?.id ?? defaultLeftServer;
  });

  const [leftPath, setLeftPath] = useState<string>(
    () => localStorage.getItem(`td_files_path_L_${defaultLeftServer}`) || "~"
  );
  const [rightPath, setRightPath] = useState<string>(
    () => localStorage.getItem(`td_files_path_R_${rightServerId}`) || "~"
  );
  const [leftInitPath, setLeftInitPath] = useState<string | undefined>(leftPath);

  useEffect(() => {
    if (activeServerId && activeServerId !== leftServerId) {
      setLeftServerId(activeServerId);
      const saved = localStorage.getItem(`td_files_path_L_${activeServerId}`) || "~";
      setLeftPath(saved);
      setLeftInitPath(saved);
    }
  }, [activeServerId, leftServerId]);

  const lastTargetSeq = useRef<number>(0);
  useEffect(() => {
    if (!targetRequest || targetRequest.seq === lastTargetSeq.current) return;
    lastTargetSeq.current = targetRequest.seq;
    const sid = targetRequest.serverId;
    const nextPath =
      targetRequest.path || localStorage.getItem(`td_files_path_L_${sid}`) || "~";
    setActivePane("left");
    setLeftServerId(sid);
    setLeftPath(nextPath);
    setLeftInitPath(nextPath);
  }, [targetRequest]);

  const [clipboard, setClipboard] = useState<ClipboardState | null>(null);
  const [remotes, setRemotes] = useState<FileRemote[]>([]);
  const [jobs, setJobs] = useState<TransferJob[]>([]);
  const [detailsHost, setDetailsHost] = useState<HTMLDivElement | null>(null);
  const [directPrompt, setDirectPrompt] = useState<{
    mode: "copy" | "move";
    srcServerId: number;
    destServerId: number;
    srcDir: string;
    destDir: string;
    paths: string[];
    initialScan: TransferScan;
  } | null>(null);

  const leftRef = useRef<PaneHandle | null>(null);
  const rightRef = useRef<PaneHandle | null>(null);

  // File-manager bookmarks are the sidebar paths (single source in App); drop the old local cache.
  useEffect(() => {
    try {
      localStorage.removeItem("td_files_bookmarks");
    } catch {
      // ignore
    }
  }, []);

  const bookmarks = useMemo(() => fileBookmarkMap(sidebarBookmarks), [sidebarBookmarks]);

  const loadRemotes = useCallback(() => {
    api
      .filesRemotes()
      .then(setRemotes)
      .catch(() => {});
  }, []);

  useEffect(() => {
    loadRemotes();
  }, [loadRemotes]);

  // Poll running transfer jobs
  useEffect(() => {
    const running = jobs.filter((j) => j.state === "running");
    if (running.length === 0) return;
    const t = setInterval(async () => {
      const updates = await Promise.all(
        running.map((j) => api.getTransfer(j.id).catch(() => j))
      );
      let anyFinished = false;
      setJobs((prev) =>
        prev.map((p) => {
          const u = updates.find((x) => x.id === p.id);
          if (u && p.state === "running" && u.state !== "running") anyFinished = true;
          return u ?? p;
        })
      );
      if (anyFinished) {
        leftRef.current?.refresh();
        rightRef.current?.refresh();
      }
    }, 800);
    return () => clearInterval(t);
  }, [jobs]);

  const handleLeftPathChange = useCallback((p: string) => {
    setLeftPath(p);
    try {
      localStorage.setItem(`td_files_path_L_${leftServerId}`, p);
    } catch {
      // ignore
    }
  }, [leftServerId]);

  const handleRightPathChange = useCallback((p: string) => {
    setRightPath(p);
    try {
      localStorage.setItem(`td_files_path_R_${rightServerId}`, p);
    } catch {
      // ignore
    }
  }, [rightServerId]);

  const toggleSplit = useCallback(() => {
    setSplit((prev) => {
      const next = !prev;
      try {
        localStorage.setItem("td_files_split", next ? "1" : "0");
      } catch {
        // ignore
      }
      if (!next) setActivePane("left");
      return next;
    });
  }, []);

  const toggleDetails = useCallback(() => {
    setShowDetails((prev) => {
      const next = !prev;
      try {
        localStorage.setItem("td_files_details", next ? "1" : "0");
      } catch {
        // ignore
      }
      return next;
    });
  }, []);

  const handleInterServerTransfer = useCallback(
    async (
      mode: "copy" | "move",
      srcServerId: number,
      srcDir: string,
      names: string[],
      dstServerId: number,
      dstDir: string
    ) => {
      const paths = names.map((n) => joinPath(srcDir, n));
      try {
        const scan = await api.directTransferScan({
          source_server_id: srcServerId,
          dest_server_id: dstServerId,
          source_dir: srcDir,
          dest_dir: dstDir,
          paths,
          use_gitignore: true
        });
        setDirectPrompt({
          mode,
          srcServerId,
          destServerId: dstServerId,
          srcDir,
          destDir: dstDir,
          paths,
          initialScan: scan
        });
      } catch {
        // ignore
      }
    },
    []
  );

  useEffect(() => {
    const onKey = (ev: KeyboardEvent) => {
      const tag = (ev.target as HTMLElement)?.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;
      if (ev.key === "F3") {
        ev.preventDefault();
        toggleSplit();
      } else if (ev.key === "F11") {
        ev.preventDefault();
        toggleDetails();
      } else if (ev.altKey && ev.key === "1") {
        ev.preventDefault();
        setActivePane("left");
      } else if (ev.altKey && ev.key === "2" && split) {
        ev.preventDefault();
        setActivePane("right");
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [toggleSplit, toggleDetails, split]);

  if (servers.length === 0) {
    return (
      <div className="fm-empty-workspace">
        No servers configured. Add a server first to browse files.
      </div>
    );
  }

  const leftSrv = servers.find((s) => s.id === leftServerId);
  const rightSrv = servers.find((s) => s.id === rightServerId);

  return (
    <div className="fm-workspace">
      {split && (
        <div className="fm-mobile-pane-bar">
          <button
            type="button"
            className={`fm-mobile-pane-tab ${activePane === "left" ? "active" : ""}`}
            onClick={() => setActivePane("left")}
          >
            <ServerIcon size={12} />
            <span className="fm-mobile-pane-srv">{leftSrv?.name ?? "Pane 1"}</span>
            <span className="fm-mobile-pane-path">{leftPath}</span>
          </button>
          <button
            type="button"
            className="fm-mobile-pane-swap"
            title="Switch active pane"
            onClick={() => setActivePane((p) => (p === "left" ? "right" : "left"))}
          >
            <ArrowLeftRightIcon size={13} />
          </button>
          <button
            type="button"
            className={`fm-mobile-pane-tab ${activePane === "right" ? "active" : ""}`}
            onClick={() => setActivePane("right")}
          >
            <ServerIcon size={12} />
            <span className="fm-mobile-pane-srv">{rightSrv?.name ?? "Pane 2"}</span>
            <span className="fm-mobile-pane-path">{rightPath}</span>
          </button>
        </div>
      )}

      <div className={`fm-panes ${split ? "split" : "single"} active-${activePane}`}>
        <FilePane
          key={`left-${leftServerId}-${leftInitPath ?? "~"}`}
          paneId="left"
          active={activePane === "left"}
          onActivate={() => setActivePane("left")}
          servers={servers}
          serverId={leftServerId}
          onServerChange={(id) => {
            setLeftServerId(id);
            onSelectServer(id);
            const saved = localStorage.getItem(`td_files_path_L_${id}`) || "~";
            setLeftPath(saved);
            setLeftInitPath(saved);
          }}
          initialPath={leftInitPath}
          onPathChange={handleLeftPathChange}
          otherPane={split ? { serverId: rightServerId, path: rightPath } : null}
          onRefreshOther={() => rightRef.current?.refresh()}
          clipboard={clipboard}
          onClipboardChange={setClipboard}
          onOpenTerminal={onOpenTerminal}
          split={split}
          onToggleSplit={toggleSplit}
          showDetails={showDetails}
          onToggleDetails={toggleDetails}
          detailsHost={detailsHost}
          remotes={remotes}
          onRemotesChanged={loadRemotes}
          onJobStarted={(job) => setJobs((prev) => [job, ...prev])}
          onTransferToOther={handleInterServerTransfer}
          bookmarks={bookmarks}
          onToggleBookmark={onToggleBookmark}
          handleRef={(h) => {
            leftRef.current = h;
          }}
        />

        {split && (
          <FilePane
            key={`right-${rightServerId}`}
            paneId="right"
            active={activePane === "right"}
            onActivate={() => setActivePane("right")}
            servers={servers}
            serverId={rightServerId}
            onServerChange={(id) => {
              setRightServerId(id);
              try {
                localStorage.setItem("td_files_right_srv", String(id));
              } catch {
                // ignore
              }
              setRightPath(localStorage.getItem(`td_files_path_R_${id}`) || "~");
            }}
            initialPath={localStorage.getItem(`td_files_path_R_${rightServerId}`) || "~"}
            onPathChange={handleRightPathChange}
            otherPane={{ serverId: leftServerId, path: leftPath }}
            onRefreshOther={() => leftRef.current?.refresh()}
            clipboard={clipboard}
            onClipboardChange={setClipboard}
            onOpenTerminal={onOpenTerminal}
            split={split}
            onToggleSplit={toggleSplit}
            showDetails={showDetails}
            onToggleDetails={toggleDetails}
            detailsHost={detailsHost}
            remotes={remotes}
            onRemotesChanged={loadRemotes}
            onJobStarted={(job) => setJobs((prev) => [job, ...prev])}
            onTransferToOther={handleInterServerTransfer}
            bookmarks={bookmarks}
            onToggleBookmark={onToggleBookmark}
            handleRef={(h) => {
              rightRef.current = h;
            }}
          />
        )}

        {showDetails && (
          <aside className="fm-details">
            <div className="fm-details-header">
              <span style={{ display: "inline-flex", alignItems: "center", gap: 6 }}>
                <PanelRightIcon size={14} /> Details
              </span>
              <button type="button" className="fm-icon-btn" onClick={toggleDetails}>
                <XIcon size={14} />
              </button>
            </div>
            <div ref={setDetailsHost} className="fm-details-body" />
          </aside>
        )}
      </div>

      <TransferList
        jobs={jobs}
        onCancel={async (id) => {
          try {
            const u = await api.cancelTransfer(id);
            setJobs((prev) => prev.map((j) => (j.id === id ? u : j)));
          } catch {
            // ignore
          }
        }}
        onDismiss={(id) => setJobs((prev) => prev.filter((j) => j.id !== id))}
      />

      {directPrompt && (
        <DirectTransferModal
          mode={directPrompt.mode}
          sourceServerId={directPrompt.srcServerId}
          destServerId={directPrompt.destServerId}
          sourceDir={directPrompt.srcDir}
          destDir={directPrompt.destDir}
          paths={directPrompt.paths}
          initialScan={directPrompt.initialScan}
          servers={servers}
          onCancel={() => setDirectPrompt(null)}
          onStart={async (useGitignore, scan) => {
            if (scan.files === 0 && scan.dirs === 0) {
              setDirectPrompt(null);
              return;
            }
            const job = await api.directTransferStart({
              source_server_id: directPrompt.srcServerId,
              dest_server_id: directPrompt.destServerId,
              source_dir: directPrompt.srcDir,
              dest_dir: directPrompt.destDir,
              paths: directPrompt.paths,
              mode: directPrompt.mode,
              use_gitignore: useGitignore,
              ignore: useGitignore ? undefined : []
            });
            setJobs((prev) => [job, ...prev]);
            setDirectPrompt(null);
          }}
        />
      )}
    </div>
  );
}
