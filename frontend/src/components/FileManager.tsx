import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api, downloadFileUrl, errorMessage } from "../api";
import {
  bitsToMode,
  filterAndSortEntries,
  formatFileTime,
  joinRemotePath,
  modeToRwx,
  parentDirPath,
  parseModeBits,
  splitBreadcrumbs,
  type PermissionBits,
  type SortField,
} from "../fileManager";
import { formatBytes } from "../format";
import { COMMON_SERVER_PATHS } from "../layout";
import type { DirEntry, Server } from "../types";
import { Icon } from "./Icon";

interface FileManagerProps {
  servers: Server[];
  initialServerId: number;
  initialPath?: string;
  onClose: () => void;
  onOpenTerminalAt: (serverId: number, path: string) => void;
  onBookmarkSaved?: () => void;
}

type SubModal =
  | { kind: "editor"; path: string; content: string; originalContent: string; mode: string; size: number }
  | { kind: "mkdir"; name: string }
  | { kind: "newfile"; name: string }
  | { kind: "rename"; entry: DirEntry; newPath: string }
  | { kind: "chmod"; entry: DirEntry; mode: string; bits: PermissionBits }
  | { kind: "delete"; entry: DirEntry }
  | null;

export function FileManager({
  servers,
  initialServerId,
  initialPath = "~",
  onClose,
  onOpenTerminalAt,
  onBookmarkSaved,
}: FileManagerProps) {
  const [serverId, setServerId] = useState<number>(initialServerId);
  const [currentPath, setCurrentPath] = useState<string>(initialPath || "~");
  const [pathInput, setPathInput] = useState<string>(initialPath || "~");
  const [entries, setEntries] = useState<DirEntry[]>([]);
  const [loading, setLoading] = useState<boolean>(false);
  const [busyText, setBusyText] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const [query, setQuery] = useState<string>("");
  const [showHidden, setShowHidden] = useState<boolean>(true);
  const [sortBy, setSortBy] = useState<SortField>("name");
  const [sortAsc, setSortAsc] = useState<boolean>(true);

  const [subModal, setSubModal] = useState<SubModal>(null);
  const uploadInputRef = useRef<HTMLInputElement>(null);

  const loadDirectory = useCallback(
    async (targetServerId: number, targetPath: string) => {
      setLoading(true);
      setError(null);
      try {
        const res = await api.listDir(targetServerId, targetPath);
        setCurrentPath(res.path);
        setPathInput(res.path);
        setEntries(res.entries);
      } catch (err) {
        setError(errorMessage(err));
      } finally {
        setLoading(false);
      }
    },
    [],
  );

  useEffect(() => {
    void loadDirectory(serverId, initialPath || "~");
  }, [serverId, initialPath, loadDirectory]);

  useEffect(() => {
    if (!notice) return;
    const t = window.setTimeout(() => setNotice(null), 3500);
    return () => window.clearTimeout(t);
  }, [notice]);

  const breadcrumbs = useMemo(() => splitBreadcrumbs(currentPath), [currentPath]);

  const visibleEntries = useMemo(
    () =>
      filterAndSortEntries(entries, {
        query,
        showHidden,
        sortBy,
        sortAsc,
      }),
    [entries, query, showHidden, sortBy, sortAsc],
  );

  const navigateTo = (path: string) => {
    void loadDirectory(serverId, path);
  };

  const openEditor = async (path: string) => {
    setBusyText("Membaca file…");
    setError(null);
    try {
      const file = await api.readFile(serverId, path);
      setSubModal({
        kind: "editor",
        path: file.path,
        content: file.content,
        originalContent: file.content,
        mode: file.mode,
        size: file.size,
      });
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusyText(null);
    }
  };

  const handleSaveEditor = async () => {
    if (!subModal || subModal.kind !== "editor") return;
    setBusyText("Menyimpan file…");
    setError(null);
    try {
      await api.writeFile(serverId, subModal.path, subModal.content);
      setNotice(`Disimpan: ${subModal.path}`);
      setSubModal(null);
      void loadDirectory(serverId, currentPath);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusyText(null);
    }
  };

  const handleCreateDir = async () => {
    if (!subModal || subModal.kind !== "mkdir") return;
    const name = subModal.name.trim();
    if (!name) return;
    const fullPath = name.startsWith("/") ? name : joinRemotePath(currentPath, name);
    setBusyText("Membuat folder…");
    setError(null);
    try {
      await api.mkdir(serverId, fullPath);
      setSubModal(null);
      setNotice(`Folder dibuat: ${fullPath}`);
      void loadDirectory(serverId, currentPath);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusyText(null);
    }
  };

  const handleCreateNewFile = async () => {
    if (!subModal || subModal.kind !== "newfile") return;
    const name = subModal.name.trim();
    if (!name) return;
    const fullPath = name.startsWith("/") ? name : joinRemotePath(currentPath, name);
    setSubModal({
      kind: "editor",
      path: fullPath,
      content: "",
      originalContent: "",
      mode: "0644",
      size: 0,
    });
  };

  const handleRename = async () => {
    if (!subModal || subModal.kind !== "rename") return;
    const newPath = subModal.newPath.trim();
    if (!newPath || newPath === subModal.entry.path) {
      setSubModal(null);
      return;
    }
    setBusyText("Memindahkan / mengubah nama…");
    setError(null);
    try {
      await api.renameFile(serverId, subModal.entry.path, newPath);
      setSubModal(null);
      setNotice(`Dipindahkan ke ${newPath}`);
      void loadDirectory(serverId, currentPath);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusyText(null);
    }
  };

  const handleChmod = async () => {
    if (!subModal || subModal.kind !== "chmod") return;
    setBusyText("Mengubah izin…");
    setError(null);
    try {
      const res = await api.chmodFile(serverId, subModal.entry.path, subModal.mode);
      setSubModal(null);
      setNotice(res.message);
      void loadDirectory(serverId, currentPath);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusyText(null);
    }
  };

  const handleDelete = async () => {
    if (!subModal || subModal.kind !== "delete") return;
    setBusyText("Menghapus…");
    setError(null);
    try {
      await api.deleteFile(serverId, subModal.entry.path);
      setNotice(`Dihapus: ${subModal.entry.name}`);
      setSubModal(null);
      void loadDirectory(serverId, currentPath);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusyText(null);
    }
  };

  const handleUploadFiles = async (files: File[]) => {
    if (files.length === 0) return;
    setBusyText(`Mengunggah ${files.length} file…`);
    setError(null);
    try {
      for (const file of files) {
        const dest = joinRemotePath(currentPath, file.name);
        await api.uploadFile(serverId, dest, file);
      }
      setNotice(`${files.length} file berhasil diunggah ke ${currentPath}`);
      void loadDirectory(serverId, currentPath);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusyText(null);
    }
  };

  const handleSaveAsBookmark = async () => {
    const folderName = currentPath === "/" ? "Root (/)" : currentPath.split("/").filter(Boolean).pop() || currentPath;
    setBusyText("Menyimpan path…");
    setError(null);
    try {
      await api.createBookmark({
        server_id: serverId,
        name: folderName,
        path: currentPath,
        startup_cmd: "",
      });
      setNotice(`Path disimpan ke favorit: ${currentPath}`);
      onBookmarkSaved?.();
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusyText(null);
    }
  };

  const updateChmodBit = (
    role: "owner" | "group" | "other",
    idx: 0 | 1 | 2,
    checked: boolean,
  ) => {
    if (!subModal || subModal.kind !== "chmod") return;
    const nextBits: PermissionBits = {
      owner: [...subModal.bits.owner] as [boolean, boolean, boolean],
      group: [...subModal.bits.group] as [boolean, boolean, boolean],
      other: [...subModal.bits.other] as [boolean, boolean, boolean],
    };
    nextBits[role][idx] = checked;
    setSubModal({
      ...subModal,
      bits: nextBits,
      mode: bitsToMode(nextBits),
    });
  };

  return (
    <div className="backdrop file-manager-backdrop" onClick={onClose}>
      <div
        className="modal file-manager-modal"
        role="dialog"
        aria-modal="true"
        aria-label="Manajemen File Server (Cockpit)"
        onClick={(e) => e.stopPropagation()}
      >
        <header className="modal-head fm-header">
          <div className="fm-title-group">
            <span className="brand-mark" aria-hidden="true">
              <Icon name="folder" size={16} />
            </span>
            <h2>File Manager (Cockpit)</h2>
            {servers.length > 1 && (
              <select
                className="fm-server-select"
                value={serverId}
                onChange={(e) => setServerId(Number(e.target.value))}
                aria-label="Pilih server"
              >
                {servers.map((srv) => (
                  <option key={srv.id} value={srv.id}>
                    {srv.name} ({srv.username}@{srv.host})
                  </option>
                ))}
              </select>
            )}
          </div>
          <div className="fm-header-actions">
            <button
              type="button"
              className="btn btn-small btn-primary"
              onClick={() => onOpenTerminalAt(serverId, currentPath)}
              title="Buka terminal langsung di folder ini"
            >
              <Icon name="terminal" size={13} /> Terminal di Sini
            </button>
            <button type="button" className="icon-btn" onClick={onClose} aria-label="Tutup">
              <Icon name="close" size={16} />
            </button>
          </div>
        </header>

        <div className="fm-body">
          {/* Path & Breadcrumbs Bar */}
          <div className="fm-nav-bar">
            <button
              type="button"
              className="btn btn-small"
              onClick={() => navigateTo(parentDirPath(currentPath))}
              disabled={loading || currentPath === "/"}
              title="Naik satu folder"
            >
              <Icon name="up" size={13} /> Naik
            </button>
            <div className="fm-breadcrumbs" aria-label="Lokasi folder">
              {breadcrumbs.map((seg, i) => (
                <span key={seg.path} className="fm-crumb-wrap">
                  {i > 0 && <span className="fm-crumb-sep">/</span>}
                  <button
                    type="button"
                    className={`fm-crumb${i === breadcrumbs.length - 1 ? " is-current" : ""}`}
                    onClick={() => navigateTo(seg.path)}
                  >
                    {seg.label}
                  </button>
                </span>
              ))}
            </div>
            <form
              className="fm-path-form"
              onSubmit={(e) => {
                e.preventDefault();
                navigateTo(pathInput.trim() || "/");
              }}
            >
              <input
                type="text"
                className="fm-path-input"
                value={pathInput}
                onChange={(e) => setPathInput(e.target.value)}
                placeholder="/var/www/html"
                aria-label="Ketik path tujuan"
              />
              <button type="submit" className="btn btn-small" disabled={loading}>
                Buka
              </button>
            </form>
          </div>

          {/* Quick Server Path Chips */}
          <div className="fm-preset-strip">
            {COMMON_SERVER_PATHS.map((preset) => (
              <button
                key={preset.path}
                type="button"
                className={`preset-chip${currentPath === preset.path ? " is-active" : ""}`}
                onClick={() => navigateTo(preset.path)}
              >
                <code>{preset.path}</code>
              </button>
            ))}
          </div>

          {/* Action & Filter Toolbar */}
          <div className="fm-toolbar">
            <div className="fm-search-wrap">
              <Icon name="search" size={14} />
              <input
                type="search"
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                placeholder="Filter file/folder…"
                aria-label="Filter nama file"
              />
            </div>

            <div className="fm-sort-group">
              <select
                value={sortBy}
                onChange={(e) => setSortBy(e.target.value as SortField)}
                aria-label="Urutkan berdasarkan"
              >
                <option value="name">Nama</option>
                <option value="size">Ukuran</option>
                <option value="mtime">Waktu Ubah</option>
              </select>
              <button
                type="button"
                className="btn btn-small"
                onClick={() => setSortAsc((v) => !v)}
                title={sortAsc ? "Urutan Naik (A-Z / Kecil-Besar)" : "Urutan Turun (Z-A / Besar-Kecil)"}
              >
                {sortAsc ? "↑" : "↓"}
              </button>
              <label className="fm-hidden-toggle">
                <input
                  type="checkbox"
                  checked={showHidden}
                  onChange={(e) => setShowHidden(e.target.checked)}
                />
                <span>.dotfile</span>
              </label>
            </div>

            <div className="fm-actions-group">
              <button
                type="button"
                className="btn btn-small"
                onClick={() => setSubModal({ kind: "mkdir", name: "" })}
              >
                <Icon name="plus" size={12} /> Folder
              </button>
              <button
                type="button"
                className="btn btn-small"
                onClick={() => setSubModal({ kind: "newfile", name: "" })}
              >
                <Icon name="file" size={12} /> File Baru
              </button>
              <button
                type="button"
                className="btn btn-small"
                onClick={() => uploadInputRef.current?.click()}
              >
                <Icon name="upload" size={12} /> Unggah
              </button>
              <button
                type="button"
                className="btn btn-small"
                onClick={() => void handleSaveAsBookmark()}
                title="Simpan folder ini ke daftar Path Tersimpan"
              >
                <Icon name="jump" size={12} /> Simpan Path
              </button>
              <button
                type="button"
                className="btn btn-small"
                onClick={() => navigateTo(currentPath)}
                disabled={loading}
                title="Muat ulang daftar file"
              >
                <Icon name="refresh" size={12} />
              </button>
            </div>
          </div>

          {error && <p className="notice bad small">{error}</p>}
          {notice && <p className="notice ok small">{notice}</p>}
          {busyText && <p className="notice small">{busyText}</p>}

          {/* Inline Sub-Modals for Mkdir / NewFile / Rename / Chmod / Delete */}
          {subModal && subModal.kind === "mkdir" && (
            <form
              className="fm-inline-dialog"
              onSubmit={(e) => {
                e.preventDefault();
                void handleCreateDir();
              }}
            >
              <strong>Buat Folder Baru di {currentPath}:</strong>
              <input
                type="text"
                value={subModal.name}
                onChange={(e) => setSubModal({ kind: "mkdir", name: e.target.value })}
                placeholder="nama-folder"
                autoFocus
              />
              <button type="submit" className="btn btn-small btn-primary">
                Buat
              </button>
              <button type="button" className="btn btn-small" onClick={() => setSubModal(null)}>
                Batal
              </button>
            </form>
          )}

          {subModal && subModal.kind === "newfile" && (
            <form
              className="fm-inline-dialog"
              onSubmit={(e) => {
                e.preventDefault();
                void handleCreateNewFile();
              }}
            >
              <strong>Buat File Teks Baru di {currentPath}:</strong>
              <input
                type="text"
                value={subModal.name}
                onChange={(e) => setSubModal({ kind: "newfile", name: e.target.value })}
                placeholder=".env atau config.yml"
                autoFocus
              />
              <button type="submit" className="btn btn-small btn-primary">
                Buka Editor
              </button>
              <button type="button" className="btn btn-small" onClick={() => setSubModal(null)}>
                Batal
              </button>
            </form>
          )}

          {subModal && subModal.kind === "rename" && (
            <form
              className="fm-inline-dialog"
              onSubmit={(e) => {
                e.preventDefault();
                void handleRename();
              }}
            >
              <strong>Ganti Nama / Pindahkan ({subModal.entry.name}):</strong>
              <input
                type="text"
                value={subModal.newPath}
                onChange={(e) => setSubModal({ ...subModal, newPath: e.target.value })}
                autoFocus
              />
              <button type="submit" className="btn btn-small btn-primary">
                Simpan
              </button>
              <button type="button" className="btn btn-small" onClick={() => setSubModal(null)}>
                Batal
              </button>
            </form>
          )}

          {subModal && subModal.kind === "chmod" && (
            <form
              className="fm-inline-dialog fm-chmod-dialog"
              onSubmit={(e) => {
                e.preventDefault();
                void handleChmod();
              }}
            >
              <div className="fm-chmod-head">
                <strong>Izin Akses Cockpit (`chmod`) — {subModal.entry.name}</strong>
                <input
                  type="text"
                  className="fm-octal-input"
                  value={subModal.mode}
                  onChange={(e) => {
                    const val = e.target.value;
                    setSubModal({
                      ...subModal,
                      mode: val,
                      bits: parseModeBits(val),
                    });
                  }}
                  maxLength={4}
                  aria-label="Mode oktal"
                />
                <code>{modeToRwx(subModal.mode, subModal.entry.is_dir)}</code>
              </div>
              <div className="fm-chmod-grid">
                {(["owner", "group", "other"] as const).map((role) => (
                  <div key={role} className="fm-chmod-col">
                    <span className="fm-chmod-role">
                      {role === "owner" ? "Pemilik (Owner)" : role === "group" ? "Grup (Group)" : "Lainnya (Others)"}
                    </span>
                    <label>
                      <input
                        type="checkbox"
                        checked={subModal.bits[role][0]}
                        onChange={(e) => updateChmodBit(role, 0, e.target.checked)}
                      />
                      Baca (r)
                    </label>
                    <label>
                      <input
                        type="checkbox"
                        checked={subModal.bits[role][1]}
                        onChange={(e) => updateChmodBit(role, 1, e.target.checked)}
                      />
                      Tulis (w)
                    </label>
                    <label>
                      <input
                        type="checkbox"
                        checked={subModal.bits[role][2]}
                        onChange={(e) => updateChmodBit(role, 2, e.target.checked)}
                      />
                      Eksekusi (x)
                    </label>
                  </div>
                ))}
              </div>
              <div className="fm-inline-actions">
                <button type="submit" className="btn btn-small btn-primary">
                  Terapkan Izin
                </button>
                <button type="button" className="btn btn-small" onClick={() => setSubModal(null)}>
                  Batal
                </button>
              </div>
            </form>
          )}

          {subModal && subModal.kind === "delete" && (
            <div className="fm-inline-dialog fm-delete-dialog">
              <strong>
                Hapus {subModal.entry.is_dir ? "folder beserta isinya" : "file"} <code>{subModal.entry.path}</code>?
              </strong>
              <div className="fm-inline-actions">
                <button type="button" className="btn btn-small btn-danger" onClick={() => void handleDelete()}>
                  Ya, Hapus Permanen
                </button>
                <button type="button" className="btn btn-small" onClick={() => setSubModal(null)}>
                  Batal
                </button>
              </div>
            </div>
          )}

          {/* Cockpit File Editor View OR File Table */}
          {subModal && subModal.kind === "editor" ? (
            <div className="fm-editor">
              <div className="fm-editor-bar">
                <code className="fm-editor-path">{subModal.path}</code>
                <span className="fm-editor-meta">
                  {formatBytes(new Blob([subModal.content]).size)} • {subModal.mode}
                </span>
                <span className="spacer" />
                <button
                  type="button"
                  className="btn btn-small btn-primary"
                  onClick={() => void handleSaveEditor()}
                  disabled={busyText !== null}
                >
                  Simpan Perubahan
                </button>
                <button type="button" className="btn btn-small" onClick={() => setSubModal(null)}>
                  Tutup Editor
                </button>
              </div>
              <textarea
                className="fm-editor-textarea"
                value={subModal.content}
                onChange={(e) => setSubModal({ ...subModal, content: e.target.value })}
                spellCheck={false}
                autoCapitalize="off"
                autoComplete="off"
              />
            </div>
          ) : (
            <div className="fm-list-wrap">
              {loading ? (
                <p className="muted small fm-empty">Memuat isi direktori {currentPath}…</p>
              ) : visibleEntries.length === 0 ? (
                <p className="muted small fm-empty">Tidak ada file atau folder yang cocok.</p>
              ) : (
                <div className="fm-table" role="table" aria-label="Daftar file dan folder">
                  <div className="fm-row fm-row-head" role="row">
                    <span className="fm-col-name">Nama</span>
                    <span className="fm-col-perm">Izin</span>
                    <span className="fm-col-size">Ukuran</span>
                    <span className="fm-col-time">Diubah</span>
                    <span className="fm-col-actions">Aksi</span>
                  </div>
                  {visibleEntries.map((entry) => (
                    <div
                      key={entry.path}
                      className={`fm-row${entry.is_dir ? " is-dir" : ""}`}
                      role="row"
                    >
                      <button
                        type="button"
                        className="fm-entry-main"
                        onClick={() => (entry.is_dir ? navigateTo(entry.path) : void openEditor(entry.path))}
                      >
                        <Icon name={entry.is_dir ? "folder" : "file"} size={15} />
                        <span className="fm-entry-name">{entry.name}</span>
                        {entry.is_symlink && <span className="fm-symlink-badge">symlink</span>}
                      </button>

                      <span className="fm-col-perm">
                        <code title={entry.mode || ""}>{modeToRwx(entry.mode ?? "", entry.is_dir)}</code>
                      </span>

                      <span className="fm-col-size">
                        {entry.is_dir ? "Folder" : formatBytes(entry.size ?? 0)}
                      </span>

                      <span className="fm-col-time">{formatFileTime(entry.mtime)}</span>

                      <div className="fm-col-actions">
                        {entry.is_dir ? (
                          <button
                            type="button"
                            className="icon-btn"
                            onClick={() => onOpenTerminalAt(serverId, entry.path)}
                            title="Buka terminal di folder ini"
                          >
                            <Icon name="terminal" size={14} />
                          </button>
                        ) : (
                          <>
                            <button
                              type="button"
                              className="icon-btn"
                              onClick={() => void openEditor(entry.path)}
                              title="Edit file teks"
                            >
                              <Icon name="edit" size={14} />
                            </button>
                            <a
                              className="icon-btn"
                              href={downloadFileUrl(serverId, entry.path)}
                              download={entry.name}
                              title="Unduh file"
                            >
                              <Icon name="download" size={14} />
                            </a>
                          </>
                        )}
                        <button
                          type="button"
                          className="icon-btn"
                          onClick={() =>
                            setSubModal({
                              kind: "chmod",
                              entry,
                              mode: entry.mode || (entry.is_dir ? "0755" : "0644"),
                              bits: parseModeBits(entry.mode || (entry.is_dir ? "0755" : "0644")),
                            })
                          }
                          title="Ubah izin akses (chmod)"
                        >
                          <Icon name="sliders" size={14} />
                        </button>
                        <button
                          type="button"
                          className="icon-btn"
                          onClick={() => setSubModal({ kind: "rename", entry, newPath: entry.path })}
                          title="Ganti nama / pindahkan"
                        >
                          <Icon name="jump" size={14} />
                        </button>
                        <button
                          type="button"
                          className="icon-btn danger"
                          onClick={() => setSubModal({ kind: "delete", entry })}
                          title="Hapus"
                        >
                          <Icon name="trash" size={14} />
                        </button>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </div>
          )}
        </div>

        <input
          ref={uploadInputRef}
          type="file"
          multiple
          className="sr-only"
          onChange={(e) => {
            const files = e.target.files ? Array.from(e.target.files) : [];
            e.target.value = "";
            if (files.length > 0) void handleUploadFiles(files);
          }}
        />
      </div>
    </div>
  );
}
