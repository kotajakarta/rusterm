import { useEffect, useState } from "react";
import { api } from "../../api";
import type {
  FileRemote,
  FileRemoteInput,
  Server,
  TransferDirection,
  TransferJob,
  TransferScan
} from "../../types";
import {
  ArrowLeftRightIcon,
  CloudDownloadIcon,
  CloudUploadIcon,
  Trash2Icon,
  XIcon
} from "./FileIcons";
import {
  DEFAULT_IGNORE,
  formatSize,
  joinPath,
  parentOf,
  parseJsonc,
  readSftpConfig
} from "./fileUtils";

async function fetchSftpConfig(serverId: number, dirPath: string) {
  try {
    const res = await api.filesRead(serverId, joinPath(dirPath, ".vscode/sftp.json"));
    return readSftpConfig(parseJsonc(res.content));
  } catch {
    return null;
  }
}

// --- Add / Edit Remote Mapping (SFTP-style) ---

interface RemoteDialogProps {
  localServerId: number;
  localPath: string;
  servers: Server[];
  existing?: FileRemote;
  onSaved: () => void;
  onCancel: () => void;
}

export function RemoteDialog({
  localServerId,
  localPath,
  servers,
  existing,
  onSaved,
  onCancel
}: RemoteDialogProps) {
  const otherServers = servers.filter((s) => s.id !== localServerId);
  const [name, setName] = useState(existing?.name ?? "production");
  const [locPath, setLocPath] = useState(existing?.local_path ?? localPath);
  const [remoteServerId, setRemoteServerId] = useState<number>(
    existing?.remote_server_id ?? (otherServers[0]?.id ?? localServerId)
  );
  const [remotePath, setRemotePath] = useState(existing?.remote_path ?? localPath);
  const [ignoreText, setIgnoreText] = useState(
    (existing?.ignore ?? DEFAULT_IGNORE).join("\n")
  );
  const [busy, setBusy] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const [sftpHint, setSftpHint] = useState<string | null>(null);
  const [locDirs, setLocDirs] = useState<string[]>([]);
  const [remDirs, setRemDirs] = useState<string[]>([]);

  useEffect(() => {
    if (!locPath.startsWith("/")) {
      setLocDirs([]);
      return;
    }
    const dir = locPath.endsWith("/") && locPath !== "/" ? locPath.slice(0, -1) : parentOf(locPath);
    const t = setTimeout(() => {
      api
        .filesList(localServerId, dir, false)
        .then((res) => {
          const dirs = res.entries
            .filter((e) => e.type === "directory" || e.target_type === "directory")
            .map((e) => joinPath(res.path, e.name));
          if (!dirs.includes(res.path)) dirs.unshift(res.path);
          setLocDirs(dirs);
        })
        .catch(() => setLocDirs([]));
    }, 200);
    return () => clearTimeout(t);
  }, [localServerId, locPath]);

  useEffect(() => {
    if (!remotePath.startsWith("/")) {
      setRemDirs([]);
      return;
    }
    const dir =
      remotePath.endsWith("/") && remotePath !== "/"
        ? remotePath.slice(0, -1)
        : parentOf(remotePath);
    const t = setTimeout(() => {
      api
        .filesList(remoteServerId, dir, false)
        .then((res) => {
          const dirs = res.entries
            .filter((e) => e.type === "directory" || e.target_type === "directory")
            .map((e) => joinPath(res.path, e.name));
          if (!dirs.includes(res.path)) dirs.unshift(res.path);
          setRemDirs(dirs);
        })
        .catch(() => setRemDirs([]));
    }, 200);
    return () => clearTimeout(t);
  }, [remoteServerId, remotePath]);

  useEffect(() => {
    if (existing) return;
    let alive = true;
    fetchSftpConfig(localServerId, locPath).then((cfg) => {
      if (!alive || !cfg) return;
      if (cfg.remotePath) setRemotePath(cfg.remotePath);
      if (cfg.ignore?.length) setIgnoreText(cfg.ignore.join("\n"));
      if (cfg.host) {
        const match = servers.find(
          (s) => s.host === cfg.host || s.name.toLowerCase() === cfg.host?.toLowerCase()
        );
        if (match) setRemoteServerId(match.id);
      }
      setSftpHint(
        `Detected .vscode/sftp.json${cfg.host ? ` (${cfg.host}:${cfg.remotePath || ""})` : ""}`
      );
    });
    return () => {
      alive = false;
    };
  }, [existing, localServerId, locPath, servers]);

  const valid =
    name.trim().length > 0 &&
    locPath.trim().startsWith("/") &&
    remotePath.trim().startsWith("/") &&
    Boolean(remoteServerId);

  const save = async () => {
    if (!valid) return;
    setBusy(true);
    setErr(null);
    const payload: FileRemoteInput = {
      name: name.trim(),
      local_server_id: localServerId,
      local_path: locPath.trim(),
      remote_server_id: remoteServerId,
      remote_path: remotePath.trim(),
      ignore: ignoreText
        .split(/\r?\n/)
        .map((l) => l.trim())
        .filter(Boolean)
    };
    try {
      if (existing) await api.updateFileRemote(existing.id, payload);
      else await api.createFileRemote(payload);
      onSaved();
    } catch (e) {
      setErr(e instanceof Error ? e.message : "Failed to save remote");
      setBusy(false);
    }
  };

  const remove = async () => {
    if (!existing) return;
    setDeleting(true);
    try {
      await api.deleteFileRemote(existing.id);
      onSaved();
    } catch (e) {
      setErr(e instanceof Error ? e.message : "Failed to delete remote");
      setDeleting(false);
    }
  };

  const localSrv = servers.find((s) => s.id === localServerId);

  return (
    <div
      className="fm-modal-backdrop"
      onMouseDown={(ev) => {
        if (ev.target === ev.currentTarget && !busy && !deleting) onCancel();
      }}
    >
      <div className="fm-modal fm-modal-md" role="dialog" aria-modal="true">
        <div className="fm-modal-header">
          <span className="fm-modal-title">
            {existing ? `Edit remote "${existing.name}"` : "Add remote mapping (SFTP sync)"}
          </span>
          <button type="button" className="fm-icon-btn" onClick={onCancel}>
            <XIcon size={15} />
          </button>
        </div>

        <div className="fm-modal-body">
          {sftpHint && <div className="fm-alert fm-alert-info">{sftpHint}</div>}

          <div className="fm-perm-grid">
            <div className="fm-field">
              <label className="fm-label">Remote name</label>
              <input
                className="fm-input"
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="production / staging"
              />
            </div>
            <div className="fm-field">
              <label className="fm-label">Local server</label>
              <input
                className="fm-input"
                disabled
                value={localSrv ? `${localSrv.name} (${localSrv.host})` : `#${localServerId}`}
              />
            </div>
          </div>

          <div className="fm-field">
            <label className="fm-label">Local root directory</label>
            <input
              className="fm-input fm-mono"
              list="fm-loc-dirs"
              value={locPath}
              onChange={(e) => setLocPath(e.target.value)}
              placeholder="/path/to/project"
            />
            <datalist id="fm-loc-dirs">
              {locDirs.map((d) => (
                <option key={d} value={d} />
              ))}
            </datalist>
          </div>

          <div className="fm-perm-grid">
            <div className="fm-field">
              <label className="fm-label">Remote server</label>
              <select
                className="fm-select"
                value={remoteServerId}
                onChange={(e) => setRemoteServerId(Number(e.target.value))}
              >
                {servers.map((s) => (
                  <option key={s.id} value={s.id}>
                    {s.name} ({s.username}@{s.host})
                  </option>
                ))}
              </select>
            </div>
            <div className="fm-field">
              <label className="fm-label">Remote root directory</label>
              <input
                className="fm-input fm-mono"
                list="fm-rem-dirs"
                value={remotePath}
                onChange={(e) => setRemotePath(e.target.value)}
                placeholder="/var/www/project"
              />
              <datalist id="fm-rem-dirs">
                {remDirs.map((d) => (
                  <option key={d} value={d} />
                ))}
              </datalist>
            </div>
          </div>

          <div className="fm-field">
            <div className="fm-label-row">
              <label className="fm-label">Ignore patterns (one glob per line)</label>
              <button
                type="button"
                className="fm-link-btn"
                onClick={() => {
                  fetchSftpConfig(localServerId, locPath).then((cfg) => {
                    if (!cfg) {
                      setErr(`No .vscode/sftp.json found in ${locPath}`);
                      return;
                    }
                    if (cfg.remotePath) setRemotePath(cfg.remotePath);
                    if (cfg.ignore?.length) setIgnoreText(cfg.ignore.join("\n"));
                    if (cfg.host) {
                      const match = servers.find(
                        (s) =>
                          s.host === cfg.host ||
                          s.name.toLowerCase() === cfg.host?.toLowerCase()
                      );
                      if (match) setRemoteServerId(match.id);
                    }
                    setSftpHint(
                      `Imported from .vscode/sftp.json${cfg.host ? ` (${cfg.host})` : ""}`
                    );
                  });
                }}
              >
                Import from .vscode/sftp.json
              </button>
            </div>
            <textarea
              className="fm-textarea fm-mono"
              rows={5}
              value={ignoreText}
              onChange={(e) => setIgnoreText(e.target.value)}
            />
          </div>

          {err && <div className="fm-alert fm-alert-error">{err}</div>}
        </div>

        <div className="fm-modal-footer">
          {existing && (
            <button
              type="button"
              className="fm-btn fm-btn-danger"
              disabled={busy || deleting}
              onClick={() => void remove()}
              style={{ marginRight: "auto" }}
            >
              <Trash2Icon size={13} />
              <span>{deleting ? "Deleting…" : "Delete"}</span>
            </button>
          )}
          <button
            type="button"
            className="fm-btn"
            disabled={busy || deleting}
            onClick={onCancel}
          >
            Cancel
          </button>
          <button
            type="button"
            className="fm-btn fm-btn-primary"
            disabled={!valid || busy || deleting}
            onClick={() => void save()}
          >
            {busy ? "Saving…" : existing ? "Save" : "Create remote"}
          </button>
        </div>
      </div>
    </div>
  );
}

// --- Transfer Confirmation + Progress Drawer ---

export function TransferSummary({
  remote,
  direction,
  scan,
  servers
}: {
  remote: FileRemote;
  direction: TransferDirection;
  scan: TransferScan;
  servers: Server[];
}) {
  const srcSrv = servers.find((s) => s.id === scan.source_server_id);
  const dstSrv = servers.find((s) => s.id === scan.dest_server_id);
  return (
    <div style={{ display: "grid", gap: 10, fontSize: 13 }}>
      <div className="fm-transfer-box">
        <div>
          <span className="fm-muted">Remote: </span>
          <strong>{remote.name}</strong>{" "}
          <span className="fm-tag">{direction === "upload" ? "Upload →" : "← Download"}</span>
        </div>
        <div className="fm-mono" style={{ fontSize: 12 }}>
          {srcSrv?.name ?? `#${scan.source_server_id}`}:{scan.source_root} →{" "}
          {dstSrv?.name ?? `#${scan.dest_server_id}`}:{scan.dest_root}
        </div>
      </div>

      <div className="fm-transfer-stats">
        <div className="fm-transfer-stat">
          <div className="fm-muted" style={{ fontSize: 11 }}>
            Files to transfer
          </div>
          <div style={{ fontSize: 18, fontWeight: 600 }}>
            {scan.files.toLocaleString()}
          </div>
        </div>
        <div className="fm-transfer-stat">
          <div className="fm-muted" style={{ fontSize: 11 }}>
            Directories
          </div>
          <div style={{ fontSize: 18, fontWeight: 600 }}>
            {scan.dirs.toLocaleString()}
          </div>
        </div>
        <div className="fm-transfer-stat">
          <div className="fm-muted" style={{ fontSize: 11 }}>
            Total size
          </div>
          <div style={{ fontSize: 18, fontWeight: 600 }}>
            {formatSize(scan.bytes)}
          </div>
        </div>
      </div>

      {scan.skipped.length > 0 && (
        <div className="fm-alert fm-alert-info">
          <div>
            <strong>Ignored ({scan.skipped.length})</strong>:{" "}
            <span className="fm-mono" style={{ fontSize: 11 }}>
              {scan.skipped.slice(0, 8).join(", ")}
              {scan.skipped.length > 8 ? ` …+${scan.skipped.length - 8} more` : ""}
            </span>
          </div>
        </div>
      )}
    </div>
  );
}

export function DirectTransferSummary({
  mode,
  scan,
  servers
}: {
  mode: "copy" | "move";
  scan: TransferScan;
  servers: Server[];
}) {
  const srcSrv = servers.find((s) => s.id === scan.source_server_id);
  const dstSrv = servers.find((s) => s.id === scan.dest_server_id);
  return (
    <div style={{ display: "grid", gap: 10, fontSize: 13 }}>
      <div className="fm-transfer-box">
        <div>
          <span className="fm-tag">
            {mode === "move" ? "Move across servers →" : "Copy across servers →"}
          </span>
          {mode === "move" && (
            <span className="fm-muted" style={{ fontSize: 11, marginLeft: 6 }}>
              Source items will be deleted after transfer succeeds
            </span>
          )}
        </div>
        <div className="fm-mono" style={{ fontSize: 12 }}>
          {srcSrv?.name ?? `#${scan.source_server_id}`}:{scan.source_root} →{" "}
          {dstSrv?.name ?? `#${scan.dest_server_id}`}:{scan.dest_root}
        </div>
        <div className="fm-mono fm-muted" style={{ fontSize: 11 }}>
          Items: {scan.rels.slice(0, 6).join(", ")}
          {scan.rels.length > 6 ? ` …+${scan.rels.length - 6} more` : ""}
        </div>
      </div>

      <div className="fm-transfer-stats">
        <div className="fm-transfer-stat">
          <div className="fm-muted" style={{ fontSize: 11 }}>
            Files
          </div>
          <div style={{ fontSize: 18, fontWeight: 600 }}>
            {scan.files.toLocaleString()}
          </div>
        </div>
        <div className="fm-transfer-stat">
          <div className="fm-muted" style={{ fontSize: 11 }}>
            Directories
          </div>
          <div style={{ fontSize: 18, fontWeight: 600 }}>
            {scan.dirs.toLocaleString()}
          </div>
        </div>
        <div className="fm-transfer-stat">
          <div className="fm-muted" style={{ fontSize: 11 }}>
            Total size
          </div>
          <div style={{ fontSize: 18, fontWeight: 600 }}>
            {formatSize(scan.bytes)}
          </div>
        </div>
      </div>

      {scan.skipped.length > 0 && (
        <div className="fm-alert fm-alert-info">
          <div>
            <strong>Ignored ({scan.skipped.length})</strong>:{" "}
            <span className="fm-mono" style={{ fontSize: 11 }}>
              {scan.skipped.slice(0, 8).join(", ")}
              {scan.skipped.length > 8 ? ` …+${scan.skipped.length - 8} more` : ""}
            </span>
          </div>
        </div>
      )}
    </div>
  );
}

export function DirectTransferModal({
  mode,
  sourceServerId,
  destServerId,
  sourceDir,
  destDir,
  paths,
  initialScan,
  servers,
  onStart,
  onCancel
}: {
  mode: "copy" | "move";
  sourceServerId: number;
  destServerId: number;
  sourceDir: string;
  destDir: string;
  paths: string[];
  initialScan: TransferScan;
  servers: Server[];
  onStart: (useGitignore: boolean, scan: TransferScan) => Promise<void>;
  onCancel: () => void;
}) {
  const [useGitignore, setUseGitignore] = useState(true);
  const [scan, setScan] = useState<TransferScan>(initialScan);
  const [rescanning, setRescanning] = useState(false);
  const [starting, setStarting] = useState(false);

  const toggleGitignore = async (checked: boolean) => {
    setUseGitignore(checked);
    setRescanning(true);
    try {
      const next = await api.directTransferScan({
        source_server_id: sourceServerId,
        dest_server_id: destServerId,
        source_dir: sourceDir,
        dest_dir: destDir,
        paths,
        use_gitignore: checked,
        ignore: checked ? undefined : []
      });
      setScan(next);
    } catch {
      // keep previous scan on transient error
    } finally {
      setRescanning(false);
    }
  };

  const confirm = async () => {
    setStarting(true);
    try {
      await onStart(useGitignore, scan);
    } finally {
      setStarting(false);
    }
  };

  return (
    <div
      className="fm-modal-backdrop"
      onMouseDown={(ev) => {
        if (ev.target === ev.currentTarget && !starting) onCancel();
      }}
    >
      <div className="fm-modal fm-modal-md" role="dialog" aria-modal="true">
        <div className="fm-modal-header">
          <span className="fm-modal-title">
            {mode === "move" ? "Move to other server?" : "Copy to other server?"}
          </span>
          <button type="button" className="fm-icon-btn" onClick={onCancel} disabled={starting}>
            <XIcon size={15} />
          </button>
        </div>
        <div className="fm-modal-body">
          <DirectTransferSummary mode={mode} scan={scan} servers={servers} />
          <label className="fm-checkbox" style={{ marginTop: 8 }}>
            <input
              type="checkbox"
              checked={useGitignore}
              disabled={rescanning || starting}
              onChange={(e) => void toggleGitignore(e.target.checked)}
            />
            <span>
              Hormati <code>.gitignore</code> &amp; abaikan <code>.git</code>,{" "}
              <code>node_modules</code>, <code>__pycache__</code>, <code>.env</code>
            </span>
          </label>
        </div>
        <div className="fm-modal-footer">
          <button type="button" className="fm-btn" onClick={onCancel} disabled={starting}>
            Cancel
          </button>
          <button
            type="button"
            className="fm-btn fm-btn-primary"
            disabled={rescanning || starting}
            onClick={() => void confirm()}
          >
            {starting ? "Starting…" : mode === "move" ? "Start Move" : "Start Copy"}
          </button>
        </div>
      </div>
    </div>
  );
}

export function TransferList({
  jobs,
  onCancel,
  onDismiss
}: {
  jobs: TransferJob[];
  onCancel: (id: string) => void;
  onDismiss: (id: string) => void;
}) {
  if (jobs.length === 0) return null;
  return (
    <div className="fm-transfer-tray">
      {jobs.map((j) => {
        const pct =
          j.state === "done"
            ? 100
            : j.total_bytes > 0
              ? Math.min(99, Math.round((j.sent_bytes / j.total_bytes) * 100))
              : 50;
        const Icon =
          j.direction === "upload"
            ? CloudUploadIcon
            : j.direction === "download"
              ? CloudDownloadIcon
              : ArrowLeftRightIcon;
        const dirLabel =
          j.direction === "upload"
            ? "Upload"
            : j.direction === "download"
              ? "Download"
              : j.direction === "move"
                ? "Move"
                : "Copy";
        return (
          <div key={j.id} className="fm-transfer-card">
            <div className="fm-transfer-card-head">
              <span style={{ display: "inline-flex", alignItems: "center", gap: 6 }}>
                <Icon size={13} />
                <strong>{j.remote_name}</strong>
                <span className="fm-tag">{dirLabel}</span>
              </span>
              {j.state === "running" ? (
                <button
                  type="button"
                  className="fm-btn fm-btn-danger fm-btn-xs"
                  onClick={() => onCancel(j.id)}
                >
                  Cancel
                </button>
              ) : (
                <button
                  type="button"
                  className="fm-icon-btn"
                  onClick={() => onDismiss(j.id)}
                >
                  <XIcon size={13} />
                </button>
              )}
            </div>
            <div className="fm-progress">
              <div
                className={`fm-progress-bar ${
                  j.state === "error" ? "error" : j.state === "done" ? "done" : "running"
                }`}
                style={{ width: `${pct}%` }}
              />
            </div>
            <div className="fm-transfer-card-foot">
              <span>
                {formatSize(j.sent_bytes)} / {formatSize(j.total_bytes)} (
                {j.total_files.toLocaleString()} files)
              </span>
              <span>
                {j.state === "done" && j.result
                  ? `Done · ${j.result.files} files`
                  : j.state === "error"
                    ? j.error || "Failed"
                    : j.state === "cancelled"
                      ? "Cancelled"
                      : "Streaming…"}
              </span>
            </div>
          </div>
        );
      })}
    </div>
  );
}
