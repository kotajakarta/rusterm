import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState
} from "react";
import { createPortal } from "react-dom";
import { api } from "../../api";
import type {
  FileEntry,
  FileListOut,
  FileRemote,
  Server,
  TransferDirection,
  TransferJob,
  TransferScan
} from "../../types";
import {
  DeleteDialog,
  EditorDialog,
  LinkDialog,
  NameDialog,
  type NameModalKind,
  PermissionsDialog
} from "./FileDialogs";
import {
  ArrowLeftRightIcon,
  ArrowUpIcon,
  ChevronRightIcon,
  ClipboardPasteIcon,
  CloudDownloadIcon,
  CloudUploadIcon,
  Columns2Icon,
  CopyIcon,
  CopyPlusIcon,
  DownloadIcon,
  EntryIcon,
  EyeIcon,
  EyeOffIcon,
  FileEditIcon,
  FilePlusIcon,
  FolderIcon,
  FolderInputIcon,
  FolderPlusIcon,
  HardDriveIcon,
  LayoutGridIcon,
  Link2Icon,
  ListIcon,
  MoreVerticalIcon,
  PanelRightIcon,
  PencilIcon,
  PlayIcon,
  RefreshCwIcon,
  ScissorsIcon,
  SearchIcon,
  ServerIcon,
  Settings2Icon,
  ShieldCheckIcon,
  SquareTerminalIcon,
  StarIcon,
  StarOffIcon,
  Trash2Icon,
  UploadIcon,
  XIcon
} from "./FileIcons";
import { FileThumb } from "./FileThumb";
import { FileViewer } from "./FileViewer";
import { RemoteDialog, TransferSummary } from "./RemoteSync";
import {
  formatDate,
  formatSize,
  isDirLike,
  joinPath,
  linkDestination,
  parentOf,
  previewKind,
  remotesFor,
  TYPE_LABEL
} from "./fileUtils";

const DRAG_MIME = "application/x-termdeck-files";

interface DragPayload {
  serverId: number;
  dirPath: string;
  names: string[];
}

let currentDrag: DragPayload | null = null;

export interface ClipboardState {
  mode: "copy" | "cut";
  serverId: number;
  dirPath: string;
  names: string[];
}

export interface PaneHandle {
  serverId: number;
  path: string;
  selectedPaths: () => string[];
  refresh: () => void;
}

export type SortKey = "name" | "size" | "mtime" | "owner";
export type SortDir = "asc" | "desc";
export type ViewMode = "list" | "grid";

interface UploadItem {
  id: string;
  name: string;
  size: number;
  pct: number;
  status: "uploading" | "done" | "error";
  error?: string;
}

interface DropPromptState {
  x: number;
  y: number;
  payload: DragPayload;
  destDir: string;
}

interface ContextMenuState {
  x: number;
  y: number;
  kind: "item" | "dir";
  targetEntry?: FileEntry;
}

interface RemoteScanConfirmState {
  remote: FileRemote;
  direction: TransferDirection;
  paths: string[];
  scan: TransferScan;
}

interface UploadConflictPrompt {
  file: File;
  resolve: (overwrite: boolean) => void;
}

const modeOctal = (mode: number) => `0${(mode & 0o7777).toString(8).padStart(3, "0")}`;

interface FilePaneProps {
  paneId: "left" | "right";
  active: boolean;
  onActivate: () => void;
  servers: Server[];
  serverId: number;
  onServerChange: (id: number) => void;
  initialPath?: string;
  otherPane: { serverId: number; path: string } | null;
  onRefreshOther: () => void;
  clipboard: ClipboardState | null;
  onClipboardChange: (c: ClipboardState | null) => void;
  onOpenTerminal?: (serverId: number, path: string) => void;
  onPathChange?: (path: string) => void;
  split: boolean;
  onToggleSplit: () => void;
  showDetails: boolean;
  onToggleDetails: () => void;
  detailsHost: HTMLDivElement | null;
  remotes: FileRemote[];
  onRemotesChanged: () => void;
  onJobStarted: (job: TransferJob) => void;
  onTransferToOther: (
    mode: "copy" | "move",
    srcServerId: number,
    srcDir: string,
    names: string[],
    dstServerId: number,
    dstDir: string
  ) => void;
  bookmarks: Record<string, string[]>;
  onToggleBookmark: (serverId: number, path: string) => void;
  handleRef?: (h: PaneHandle | null) => void;
}

export function FilePane({
  active,
  onActivate,
  servers,
  serverId,
  onServerChange,
  initialPath,
  otherPane,
  onRefreshOther,
  clipboard,
  onClipboardChange,
  onOpenTerminal,
  onPathChange,
  split,
  onToggleSplit,
  showDetails,
  onToggleDetails,
  detailsHost,
  remotes,
  onRemotesChanged,
  onJobStarted,
  onTransferToOther,
  bookmarks,
  onToggleBookmark,
  handleRef
}: FilePaneProps) {
  const [listing, setListing] = useState<FileListOut | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [toast, setToast] = useState<{ kind: "ok" | "error" | "info"; text: string } | null>(
    null
  );

  const [editingPath, setEditingPath] = useState(false);
  const [pathDraft, setPathDraft] = useState("");
  const [filter, setFilter] = useState("");
  const [showHidden, setShowHidden] = useState<boolean>(() => {
    try {
      return localStorage.getItem("td_files_hidden") === "1";
    } catch {
      return false;
    }
  });
  const [viewMode, setViewMode] = useState<ViewMode>(() => {
    try {
      return (localStorage.getItem("td_files_view") as ViewMode) || "list";
    } catch {
      return "list";
    }
  });
  const [sortKey, setSortKey] = useState<SortKey>("name");
  const [sortDir, setSortDir] = useState<SortDir>("asc");

  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [anchor, setAnchor] = useState<string | null>(null);
  const [focused, setFocused] = useState<string | null>(null);

  const [nameModal, setNameModal] = useState<{
    kind: NameModalKind;
    entry?: FileEntry;
  } | null>(null);
  const [linkModal, setLinkModal] = useState<{
    target?: string;
    name?: string;
  } | null>(null);
  const [deleteModal, setDeleteModal] = useState<FileEntry[] | null>(null);
  const [permEntry, setPermEntry] = useState<FileEntry | null>(null);
  const [editorEntry, setEditorEntry] = useState<FileEntry | null>(null);
  const [viewerEntry, setViewerEntry] = useState<FileEntry | null>(null);
  const [remoteDialog, setRemoteDialog] = useState<{ existing?: FileRemote } | null>(null);
  const [remoteScanConfirm, setRemoteScanConfirm] = useState<RemoteScanConfirmState | null>(null);
  const [uploadConflict, setUploadConflict] = useState<UploadConflictPrompt | null>(null);

  const [uploads, setUploads] = useState<UploadItem[]>([]);
  const [externalDragOver, setExternalDragOver] = useState(false);
  const [dropTargetDir, setDropTargetDir] = useState<string | null>(null);
  const [dropPrompt, setDropPrompt] = useState<DropPromptState | null>(null);
  const [contextMenu, setContextMenu] = useState<ContextMenuState | null>(null);
  const [bookmarkMenuOpen, setBookmarkMenuOpen] = useState(false);
  const [remoteMenuOpen, setRemoteMenuOpen] = useState(false);
  const [crumbFoldMenuOpen, setCrumbFoldMenuOpen] = useState(false);

  const fileInputRef = useRef<HTMLInputElement>(null);
  const paneRootRef = useRef<HTMLDivElement>(null);
  const crumbsRef = useRef<HTMLDivElement>(null);
  const [crumbMax, setCrumbMax] = useState<number>(8);

  const showToast = useCallback((kind: "ok" | "error" | "info", text: string) => {
    setToast({ kind, text });
  }, []);

  useEffect(() => {
    if (!toast) return;
    const t = setTimeout(() => setToast(null), 2800);
    return () => clearTimeout(t);
  }, [toast]);

  const currentPath = listing?.path ?? initialPath ?? "~";

  const onPathChangeRef = useRef(onPathChange);
  useEffect(() => {
    onPathChangeRef.current = onPathChange;
  }, [onPathChange]);

  const loadDir = useCallback(
    async (targetPath: string, preserveSelection = false) => {
      setLoading(true);
      setError(null);
      try {
        let resolvedTarget = targetPath;
        if (resolvedTarget === "~" || resolvedTarget.startsWith("~/")) {
          try {
            const p = await api.filesPrincipals(serverId);
            const home = p.home || "/root";
            resolvedTarget =
              resolvedTarget === "~" ? home : joinPath(home, resolvedTarget.slice(2));
          } catch {
            resolvedTarget = "/root";
          }
        }
        const res = await api.filesList(serverId, resolvedTarget, true);
        setListing(res);
        setPathDraft(res.path);
        onPathChangeRef.current?.(res.path);
        if (!preserveSelection) {
          setSelected(new Set());
          setFocused(null);
          setAnchor(null);
        }
      } catch (e) {
        setError(e instanceof Error ? e.message : "Failed to list directory");
      } finally {
        setLoading(false);
      }
    },
    [serverId]
  );

  const initialLoadedServerRef = useRef<number | null>(null);
  const lastInitialPathPropRef = useRef<string | undefined>(initialPath);
  useEffect(() => {
    const serverChanged = initialLoadedServerRef.current !== serverId;
    const pathPropChanged =
      initialPath !== undefined && initialPath !== lastInitialPathPropRef.current;
    if (serverChanged || pathPropChanged) {
      initialLoadedServerRef.current = serverId;
      lastInitialPathPropRef.current = initialPath;
      void loadDir(initialPath || "~");
    }
  }, [serverId, initialPath, loadDir]);

  const navigateTo = useCallback(
    (p: string) => {
      setEditingPath(false);
      setFilter("");
      let clean = p.trim() || "/";
      if (!clean.startsWith("/") && !clean.startsWith("~")) {
        clean = "/" + clean;
      }
      void loadDir(clean);
    },
    [loadDir]
  );

  const refresh = useCallback(() => {
    if (listing) void loadDir(listing.path, true);
    else void loadDir("~");
  }, [listing, loadDir]);

  useEffect(() => {
    if (!handleRef) return;
    handleRef({
      serverId,
      path: currentPath,
      selectedPaths: () => Array.from(selected).map((n) => joinPath(currentPath, n)),
      refresh
    });
    return () => handleRef(null);
  }, [handleRef, serverId, currentPath, selected, refresh]);

  const toggleHidden = useCallback(() => {
    setShowHidden((prev) => {
      const next = !prev;
      try {
        localStorage.setItem("td_files_hidden", next ? "1" : "0");
      } catch {
        // ignore
      }
      return next;
    });
  }, []);

  const changeViewMode = (m: ViewMode) => {
    setViewMode(m);
    try {
      localStorage.setItem("td_files_view", m);
    } catch {
      // ignore
    }
  };

  // --- Filtering & sorting ---
  const visibleEntries = useMemo(() => {
    if (!listing) return [];
    const q = filter.trim().toLowerCase();
    const filtered = listing.entries.filter((e) => {
      if (!showHidden && e.name.startsWith(".")) return false;
      if (q && !e.name.toLowerCase().includes(q)) return false;
      return true;
    });
    const mult = sortDir === "asc" ? 1 : -1;
    filtered.sort((a, b) => {
      const aDir = isDirLike(a) ? 0 : 1;
      const bDir = isDirLike(b) ? 0 : 1;
      if (aDir !== bDir) return aDir - bDir;
      switch (sortKey) {
        case "size":
          return (a.size - b.size) * mult || a.name.localeCompare(b.name);
        case "mtime":
          return (a.mtime - b.mtime) * mult || a.name.localeCompare(b.name);
        case "owner":
          return (
            `${a.owner}:${a.group}`.localeCompare(`${b.owner}:${b.group}`) * mult ||
            a.name.localeCompare(b.name)
          );
        case "name":
        default:
          return (
            a.name.localeCompare(b.name, undefined, {
              numeric: true,
              sensitivity: "base"
            }) * mult
          );
      }
    });
    return filtered;
  }, [listing, showHidden, filter, sortKey, sortDir]);

  const entryMap = useMemo(() => {
    const m = new Map<string, FileEntry>();
    if (listing) {
      for (const e of listing.entries) m.set(e.name, e);
    }
    return m;
  }, [listing]);

  const existingNames = useMemo(
    () => new Set(listing?.entries.map((e) => e.name) ?? []),
    [listing]
  );

  const selectedEntries = useMemo(
    () => Array.from(selected).map((n) => entryMap.get(n)).filter(Boolean) as FileEntry[],
    [selected, entryMap]
  );

  const focusedEntry = useMemo(() => {
    if (focused && entryMap.has(focused)) return entryMap.get(focused)!;
    if (selectedEntries.length === 1) return selectedEntries[0];
    return null;
  }, [focused, entryMap, selectedEntries]);

  const currentDirPseudoEntry: FileEntry | null = useMemo(() => {
    if (!listing) return null;
    if (listing.self) {
      return { ...listing.self, name: "" };
    }
    return {
      name: "",
      path: listing.path,
      type: "directory",
      size: listing.entries.length,
      mode: 0o755,
      perms: "drwxr-xr-x",
      owner: "root",
      group: "root",
      mtime: Date.now() / 1000
    };
  }, [listing]);

  // --- Selection helpers ---
  const selectOnly = (name: string) => {
    setSelected(new Set([name]));
    setAnchor(name);
    setFocused(name);
  };

  const handleRowClick = (ev: React.MouseEvent, entry: FileEntry) => {
    onActivate();
    const name = entry.name;
    const dirLike = isDirLike(entry);
    if (
      selected.has(name) &&
      selected.size === 1 &&
      dirLike &&
      !ev.shiftKey &&
      !ev.ctrlKey &&
      !ev.metaKey
    ) {
      openEntry(entry);
      return;
    }
    if (ev.shiftKey && anchor) {
      const names = visibleEntries.map((e) => e.name);
      const a = names.indexOf(anchor);
      const b = names.indexOf(name);
      if (a !== -1 && b !== -1) {
        const [lo, hi] = a < b ? [a, b] : [b, a];
        const next = new Set(ev.ctrlKey || ev.metaKey ? selected : []);
        for (let i = lo; i <= hi; i++) next.add(names[i]);
        setSelected(next);
        setFocused(name);
        return;
      }
    }
    if (ev.ctrlKey || ev.metaKey) {
      const next = new Set(selected);
      if (next.has(name)) next.delete(name);
      else next.add(name);
      setSelected(next);
      setAnchor(name);
      setFocused(name);
      return;
    }
    selectOnly(name);
  };

  const previewSiblings = useMemo(
    () => visibleEntries.filter((e) => previewKind(e) !== null),
    [visibleEntries]
  );

  const openEntry = useCallback(
    (entry: FileEntry) => {
      if (entry.type === "directory") {
        navigateTo(entry.path || joinPath(currentPath, entry.name));
        return;
      }
      if (entry.type === "link" && entry.target_type === "directory") {
        navigateTo(linkDestination(entry));
        return;
      }
      if (entry.type === "file" || (entry.type === "link" && entry.target_type === "file")) {
        if (previewKind(entry)) {
          setViewerEntry(entry);
        } else {
          setEditorEntry(entry);
        }
        return;
      }
      showToast("info", `Cannot open ${TYPE_LABEL[entry.type]} (${entry.name})`);
    },
    [currentPath, navigateTo, showToast]
  );

  // --- Clipboard / file operations ---
  const copySelection = useCallback(
    (mode: "copy" | "cut", targetEntries?: FileEntry[]) => {
      const list = targetEntries ?? selectedEntries;
      if (list.length === 0) return;
      onClipboardChange({
        mode,
        serverId,
        dirPath: currentPath,
        names: list.map((e) => e.name)
      });
      showToast(
        "info",
        `${mode === "copy" ? "Copied" : "Cut"} ${list.length} item${list.length > 1 ? "s" : ""}`
      );
    },
    [selectedEntries, onClipboardChange, serverId, currentPath, showToast]
  );

  const pasteInto = useCallback(
    async (destDir: string, payload?: DragPayload, moveOverride?: boolean) => {
      const src = payload ?? (clipboard ? { ...clipboard } : null);
      if (!src || src.names.length === 0) return;
      const isMove = moveOverride ?? (clipboard?.mode === "cut" && !payload);
      if (src.serverId !== serverId) {
        onTransferToOther(
          isMove ? "move" : "copy",
          src.serverId,
          src.dirPath,
          src.names,
          serverId,
          destDir
        );
        if (isMove && !payload) onClipboardChange(null);
        return;
      }
      const sources = src.names.map((n) => joinPath(src.dirPath, n));
      try {
        await api.filesAction(serverId, {
          op: "paste",
          paths: sources,
          dest_dir: destDir,
          mode: isMove ? "move" : "copy"
        });
        if (isMove && !payload) onClipboardChange(null);
        await loadDir(currentPath, true);
        if (otherPane && otherPane.serverId === serverId) onRefreshOther();
        showToast("ok", `${isMove ? "Moved" : "Copied"} ${sources.length} item(s)`);
      } catch (e) {
        showToast("error", e instanceof Error ? e.message : "Operation failed");
      }
    },
    [
      clipboard,
      serverId,
      currentPath,
      loadDir,
      onClipboardChange,
      otherPane,
      onRefreshOther,
      onTransferToOther,
      showToast
    ]
  );

  const transferToOtherPane = useCallback(
    async (mode: "copy" | "move", entries?: FileEntry[]) => {
      if (!otherPane) {
        showToast("info", "Open Split View (F3) to copy or move to the other pane");
        return;
      }
      const list = entries ?? selectedEntries;
      if (list.length === 0) return;
      if (otherPane.serverId !== serverId) {
        onTransferToOther(
          mode,
          serverId,
          currentPath,
          list.map((e) => e.name),
          otherPane.serverId,
          otherPane.path
        );
        return;
      }
      try {
        await api.filesAction(serverId, {
          op: "paste",
          paths: list.map((e) => e.path || joinPath(currentPath, e.name)),
          dest_dir: otherPane.path,
          mode
        });
        await loadDir(currentPath, true);
        onRefreshOther();
        showToast(
          "ok",
          `${mode === "copy" ? "Copied" : "Moved"} ${list.length} item(s) → ${otherPane.path}`
        );
      } catch (e) {
        showToast("error", e instanceof Error ? e.message : "Transfer failed");
      }
    },
    [
      otherPane,
      selectedEntries,
      serverId,
      currentPath,
      loadDir,
      onRefreshOther,
      onTransferToOther,
      showToast
    ]
  );

  const duplicateEntry = async (entry: FileEntry) => {
    try {
      const srcPath = entry.path || joinPath(currentPath, entry.name);
      const res = await api.filesAction<{ paths?: string[] }>(serverId, {
        op: "paste",
        paths: [srcPath],
        dest_dir: currentPath,
        mode: "copy"
      });
      await loadDir(currentPath, true);
      const createdPath = res.paths?.[0];
      const createdName = createdPath ? createdPath.split("/").pop() : undefined;
      if (createdName) selectOnly(createdName);
      showToast("ok", `Duplicated ${entry.name}`);
    } catch (e) {
      showToast("error", e instanceof Error ? e.message : "Duplicate failed");
    }
  };

  // --- Remote Sync (SFTP-style) ---
  const matchingRemotes = useMemo(
    () => remotesFor(remotes, serverId, [currentPath]),
    [remotes, serverId, currentPath]
  );

  const triggerRemoteTransfer = useCallback(
    async (remote: FileRemote, direction: TransferDirection, entries?: FileEntry[]) => {
      const paths =
        entries && entries.length > 0
          ? entries.map((e) => e.path || joinPath(currentPath, e.name))
          : [currentPath];
      try {
        const scan = await api.scanTransfer(remote.id, direction, paths);
        if (scan.files === 0 && scan.dirs === 0) {
          showToast("info", "Nothing to transfer (all items matched ignore rules)");
          return;
        }
        setRemoteScanConfirm({ remote, direction, paths, scan });
      } catch (e) {
        showToast("error", e instanceof Error ? e.message : "Failed to scan transfer");
      }
    },
    [currentPath, showToast]
  );

  // --- Uploads ---
  const handleFilesUpload = useCallback(
    async (fileList: FileList | File[], targetDir = currentPath) => {
      const files = Array.from(fileList);
      if (files.length === 0) return;

      for (const f of files) {
        const id = `${Date.now()}-${Math.random().toString(36).slice(2, 7)}`;
        setUploads((prev) => [
          ...prev,
          { id, name: f.name, size: f.size, pct: 0, status: "uploading" }
        ]);

        const runOne = async (overwrite: boolean): Promise<void> => {
          try {
            await api.filesUpload(serverId, targetDir, f, overwrite, (fraction) => {
              const pct = Math.round(fraction * 100);
              setUploads((prev) =>
                prev.map((u) => (u.id === id ? { ...u, pct } : u))
              );
            });
            setUploads((prev) =>
              prev.map((u) => (u.id === id ? { ...u, pct: 100, status: "done" } : u))
            );
          } catch (e) {
            const status = (e as { status?: number })?.status;
            if (status === 409 && !overwrite) {
              const confirmOverwrite = await new Promise<boolean>((resolve) => {
                setUploadConflict({ file: f, resolve });
              });
              setUploadConflict(null);
              if (confirmOverwrite) {
                return runOne(true);
              }
            }
            const msg = e instanceof Error ? e.message : "Upload failed";
            setUploads((prev) =>
              prev.map((u) => (u.id === id ? { ...u, status: "error", error: msg } : u))
            );
            showToast("error", `${f.name}: ${msg}`);
          }
        };

        await runOne(false);
      }

      await loadDir(currentPath, true);
      setTimeout(() => {
        setUploads((prev) => prev.filter((u) => u.status === "uploading"));
      }, 2500);
    },
    [currentPath, serverId, loadDir, showToast]
  );

  // --- Close floating menus on outside click ---
  useEffect(() => {
    if (!contextMenu && !dropPrompt && !bookmarkMenuOpen && !remoteMenuOpen && !crumbFoldMenuOpen) {
      return;
    }
    const closeAll = () => {
      setContextMenu(null);
      setDropPrompt(null);
      setBookmarkMenuOpen(false);
      setRemoteMenuOpen(false);
      setCrumbFoldMenuOpen(false);
    };
    window.addEventListener("mousedown", closeAll);
    return () => window.removeEventListener("mousedown", closeAll);
  }, [contextMenu, dropPrompt, bookmarkMenuOpen, remoteMenuOpen, crumbFoldMenuOpen]);

  // --- Keyboard navigation ---
  const handleKeyDown = (ev: React.KeyboardEvent) => {
    const tag = (ev.target as HTMLElement)?.tagName;
    if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;

    if (ev.key === "F5") {
      ev.preventDefault();
      if (split && otherPane) void transferToOtherPane("copy");
      else refresh();
      return;
    }
    if (ev.key === "F6" && split && otherPane) {
      ev.preventDefault();
      void transferToOtherPane("move");
      return;
    }
    if (ev.key === "F2" && focusedEntry) {
      ev.preventDefault();
      setNameModal({ kind: "rename", entry: focusedEntry });
      return;
    }
    if (ev.key === "F7") {
      ev.preventDefault();
      setNameModal({ kind: "mkdir" });
      return;
    }
    if (ev.key === "Delete" && selectedEntries.length > 0) {
      ev.preventDefault();
      setDeleteModal(selectedEntries);
      return;
    }
    if ((ev.ctrlKey || ev.metaKey) && ev.key.toLowerCase() === "a") {
      ev.preventDefault();
      setSelected(new Set(visibleEntries.map((e) => e.name)));
      return;
    }
    if ((ev.ctrlKey || ev.metaKey) && ev.key.toLowerCase() === "c") {
      ev.preventDefault();
      copySelection("copy");
      return;
    }
    if ((ev.ctrlKey || ev.metaKey) && ev.key.toLowerCase() === "x") {
      ev.preventDefault();
      copySelection("cut");
      return;
    }
    if ((ev.ctrlKey || ev.metaKey) && ev.key.toLowerCase() === "v") {
      ev.preventDefault();
      void pasteInto(currentPath);
      return;
    }
    if ((ev.ctrlKey || ev.metaKey) && ev.key === ".") {
      ev.preventDefault();
      toggleHidden();
      return;
    }
    if ((ev.altKey && ev.key === "ArrowUp") || ev.key === "Backspace") {
      ev.preventDefault();
      if (currentPath !== "/") navigateTo(parentOf(currentPath));
      return;
    }
    if (ev.key === "Enter" && focusedEntry) {
      ev.preventDefault();
      openEntry(focusedEntry);
      return;
    }
    if (ev.key === "ArrowDown" || ev.key === "ArrowUp" || ev.key === "Home" || ev.key === "End") {
      if (visibleEntries.length === 0) return;
      ev.preventDefault();
      const names = visibleEntries.map((e) => e.name);
      const curIdx = focused ? names.indexOf(focused) : -1;
      let nextIdx = 0;
      if (ev.key === "Home") nextIdx = 0;
      else if (ev.key === "End") nextIdx = names.length - 1;
      else if (ev.key === "ArrowDown") nextIdx = Math.min(names.length - 1, curIdx + 1);
      else nextIdx = Math.max(0, curIdx <= 0 ? 0 : curIdx - 1);
      selectOnly(names[nextIdx]);
    }
  };

  // --- Drag & drop handlers ---
  const onRowDragStart = (ev: React.DragEvent, entry: FileEntry) => {
    const names = selected.has(entry.name) ? Array.from(selected) : [entry.name];
    if (!selected.has(entry.name)) selectOnly(entry.name);
    const payload: DragPayload = { serverId, dirPath: currentPath, names };
    currentDrag = payload;
    ev.dataTransfer.setData(DRAG_MIME, JSON.stringify(payload));
    ev.dataTransfer.effectAllowed = "copyMove";
  };

  const onRowDragEnd = () => {
    currentDrag = null;
    setDropTargetDir(null);
  };

  const isInternalDrag = (ev: React.DragEvent) =>
    currentDrag !== null || ev.dataTransfer.types.includes(DRAG_MIME);

  const readDragPayload = (ev: React.DragEvent): DragPayload | null => {
    if (currentDrag) return currentDrag;
    try {
      const raw = ev.dataTransfer.getData(DRAG_MIME);
      return raw ? (JSON.parse(raw) as DragPayload) : null;
    } catch {
      return null;
    }
  };

  const handleDropOnDir = (ev: React.DragEvent, destDir: string) => {
    ev.preventDefault();
    ev.stopPropagation();
    setDropTargetDir(null);
    setExternalDragOver(false);

    const payload = readDragPayload(ev);
    if (payload) {
      currentDrag = null;
      if (payload.serverId === serverId && payload.dirPath === destDir) return;
      if (payload.serverId === serverId && payload.names.length === 1) {
        const srcFull = joinPath(payload.dirPath, payload.names[0]);
        if (srcFull === destDir) return;
      }
      if (ev.ctrlKey) {
        void pasteInto(destDir, payload, false);
      } else if (ev.shiftKey) {
        void pasteInto(destDir, payload, true);
      } else {
        setDropPrompt({ x: ev.clientX, y: ev.clientY, payload, destDir });
      }
      return;
    }

    if (ev.dataTransfer.files && ev.dataTransfer.files.length > 0) {
      void handleFilesUpload(ev.dataTransfer.files, destDir);
    }
  };

  // --- Sort header click ---
  const toggleSort = (key: SortKey) => {
    if (sortKey === key) setSortDir((d) => (d === "asc" ? "desc" : "asc"));
    else {
      setSortKey(key);
      setSortDir(key === "mtime" || key === "size" ? "desc" : "asc");
    }
  };

  // --- Breadcrumb segments (auto-collapse middle when space is tight) ---
  const crumbs = useMemo(() => {
    const p = listing?.path ?? "/";
    if (p === "/") return [];
    const parts = p.split("/").filter(Boolean);
    return parts.map((seg, idx) => ({
      label: seg,
      path: "/" + parts.slice(0, idx + 1).join("/")
    }));
  }, [listing?.path]);

  useLayoutEffect(() => {
    const el = crumbsRef.current;
    if (!el) return;
    const calc = (w: number) => {
      const fit = Math.max(2, Math.floor((w - 56) / 96));
      setCrumbMax(fit);
    };
    calc(el.clientWidth);
    if (typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver((entries) => {
      for (const e of entries) calc(e.contentRect.width);
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, [editingPath]);

  const foldedCrumbs = useMemo(() => {
    if (crumbs.length <= crumbMax) {
      return {
        head: crumbs,
        folded: [] as typeof crumbs,
        tail: [] as typeof crumbs
      };
    }
    const tailCount = Math.max(1, crumbMax - 1);
    return {
      head: [crumbs[0]],
      folded: crumbs.slice(1, crumbs.length - tailCount),
      tail: crumbs.slice(crumbs.length - tailCount)
    };
  }, [crumbs, crumbMax]);

  const openItemMenu = (ev: React.MouseEvent, entry: FileEntry) => {
    ev.preventDefault();
    ev.stopPropagation();
    onActivate();
    if (!selected.has(entry.name)) selectOnly(entry.name);
    setContextMenu({
      x: Math.min(ev.clientX, window.innerWidth - 240),
      y: Math.min(ev.clientY, window.innerHeight - 320),
      kind: "item",
      targetEntry: entry
    });
  };

  const openDirMenu = (ev: React.MouseEvent) => {
    ev.preventDefault();
    ev.stopPropagation();
    onActivate();
    setContextMenu({
      x: Math.min(ev.clientX, window.innerWidth - 240),
      y: Math.min(ev.clientY, window.innerHeight - 280),
      kind: "dir"
    });
  };

  // --- Details Panel content (rendered into shared aside via portal) ---
  const detailsTarget = focusedEntry ?? currentDirPseudoEntry;
  const detailsNode =
    active && showDetails && detailsHost && detailsTarget
      ? createPortal(
          <div className="fm-details-content">
            <div className="fm-details-hero">
              <EntryIcon entry={detailsTarget} size={28} />
              <div style={{ minWidth: 0 }}>
                <div className="fm-details-name">
                  {detailsTarget.name || currentPath}
                </div>
                <div className="fm-muted" style={{ fontSize: 12 }}>
                  {TYPE_LABEL[detailsTarget.type]}
                  {detailsTarget.target ? ` → ${detailsTarget.target}` : ""}
                </div>
              </div>
            </div>

            <dl className="fm-details-dl">
              <dt>Location</dt>
              <dd className="fm-mono">{currentPath}</dd>
              {detailsTarget.name && (
                <>
                  <dt>Size</dt>
                  <dd>
                    {detailsTarget.type === "directory"
                      ? "Directory"
                      : `${formatSize(detailsTarget.size)} (${detailsTarget.size.toLocaleString()} B)`}
                  </dd>
                </>
              )}
              <dt>Owner</dt>
              <dd>
                {detailsTarget.owner}:{detailsTarget.group}
              </dd>
              <dt>Permissions</dt>
              <dd className="fm-mono">
                {detailsTarget.perms} ({modeOctal(detailsTarget.mode)})
              </dd>
              <dt>Modified</dt>
              <dd>{formatDate(detailsTarget.mtime)}</dd>
            </dl>

            <div className="fm-details-actions">
              {detailsTarget.name &&
                (detailsTarget.type === "directory" ||
                  detailsTarget.target_type === "directory") && (
                  <button
                    type="button"
                    className="fm-btn fm-btn-primary"
                    onClick={() => openEntry(detailsTarget)}
                  >
                    <FolderIcon size={14} />
                    <span>Open directory</span>
                  </button>
                )}
              {detailsTarget.name &&
                (detailsTarget.type === "file" || detailsTarget.target_type === "file") && (
                  <>
                    {previewKind(detailsTarget) && (
                      <button
                        type="button"
                        className="fm-btn fm-btn-primary"
                        onClick={() => setViewerEntry(detailsTarget)}
                      >
                        <PlayIcon size={14} />
                        <span>Preview</span>
                      </button>
                    )}
                    <button
                      type="button"
                      className="fm-btn"
                      onClick={() => setEditorEntry(detailsTarget)}
                    >
                      <FileEditIcon size={14} />
                      <span>View / Edit text</span>
                    </button>
                    <button
                      type="button"
                      className="fm-btn"
                      onClick={() => {
                        window.location.href = api.filesDownloadUrl(
                          serverId,
                          detailsTarget.path || joinPath(currentPath, detailsTarget.name)
                        );
                      }}
                    >
                      <DownloadIcon size={14} />
                      <span>Download</span>
                    </button>
                  </>
                )}
              <button
                type="button"
                className="fm-btn"
                onClick={() => setPermEntry(detailsTarget)}
              >
                <ShieldCheckIcon size={14} />
                <span>Edit permissions</span>
              </button>
              {detailsTarget.name && (
                <>
                  <button
                    type="button"
                    className="fm-btn"
                    onClick={() => setNameModal({ kind: "rename", entry: detailsTarget })}
                  >
                    <PencilIcon size={14} />
                    <span>Rename</span>
                  </button>
                  <button
                    type="button"
                    className="fm-btn fm-btn-danger"
                    onClick={() => setDeleteModal([detailsTarget])}
                  >
                    <Trash2Icon size={14} />
                    <span>Delete</span>
                  </button>
                </>
              )}
            </div>
          </div>,
          detailsHost
        )
      : null;

  const totalSelectedBytes = useMemo(
    () => selectedEntries.reduce((acc, e) => acc + (e.type === "file" ? e.size : 0), 0),
    [selectedEntries]
  );

  const serverBookmarks = bookmarks[String(serverId)] ?? [];
  const isBookmarked = serverBookmarks.includes(currentPath);

  const otherSrvName = otherPane
    ? servers.find((s) => s.id === otherPane.serverId)?.name ?? `#${otherPane.serverId}`
    : "";

  return (
    <div
      ref={paneRootRef}
      tabIndex={0}
      className={`fm-pane ${active ? "active" : ""}`}
      onMouseDown={onActivate}
      onKeyDown={handleKeyDown}
      onDragOver={(ev) => {
        ev.preventDefault();
        if (!isInternalDrag(ev)) setExternalDragOver(true);
      }}
      onDragLeave={(ev) => {
        if (ev.currentTarget === ev.target) setExternalDragOver(false);
      }}
      onDrop={(ev) => handleDropOnDir(ev, currentPath)}
    >
      {/* --- Toolbar --- */}
      <div className="fm-toolbar">
        <div className="fm-server-select-wrap" title="Ganti server untuk panel ini">
          <ServerIcon size={13} className="fm-server-select-icon" />
          <select
            className="fm-server-select"
            value={serverId}
            onChange={(e) => onServerChange(Number(e.target.value))}
          >
            {servers.map((s) => (
              <option key={s.id} value={s.id}>
                {s.name}
              </option>
            ))}
          </select>
        </div>

        {/* Bookmarks dropdown */}
        <div className="fm-dropdown-wrap" onMouseDown={(e) => e.stopPropagation()}>
          <button
            type="button"
            className={`fm-btn fm-btn-sm ${isBookmarked ? "fm-btn-star-active" : ""}`}
            title="Bookmark direktori"
            onClick={() => setBookmarkMenuOpen((v) => !v)}
          >
            <StarIcon
              size={14}
              style={{
                fill: isBookmarked ? "#f59e0b" : "none",
                color: isBookmarked ? "#f59e0b" : undefined
              }}
            />
          </button>
          {bookmarkMenuOpen && (
            <div className="fm-menu fm-bookmark-menu">
              <button
                type="button"
                className="fm-menu-item"
                onClick={() => {
                  onToggleBookmark(serverId, currentPath);
                  setBookmarkMenuOpen(false);
                }}
              >
                {isBookmarked ? (
                  <StarOffIcon size={14} />
                ) : (
                  <StarIcon size={14} style={{ color: "#f59e0b" }} />
                )}
                <span>
                  {isBookmarked
                    ? `Hapus bookmark "${currentPath}"`
                    : `Bookmark "${currentPath}"`}
                </span>
              </button>
              <div className="fm-menu-sep" />
              {serverBookmarks.length === 0 ? (
                <div className="fm-menu-empty">Belum ada bookmark di server ini</div>
              ) : (
                serverBookmarks.map((p) => (
                  <div
                    key={p}
                    className={`fm-menu-item fm-menu-item-split ${p === currentPath ? "active" : ""}`}
                  >
                    <button
                      type="button"
                      className="fm-menu-item-main"
                      onClick={() => {
                        setBookmarkMenuOpen(false);
                        navigateTo(p);
                      }}
                    >
                      <FolderIcon size={13} style={{ color: "#60a5fa" }} />
                      <span className="fm-mono fm-truncate" title={p}>
                        {p}
                      </span>
                    </button>
                    <button
                      type="button"
                      className="fm-icon-btn fm-danger-hover"
                      title="Hapus bookmark"
                      onClick={(ev) => {
                        ev.stopPropagation();
                        onToggleBookmark(serverId, p);
                      }}
                    >
                      <Trash2Icon size={12} />
                    </button>
                  </div>
                ))
              )}
            </div>
          )}
        </div>

        {/* Up button */}
        <button
          type="button"
          className="fm-btn fm-btn-sm"
          disabled={!listing || listing.path === "/"}
          onClick={() => listing && navigateTo(parentOf(listing.path))}
          title="Parent directory (Alt+↑ / Backspace)"
        >
          <ArrowUpIcon size={14} />
        </button>

        {/* Smart Breadcrumb / Path Input */}
        {editingPath ? (
          <form
            className="fm-path-form"
            onSubmit={(e) => {
              e.preventDefault();
              navigateTo(pathDraft.trim() || "/");
            }}
          >
            <input
              className="fm-input fm-input-sm fm-mono fm-path-input"
              value={pathDraft}
              autoFocus
              onChange={(e) => setPathDraft(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Escape") {
                  e.preventDefault();
                  setEditingPath(false);
                  setPathDraft(currentPath);
                }
              }}
            />
            <button
              type="submit"
              className="fm-btn fm-btn-sm fm-btn-primary fm-path-go"
              title="Navigasi ke path"
            >
              <ChevronRightIcon size={14} />
            </button>
            <button
              type="button"
              className="fm-btn fm-btn-sm fm-path-cancel"
              onClick={() => {
                setEditingPath(false);
                setPathDraft(currentPath);
              }}
              title="Batal"
            >
              <XIcon size={13} />
            </button>
          </form>
        ) : (
          <div
            ref={crumbsRef}
            className="fm-crumbs"
            onDoubleClick={() => {
              setPathDraft(currentPath);
              setEditingPath(true);
            }}
            title="Double-click to edit path"
          >
            <button
              type="button"
              className={`fm-crumb ${crumbs.length === 0 ? "current" : ""}`}
              onClick={() => navigateTo("/")}
              onDragOver={(ev) => {
                ev.preventDefault();
                setDropTargetDir("/");
              }}
              onDragLeave={() => setDropTargetDir(null)}
              onDrop={(ev) => handleDropOnDir(ev, "/")}
              title="/"
            >
              <HardDriveIcon size={13} />
              <span>/</span>
            </button>
            {foldedCrumbs.head.map((c, idx) => {
              const isLast =
                foldedCrumbs.folded.length === 0 &&
                foldedCrumbs.tail.length === 0 &&
                idx === foldedCrumbs.head.length - 1;
              return (
                <span
                  key={c.path}
                  style={{ display: "inline-flex", alignItems: "center", minWidth: 0 }}
                >
                  <span className="fm-crumb-sep">/</span>
                  <button
                    type="button"
                    className={`fm-crumb ${isLast ? "current" : ""} ${
                      dropTargetDir === c.path ? "drop-target" : ""
                    }`}
                    onClick={() => navigateTo(c.path)}
                    onDragOver={(ev) => {
                      ev.preventDefault();
                      setDropTargetDir(c.path);
                    }}
                    onDragLeave={() => setDropTargetDir(null)}
                    onDrop={(ev) => handleDropOnDir(ev, c.path)}
                    title={c.path}
                  >
                    {c.label}
                  </button>
                </span>
              );
            })}
            {foldedCrumbs.folded.length > 0 && (
              <span
                className="fm-dropdown-wrap"
                style={{ display: "inline-flex", alignItems: "center" }}
                onMouseDown={(e) => e.stopPropagation()}
              >
                <span className="fm-crumb-sep">/</span>
                <button
                  type="button"
                  className="fm-crumb fm-crumb-ellipsis"
                  title="Folder perantara"
                  onClick={() => setCrumbFoldMenuOpen((v) => !v)}
                >
                  …
                </button>
                {crumbFoldMenuOpen && (
                  <div className="fm-menu">
                    {foldedCrumbs.folded.map((fc) => (
                      <button
                        key={fc.path}
                        type="button"
                        className="fm-menu-item"
                        onClick={() => {
                          setCrumbFoldMenuOpen(false);
                          navigateTo(fc.path);
                        }}
                      >
                        <FolderIcon size={13} style={{ color: "#60a5fa" }} />
                        <span className="fm-mono">{fc.path}</span>
                      </button>
                    ))}
                  </div>
                )}
              </span>
            )}
            {foldedCrumbs.tail.map((c, idx) => {
              const isLast = idx === foldedCrumbs.tail.length - 1;
              return (
                <span
                  key={c.path}
                  style={{ display: "inline-flex", alignItems: "center", minWidth: 0 }}
                >
                  <span className="fm-crumb-sep">/</span>
                  <button
                    type="button"
                    className={`fm-crumb ${isLast ? "current" : ""} ${
                      dropTargetDir === c.path ? "drop-target" : ""
                    }`}
                    onClick={() => navigateTo(c.path)}
                    onDragOver={(ev) => {
                      ev.preventDefault();
                      setDropTargetDir(c.path);
                    }}
                    onDragLeave={() => setDropTargetDir(null)}
                    onDrop={(ev) => handleDropOnDir(ev, c.path)}
                    title={c.path}
                  >
                    {c.label}
                  </button>
                </span>
              );
            })}
            <button
              type="button"
              className="fm-crumb-edit"
              onClick={() => {
                setPathDraft(currentPath);
                setEditingPath(true);
              }}
              title="Edit path"
            >
              <PencilIcon size={12} />
            </button>
          </div>
        )}

        {/* Filter input */}
        <div className="fm-filter-wrap">
          <SearchIcon size={12} className="fm-filter-icon" />
          <input
            className="fm-filter-input"
            placeholder="Filter name…"
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
          />
          {filter && (
            <button
              type="button"
              className="fm-filter-clear"
              onClick={() => setFilter("")}
              title="Clear filter"
            >
              <XIcon size={11} />
            </button>
          )}
        </div>

        {/* List / Grid view switch */}
        <div className="fm-segmented">
          <button
            type="button"
            className={`fm-seg-btn ${viewMode === "list" ? "active" : ""}`}
            onClick={() => changeViewMode("list")}
            title="List view"
          >
            <ListIcon size={13} />
          </button>
          <button
            type="button"
            className={`fm-seg-btn ${viewMode === "grid" ? "active" : ""}`}
            onClick={() => changeViewMode("grid")}
            title="Grid view"
          >
            <LayoutGridIcon size={13} />
          </button>
        </div>

        {/* SFTP Remote Sync menu */}
        <div className="fm-dropdown-wrap" onMouseDown={(e) => e.stopPropagation()}>
          <button
            type="button"
            className={`fm-btn fm-btn-sm ${matchingRemotes.length > 0 ? "fm-btn-accent" : ""}`}
            title="SFTP Remote Sync (Upload / Download between servers)"
            onClick={() => setRemoteMenuOpen((v) => !v)}
          >
            <ArrowLeftRightIcon size={14} />
            <span className="fm-btn-label">
              {matchingRemotes.length > 0 ? `Remotes (${matchingRemotes.length})` : "Remotes"}
            </span>
          </button>
          {remoteMenuOpen && (
            <div className="fm-menu fm-remote-menu">
              {matchingRemotes.map((r) => (
                <div key={r.id}>
                  <button
                    type="button"
                    className="fm-menu-item"
                    onClick={() => {
                      setRemoteMenuOpen(false);
                      void triggerRemoteTransfer(r, "upload");
                    }}
                  >
                    <CloudUploadIcon size={14} />
                    <span>Upload directory to &quot;{r.name}&quot;</span>
                  </button>
                  <button
                    type="button"
                    className="fm-menu-item"
                    onClick={() => {
                      setRemoteMenuOpen(false);
                      void triggerRemoteTransfer(r, "download");
                    }}
                  >
                    <CloudDownloadIcon size={14} />
                    <span>Download directory from &quot;{r.name}&quot;</span>
                  </button>
                  <button
                    type="button"
                    className="fm-menu-item"
                    onClick={() => {
                      setRemoteMenuOpen(false);
                      setRemoteDialog({ existing: r });
                    }}
                  >
                    <Settings2Icon size={14} />
                    <span>
                      Edit &quot;{r.name}&quot; ({r.local_path} ↔ {r.remote_path})
                    </span>
                  </button>
                  <div className="fm-menu-sep" />
                </div>
              ))}
              <button
                type="button"
                className="fm-menu-item"
                onClick={() => {
                  setRemoteMenuOpen(false);
                  setRemoteDialog({});
                }}
              >
                <ArrowLeftRightIcon size={14} />
                <span>+ Add remote mapping (SFTP)…</span>
              </button>
            </div>
          )}
        </div>

        {/* Upload button */}
        <button
          type="button"
          className="fm-btn fm-btn-sm"
          onClick={() => fileInputRef.current?.click()}
          title="Upload files (or drag & drop onto table)"
        >
          <UploadIcon size={14} />
          <span className="fm-btn-label">Upload</span>
        </button>
        <input
          ref={fileInputRef}
          type="file"
          multiple
          style={{ display: "none" }}
          onChange={(e) => {
            if (e.target.files) void handleFilesUpload(e.target.files);
            e.target.value = "";
          }}
        />

        {/* Open Terminal Here */}
        {onOpenTerminal && (
          <button
            type="button"
            className="fm-btn fm-btn-sm"
            onClick={() => onOpenTerminal(serverId, currentPath)}
            title="Open terminal here"
          >
            <SquareTerminalIcon size={14} />
            <span className="fm-btn-label">Terminal</span>
          </button>
        )}

        {/* Refresh */}
        <button
          type="button"
          className="fm-btn fm-btn-sm"
          onClick={refresh}
          disabled={loading}
          title="Refresh (F5)"
        >
          <RefreshCwIcon size={14} className={loading ? "fm-spin" : ""} />
        </button>

        {/* Split view F3 */}
        <button
          type="button"
          className={`fm-btn fm-btn-sm ${split ? "fm-btn-primary" : ""}`}
          onClick={onToggleSplit}
          title="Dual-pane Split View (F3)"
        >
          <Columns2Icon size={14} />
          <span className="fm-btn-label">Split</span>
        </button>

        {/* Details panel F11 */}
        <button
          type="button"
          className={`fm-btn fm-btn-sm ${showDetails ? "fm-btn-primary" : ""}`}
          onClick={onToggleDetails}
          title="Toggle Details Panel (F11)"
        >
          <PanelRightIcon size={14} />
        </button>

        {/* Directory ⋮ menu */}
        <button
          type="button"
          className="fm-btn fm-btn-sm"
          title="Directory actions"
          onClick={openDirMenu}
        >
          <MoreVerticalIcon size={14} />
        </button>
      </div>

      {/* Inline Toast / Error Banner */}
      {toast && (
        <div className={`fm-toast fm-toast-${toast.kind}`}>
          <span>{toast.text}</span>
          <button type="button" className="fm-icon-btn" onClick={() => setToast(null)}>
            <XIcon size={12} />
          </button>
        </div>
      )}
      {error && (
        <div className="fm-alert fm-alert-error" style={{ margin: "6px 10px 0" }}>
          <span>{error}</span>
          <button type="button" className="fm-btn fm-btn-xs" onClick={() => setError(null)}>
            Dismiss
          </button>
        </div>
      )}

      {/* --- Main Directory Stage --- */}
      <div
        className="fm-stage"
        onContextMenu={(ev) => {
          if ((ev.target as HTMLElement).closest(".fm-row, .fm-card")) return;
          openDirMenu(ev);
        }}
        onClick={(ev) => {
          if (!(ev.target as HTMLElement).closest(".fm-row, .fm-card")) {
            setSelected(new Set());
            setFocused(null);
          }
        }}
      >
        {loading && !listing ? (
          <div className="fm-empty">Loading directory…</div>
        ) : visibleEntries.length === 0 ? (
          <div className="fm-empty">
            <div>
              {filter ? `No items matching "${filter}"` : "Directory is empty"}
            </div>
            <div style={{ display: "flex", gap: 8 }}>
              <button
                type="button"
                className="fm-btn fm-btn-sm"
                onClick={() => setNameModal({ kind: "mkdir" })}
              >
                <FolderPlusIcon size={13} />
                <span>Create directory</span>
              </button>
              <button
                type="button"
                className="fm-btn fm-btn-sm"
                onClick={() => setNameModal({ kind: "create_file" })}
              >
                <FilePlusIcon size={13} />
                <span>Create file</span>
              </button>
              {!showHidden && (listing?.entries.length ?? 0) > 0 && (
                <button type="button" className="fm-btn fm-btn-sm" onClick={toggleHidden}>
                  <EyeIcon size={13} />
                  <span>Show {listing?.entries.length} hidden items</span>
                </button>
              )}
            </div>
          </div>
        ) : viewMode === "list" ? (
          <table className="fm-table">
            <thead>
              <tr>
                <th onClick={() => toggleSort("name")}>
                  Name {sortKey === "name" ? (sortDir === "asc" ? "▲" : "▼") : ""}
                </th>
                <th className="col-size" onClick={() => toggleSort("size")}>
                  Size {sortKey === "size" ? (sortDir === "asc" ? "▲" : "▼") : ""}
                </th>
                <th className="col-perm" onClick={() => toggleSort("owner")}>
                  Permissions {sortKey === "owner" ? (sortDir === "asc" ? "▲" : "▼") : ""}
                </th>
                <th className="col-mtime" onClick={() => toggleSort("mtime")}>
                  Modified {sortKey === "mtime" ? (sortDir === "asc" ? "▲" : "▼") : ""}
                </th>
                <th className="col-act" />
              </tr>
            </thead>
            <tbody>
              {visibleEntries.map((entry) => {
                const isSel = selected.has(entry.name);
                const isCut =
                  clipboard?.mode === "cut" &&
                  clipboard.serverId === serverId &&
                  clipboard.dirPath === currentPath &&
                  clipboard.names.includes(entry.name);
                const entryFullPath = entry.path || joinPath(currentPath, entry.name);
                const dirLike = isDirLike(entry);
                const isDropTarget = dirLike && dropTargetDir === entryFullPath;
                return (
                  <tr
                    key={entry.name}
                    draggable
                    onDragStart={(ev) => onRowDragStart(ev, entry)}
                    onDragEnd={onRowDragEnd}
                    onDragOver={(ev) => {
                      if (!dirLike) return;
                      ev.preventDefault();
                      ev.stopPropagation();
                      setDropTargetDir(entryFullPath);
                    }}
                    onDragLeave={() => {
                      if (dropTargetDir === entryFullPath) setDropTargetDir(null);
                    }}
                    onDrop={(ev) => {
                      if (!dirLike) return;
                      handleDropOnDir(ev, entryFullPath);
                    }}
                    onClick={(ev) => handleRowClick(ev, entry)}
                    onDoubleClick={() => openEntry(entry)}
                    onContextMenu={(ev) => openItemMenu(ev, entry)}
                    className={`fm-row ${isSel ? "selected" : ""} ${
                      isCut ? "cut" : ""
                    } ${isDropTarget ? "drop-target" : ""}`}
                  >
                    <td>
                      <div className="fm-name-cell">
                        <EntryIcon entry={entry} size={16} />
                        <span
                          className={`fm-name-text ${dirLike ? "fm-dir-clickable" : ""}`}
                          title={entry.name}
                          onClick={(ev) => {
                            if (dirLike) {
                              ev.stopPropagation();
                              openEntry(entry);
                            }
                          }}
                        >
                          {entry.name}
                        </span>
                        {entry.type === "link" && entry.target && (
                          <span className="fm-symlink-target" title={entry.target}>
                            → {entry.target}
                          </span>
                        )}
                      </div>
                    </td>
                    <td className="col-size">
                      {entry.type === "directory" ? "—" : formatSize(entry.size)}
                    </td>
                    <td className="col-perm">
                      {entry.owner}:{entry.group} · {entry.perms.slice(1)}
                    </td>
                    <td className="col-mtime">{formatDate(entry.mtime)}</td>
                    <td className="col-act" onClick={(ev) => ev.stopPropagation()}>
                      <button
                        type="button"
                        className="fm-row-btn"
                        title="Actions"
                        onClick={(ev) => openItemMenu(ev, entry)}
                      >
                        <MoreVerticalIcon size={14} />
                      </button>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        ) : (
          <div className="fm-grid">
            {visibleEntries.map((entry) => {
              const isSel = selected.has(entry.name);
              const entryFullPath = entry.path || joinPath(currentPath, entry.name);
              const dirLike = isDirLike(entry);
              const isDropTarget = dirLike && dropTargetDir === entryFullPath;
              return (
                <div
                  key={entry.name}
                  draggable
                  onDragStart={(ev) => onRowDragStart(ev, entry)}
                  onDragEnd={onRowDragEnd}
                  onDragOver={(ev) => {
                    if (!dirLike) return;
                    ev.preventDefault();
                    ev.stopPropagation();
                    setDropTargetDir(entryFullPath);
                  }}
                  onDragLeave={() => {
                    if (dropTargetDir === entryFullPath) setDropTargetDir(null);
                  }}
                  onDrop={(ev) => {
                    if (!dirLike) return;
                    handleDropOnDir(ev, entryFullPath);
                  }}
                  onClick={(ev) => handleRowClick(ev, entry)}
                  onDoubleClick={() => openEntry(entry)}
                  onContextMenu={(ev) => openItemMenu(ev, entry)}
                  className={`fm-card ${isSel ? "selected" : ""} ${
                    isDropTarget ? "drop-target" : ""
                  }`}
                >
                  <button
                    type="button"
                    className="fm-card-more"
                    title="Actions"
                    onClick={(ev) => openItemMenu(ev, entry)}
                  >
                    <MoreVerticalIcon size={13} />
                  </button>
                  <FileThumb
                    serverId={serverId}
                    dirPath={currentPath}
                    entry={entry}
                    iconSize={34}
                  />
                  <div
                    className={`fm-card-name ${dirLike ? "fm-dir-clickable" : ""}`}
                    title={entry.name}
                    onClick={(ev) => {
                      if (dirLike) {
                        ev.stopPropagation();
                        openEntry(entry);
                      }
                    }}
                  >
                    {entry.name}
                  </div>
                  {entry.type === "link" && entry.target && (
                    <div
                      className="fm-card-sub fm-mono"
                      title={`→ ${entry.target}`}
                      style={{
                        maxWidth: "100%",
                        overflow: "hidden",
                        textOverflow: "ellipsis",
                        whiteSpace: "nowrap"
                      }}
                    >
                      → {entry.target}
                    </div>
                  )}
                  <div className="fm-card-sub">
                    {entry.type === "directory"
                      ? "Folder"
                      : entry.type === "link" && entry.target_type === "directory"
                        ? "Link → Folder"
                        : formatSize(entry.size)}
                  </div>
                </div>
              );
            })}
          </div>
        )}

        {externalDragOver && (
          <div className="fm-drop-overlay">
            Drop files to upload to {currentPath}
          </div>
        )}
      </div>

      {/* --- Upload Progress Tray --- */}
      {uploads.length > 0 && (
        <div style={{ padding: "6px 10px", borderTop: "1px solid var(--border)" }}>
          {uploads.map((u) => (
            <div
              key={u.id}
              style={{
                display: "flex",
                alignItems: "center",
                gap: 8,
                fontSize: 12,
                marginBottom: 4
              }}
            >
              <span className="fm-mono" style={{ minWidth: 140 }}>
                {u.name}
              </span>
              <div className="fm-progress" style={{ flex: 1 }}>
                <div
                  className={`fm-progress-bar ${
                    u.status === "error" ? "error" : u.status === "done" ? "done" : "running"
                  }`}
                  style={{ width: `${u.pct}%` }}
                />
              </div>
              <span className="fm-muted" style={{ fontSize: 11 }}>
                {u.status === "error" ? u.error : `${u.pct}%`}
              </span>
            </div>
          ))}
        </div>
      )}

      {/* --- Footer Status Bar --- */}
      <div className="fm-footer">
        {(() => {
          const info = focusedEntry ?? currentDirPseudoEntry;
          return (
            <div className="fm-footer-info">
              {info ? (
                <>
                  <span className="fm-footer-title">
                    <EntryIcon entry={info} size={13} />
                    <strong className="fm-mono">{info.name || currentPath}</strong>
                  </span>
                  <span className="fm-tag">{TYPE_LABEL[info.type]}</span>
                  {info.name && info.type !== "directory" && (
                    <span className="fm-mono">{formatSize(info.size)}</span>
                  )}
                  {info.target && (
                    <span className="fm-mono fm-footer-target" title={info.target}>
                      → {info.target}
                    </span>
                  )}
                  <span className="fm-mono">
                    {info.owner}:{info.group}
                  </span>
                  <span className="fm-mono">
                    {info.perms} ({modeOctal(info.mode)})
                  </span>
                  <span>Modified {formatDate(info.mtime)}</span>
                </>
              ) : (
                <span className="fm-mono">{currentPath}</span>
              )}
            </div>
          );
        })()}
        <div className="fm-footer-meta">
          <span>
            {visibleEntries.length} item{visibleEntries.length === 1 ? "" : "s"}
            {!showHidden && (listing?.entries.length ?? 0) > visibleEntries.length
              ? ` (${(listing?.entries.length ?? 0) - visibleEntries.length} hidden)`
              : ""}
          </span>
          {selectedEntries.length > 0 && (
            <span>
              · Selected {selectedEntries.length} ({formatSize(totalSelectedBytes)})
            </span>
          )}
          {clipboard && (
            <span className="fm-footer-clip">
              <span>
                Clipboard: {clipboard.names.length} ({clipboard.mode})
              </span>
              <button
                type="button"
                className="fm-link-btn"
                onClick={() => void pasteInto(currentPath)}
              >
                Paste
              </button>
              <button
                type="button"
                className="fm-link-btn fm-danger-text"
                onClick={() => onClipboardChange(null)}
              >
                ×
              </button>
            </span>
          )}
        </div>
      </div>

      {/* --- Floating Context / Action Menu --- */}
      {contextMenu && (
        <div
          className="fm-drop-menu"
          style={{ left: contextMenu.x, top: contextMenu.y }}
          onMouseDown={(ev) => ev.stopPropagation()}
        >
          {contextMenu.kind === "item" && contextMenu.targetEntry ? (
            (() => {
              const entry = contextMenu.targetEntry;
              const multi = selected.has(entry.name) && selected.size > 1;
              const targets = multi ? selectedEntries : [entry];
              const isDir = isDirLike(entry);
              const isFile =
                entry.type === "file" ||
                (entry.type === "link" && entry.target_type === "file");
              const mediaKind = previewKind(entry);
              const entryPath = entry.path || joinPath(currentPath, entry.name);
              return (
                <>
                  {!multi && (
                    <button
                      type="button"
                      onClick={() => {
                        setContextMenu(null);
                        openEntry(entry);
                      }}
                    >
                      {isDir ? (
                        <FolderIcon size={14} />
                      ) : mediaKind ? (
                        <PlayIcon size={14} />
                      ) : (
                        <FileEditIcon size={14} />
                      )}
                      <span>
                        {isDir ? "Open" : mediaKind ? "Preview" : "View / Edit"}
                      </span>
                    </button>
                  )}
                  {!multi && isFile && mediaKind && (
                    <button
                      type="button"
                      onClick={() => {
                        setContextMenu(null);
                        setEditorEntry(entry);
                      }}
                    >
                      <FileEditIcon size={14} />
                      <span>Edit as text</span>
                    </button>
                  )}
                  {!multi && isDir && onOpenTerminal && (
                    <button
                      type="button"
                      onClick={() => {
                        setContextMenu(null);
                        onOpenTerminal(
                          serverId,
                          entry.type === "link" ? linkDestination(entry) : entryPath
                        );
                      }}
                    >
                      <SquareTerminalIcon size={14} />
                      <span>Open terminal here</span>
                    </button>
                  )}
                  <div className="fm-menu-sep" />
                  <button
                    type="button"
                    onClick={() => {
                      setContextMenu(null);
                      copySelection("copy", targets);
                    }}
                  >
                    <CopyIcon size={14} />
                    <span>Copy{multi ? ` (${targets.length})` : ""}</span>
                  </button>
                  <button
                    type="button"
                    onClick={() => {
                      setContextMenu(null);
                      copySelection("cut", targets);
                    }}
                  >
                    <ScissorsIcon size={14} />
                    <span>Cut{multi ? ` (${targets.length})` : ""}</span>
                  </button>
                  {!multi && (
                    <button
                      type="button"
                      onClick={() => {
                        setContextMenu(null);
                        void duplicateEntry(entry);
                      }}
                    >
                      <CopyPlusIcon size={14} />
                      <span>Duplicate</span>
                    </button>
                  )}
                  {split && otherPane && (
                    <>
                      <div className="fm-menu-sep" />
                      <button
                        type="button"
                        onClick={() => {
                          setContextMenu(null);
                          void transferToOtherPane("copy", targets);
                        }}
                      >
                        <FolderInputIcon size={14} />
                        <span>
                          Copy to other pane ({otherSrvName}:{otherPane.path})
                        </span>
                      </button>
                      <button
                        type="button"
                        onClick={() => {
                          setContextMenu(null);
                          void transferToOtherPane("move", targets);
                        }}
                      >
                        <FolderInputIcon size={14} />
                        <span>
                          Move to other pane ({otherSrvName}:{otherPane.path})
                        </span>
                      </button>
                    </>
                  )}
                  {matchingRemotes.length > 0 && (
                    <>
                      <div className="fm-menu-sep" />
                      {matchingRemotes.map((r) => (
                        <div key={r.id}>
                          <button
                            type="button"
                            onClick={() => {
                              setContextMenu(null);
                              void triggerRemoteTransfer(r, "upload", targets);
                            }}
                          >
                            <CloudUploadIcon size={14} />
                            <span>Sync: Upload to &quot;{r.name}&quot;</span>
                          </button>
                          <button
                            type="button"
                            onClick={() => {
                              setContextMenu(null);
                              void triggerRemoteTransfer(r, "download", targets);
                            }}
                          >
                            <CloudDownloadIcon size={14} />
                            <span>Sync: Download from &quot;{r.name}&quot;</span>
                          </button>
                        </div>
                      ))}
                    </>
                  )}
                  <div className="fm-menu-sep" />
                  {!multi && (
                    <>
                      <button
                        type="button"
                        onClick={() => {
                          setContextMenu(null);
                          setNameModal({ kind: "rename", entry });
                        }}
                      >
                        <PencilIcon size={14} />
                        <span>Rename…</span>
                      </button>
                      <button
                        type="button"
                        onClick={() => {
                          setContextMenu(null);
                          setLinkModal({
                            target: entryPath,
                            name: `${entry.name}-link`
                          });
                        }}
                      >
                        <Link2Icon size={14} />
                        <span>Create link to this…</span>
                      </button>
                      <button
                        type="button"
                        onClick={() => {
                          setContextMenu(null);
                          setPermEntry(entry);
                        }}
                      >
                        <ShieldCheckIcon size={14} />
                        <span>Edit permissions…</span>
                      </button>
                      {!isDir && (
                        <button
                          type="button"
                          onClick={() => {
                            setContextMenu(null);
                            window.location.href = api.filesDownloadUrl(serverId, entryPath);
                          }}
                        >
                          <DownloadIcon size={14} />
                          <span>Download</span>
                        </button>
                      )}
                    </>
                  )}
                  <div className="fm-menu-sep" />
                  <button
                    type="button"
                    className="fm-danger-text"
                    onClick={() => {
                      setContextMenu(null);
                      setDeleteModal(targets);
                    }}
                  >
                    <Trash2Icon size={14} />
                    <span>Delete{multi ? ` (${targets.length})` : ""}</span>
                  </button>
                </>
              );
            })()
          ) : (
            <>
              <button
                type="button"
                onClick={() => {
                  setContextMenu(null);
                  setNameModal({ kind: "mkdir" });
                }}
              >
                <FolderPlusIcon size={14} />
                <span>Create directory</span>
              </button>
              <button
                type="button"
                onClick={() => {
                  setContextMenu(null);
                  setNameModal({ kind: "create_file" });
                }}
              >
                <FilePlusIcon size={14} />
                <span>Create file</span>
              </button>
              <button
                type="button"
                onClick={() => {
                  setContextMenu(null);
                  setLinkModal({});
                }}
              >
                <Link2Icon size={14} />
                <span>Create link…</span>
              </button>
              <div className="fm-menu-sep" />
              <button
                type="button"
                disabled={!clipboard}
                onClick={() => {
                  setContextMenu(null);
                  void pasteInto(currentPath);
                }}
              >
                <ClipboardPasteIcon size={14} />
                <span>Paste{clipboard ? ` (${clipboard.names.length})` : ""}</span>
              </button>
              <div className="fm-menu-sep" />
              <button
                type="button"
                onClick={() => {
                  setContextMenu(null);
                  toggleHidden();
                }}
              >
                {showHidden ? <EyeOffIcon size={14} /> : <EyeIcon size={14} />}
                <span>{showHidden ? "Hide hidden files" : "Show hidden files"}</span>
              </button>
              <button
                type="button"
                onClick={() => {
                  setContextMenu(null);
                  if (currentDirPseudoEntry) setPermEntry(currentDirPseudoEntry);
                }}
              >
                <ShieldCheckIcon size={14} />
                <span>Edit permissions…</span>
              </button>
              {onOpenTerminal && (
                <button
                  type="button"
                  onClick={() => {
                    setContextMenu(null);
                    onOpenTerminal(serverId, currentPath);
                  }}
                >
                  <SquareTerminalIcon size={14} />
                  <span>Open in terminal</span>
                </button>
              )}
            </>
          )}
        </div>
      )}

      {/* --- Dolphin-style Drop Menu (Copy here / Move here / Link here) --- */}
      {dropPrompt && (
        <div
          className="fm-drop-menu"
          style={{
            left: Math.min(dropPrompt.x, window.innerWidth - 200),
            top: Math.min(dropPrompt.y, window.innerHeight - 170)
          }}
          onMouseDown={(ev) => ev.stopPropagation()}
        >
          <div className="fm-drop-menu-head">Drop into {dropPrompt.destDir}</div>
          <button
            type="button"
            onClick={() => {
              const p = dropPrompt;
              setDropPrompt(null);
              void pasteInto(p.destDir, p.payload, false);
            }}
          >
            <CopyIcon size={14} />
            <span>Copy here</span>
          </button>
          <button
            type="button"
            onClick={() => {
              const p = dropPrompt;
              setDropPrompt(null);
              void pasteInto(p.destDir, p.payload, true);
            }}
          >
            <FolderInputIcon size={14} />
            <span>Move here</span>
          </button>
          {dropPrompt.payload.serverId === serverId &&
            dropPrompt.payload.names.length === 1 && (
              <button
                type="button"
                onClick={async () => {
                  const p = dropPrompt;
                  setDropPrompt(null);
                  const name = p.payload.names[0];
                  try {
                    await api.filesAction(serverId, {
                      op: "symlink",
                      path: joinPath(p.destDir, name),
                      target: joinPath(p.payload.dirPath, name)
                    });
                    await loadDir(currentPath, true);
                    showToast("ok", `Linked ${name}`);
                  } catch (e) {
                    showToast("error", e instanceof Error ? e.message : "Symlink failed");
                  }
                }}
              >
                <Link2Icon size={14} />
                <span>Link here</span>
              </button>
            )}
          <button type="button" onClick={() => setDropPrompt(null)}>
            <XIcon size={14} />
            <span>Cancel</span>
          </button>
        </div>
      )}

      {/* --- Remote Sync Confirmation Modal --- */}
      {remoteScanConfirm && (
        <div
          className="fm-modal-backdrop"
          onMouseDown={(ev) => {
            if (ev.target === ev.currentTarget) setRemoteScanConfirm(null);
          }}
        >
          <div className="fm-modal fm-modal-md" role="dialog" aria-modal="true">
            <div className="fm-modal-header">
              <span className="fm-modal-title">
                {remoteScanConfirm.direction === "upload"
                  ? `Upload to "${remoteScanConfirm.remote.name}"?`
                  : `Download from "${remoteScanConfirm.remote.name}"?`}
              </span>
              <button
                type="button"
                className="fm-icon-btn"
                onClick={() => setRemoteScanConfirm(null)}
              >
                <XIcon size={15} />
              </button>
            </div>
            <div className="fm-modal-body">
              <TransferSummary
                remote={remoteScanConfirm.remote}
                direction={remoteScanConfirm.direction}
                scan={remoteScanConfirm.scan}
                servers={servers}
              />
            </div>
            <div className="fm-modal-footer">
              <button
                type="button"
                className="fm-btn"
                onClick={() => setRemoteScanConfirm(null)}
              >
                Cancel
              </button>
              <button
                type="button"
                className="fm-btn fm-btn-primary"
                onClick={async () => {
                  const c = remoteScanConfirm;
                  setRemoteScanConfirm(null);
                  try {
                    const job = await api.startTransfer(c.remote.id, c.direction, c.paths);
                    onJobStarted(job);
                  } catch (e) {
                    showToast("error", e instanceof Error ? e.message : "Transfer failed");
                  }
                }}
              >
                {remoteScanConfirm.direction === "upload" ? "Start Upload" : "Start Download"}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* --- Upload 409 Overwrite Confirmation Modal --- */}
      {uploadConflict && (
        <div className="fm-modal-backdrop">
          <div className="fm-modal fm-modal-sm" role="dialog" aria-modal="true">
            <div className="fm-modal-header">
              <span className="fm-modal-title">
                Timpa &quot;{uploadConflict.file.name}&quot;?
              </span>
            </div>
            <div className="fm-modal-body">
              <div className="fm-text">
                File dengan nama yang sama sudah ada di <code>{currentPath}</code>.
              </div>
            </div>
            <div className="fm-modal-footer">
              <button
                type="button"
                className="fm-btn"
                onClick={() => uploadConflict.resolve(false)}
              >
                Lewati
              </button>
              <button
                type="button"
                className="fm-btn fm-btn-primary"
                onClick={() => uploadConflict.resolve(true)}
              >
                Timpa
              </button>
            </div>
          </div>
        </div>
      )}

      {/* --- Modals --- */}
      {nameModal && (
        <NameDialog
          kind={nameModal.kind}
          initial={nameModal.entry?.name ?? ""}
          existing={existingNames}
          onCancel={() => setNameModal(null)}
          onSubmit={async (newName) => {
            if (nameModal.kind === "mkdir") {
              await api.filesAction(serverId, {
                op: "mkdir",
                path: joinPath(currentPath, newName)
              });
              setNameModal(null);
              await loadDir(currentPath, true);
              selectOnly(newName);
            } else if (nameModal.kind === "create_file") {
              await api.filesAction(serverId, {
                op: "write",
                path: joinPath(currentPath, newName),
                content: "",
                create: true
              });
              setNameModal(null);
              await loadDir(currentPath, true);
              selectOnly(newName);
            } else if (nameModal.kind === "rename" && nameModal.entry) {
              await api.filesAction(serverId, {
                op: "rename",
                path: nameModal.entry.path || joinPath(currentPath, nameModal.entry.name),
                new_path: joinPath(currentPath, newName)
              });
              setNameModal(null);
              await loadDir(currentPath, true);
              selectOnly(newName);
            }
          }}
        />
      )}

      {linkModal && (
        <LinkDialog
          dirPath={currentPath}
          initialTarget={linkModal.target}
          initialName={linkModal.name}
          existing={existingNames}
          onCancel={() => setLinkModal(null)}
          onSubmit={async (linkPath, target) => {
            await api.filesAction(serverId, {
              op: "symlink",
              path: linkPath,
              target
            });
            setLinkModal(null);
            await loadDir(currentPath, true);
          }}
        />
      )}

      {deleteModal && (
        <DeleteDialog
          entries={deleteModal}
          onCancel={() => setDeleteModal(null)}
          onConfirm={async () => {
            await api.filesAction(serverId, {
              op: "delete",
              paths: deleteModal.map((e) => e.path || joinPath(currentPath, e.name))
            });
            setDeleteModal(null);
            setSelected(new Set());
            setFocused(null);
            await loadDir(currentPath, false);
            showToast("ok", `Deleted ${deleteModal.length} item(s)`);
          }}
        />
      )}

      {permEntry && (
        <PermissionsDialog
          serverId={serverId}
          dirPath={currentPath}
          entry={permEntry}
          onCancel={() => setPermEntry(null)}
          onSaved={() => {
            setPermEntry(null);
            void loadDir(currentPath, true);
            showToast("ok", "Permissions updated");
          }}
        />
      )}

      {editorEntry && (
        <EditorDialog
          serverId={serverId}
          dirPath={currentPath}
          entry={editorEntry}
          onClose={() => setEditorEntry(null)}
          onSaved={() => {
            void loadDir(currentPath, true);
          }}
        />
      )}

      {viewerEntry && (
        <FileViewer
          serverId={serverId}
          dirPath={currentPath}
          entry={viewerEntry}
          siblings={previewSiblings}
          onSelect={(next) => {
            selectOnly(next.name);
            setViewerEntry(next);
          }}
          onClose={() => setViewerEntry(null)}
        />
      )}

      {remoteDialog && (
        <RemoteDialog
          localServerId={serverId}
          localPath={currentPath}
          servers={servers}
          existing={remoteDialog.existing}
          onCancel={() => setRemoteDialog(null)}
          onSaved={() => {
            setRemoteDialog(null);
            onRemotesChanged();
            showToast("ok", "Remote mapping saved");
          }}
        />
      )}

      {detailsNode}
    </div>
  );
}
