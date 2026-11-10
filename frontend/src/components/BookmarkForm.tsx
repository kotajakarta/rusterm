import { useState, type FormEvent } from "react";

import { api, errorMessage } from "../api";
import { COMMON_SERVER_PATHS } from "../layout";
import type { Bookmark, DirListing, Server } from "../types";
import { Icon } from "./Icon";
import { Modal } from "./Modal";

interface BookmarkFormProps {
  servers: Server[];
  bookmark: Bookmark | null;
  serverId: number;
  onClose: () => void;
  onSaved: (bookmark: Bookmark) => void;
}

function lastSegment(path: string): string {
  const parts = path.replace(/\/+$/, "").split("/");
  return parts[parts.length - 1] || path;
}

function parentOf(path: string): string {
  const trimmed = path.replace(/\/+$/, "");
  const idx = trimmed.lastIndexOf("/");
  return idx <= 0 ? "/" : trimmed.slice(0, idx);
}

export function BookmarkForm({ servers, bookmark, serverId: initialServerId, onClose, onSaved }: BookmarkFormProps) {
  const [serverId, setServerId] = useState(bookmark?.server_id ?? initialServerId);
  const [name, setName] = useState(bookmark?.name ?? "");
  const [path, setPath] = useState(bookmark?.path ?? "");
  const [startup, setStartup] = useState(bookmark?.startup_cmd ?? "");
  const [listing, setListing] = useState<DirListing | null>(null);
  const [browsing, setBrowsing] = useState(false);
  const [loadingDir, setLoadingDir] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const browse = async (target: string) => {
    setBrowsing(true);
    setLoadingDir(true);
    setError(null);
    try {
      setListing(await api.listDir(serverId, target || "~"));
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setLoadingDir(false);
    }
  };

  const choose = (dir: string) => {
    setPath(dir);
    if (!name.trim()) setName(lastSegment(dir));
    setBrowsing(false);
  };

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    setBusy(true);
    setError(null);
    const data = {
      server_id: serverId,
      name: name.trim() || lastSegment(path.trim()),
      path: path.trim(),
      startup_cmd: startup.trim(),
    };
    try {
      onSaved(bookmark ? await api.updateBookmark(bookmark.id, data) : await api.createBookmark(data));
    } catch (err) {
      setError(errorMessage(err));
      setBusy(false);
    }
  };

  return (
    <Modal
      title={bookmark ? "Ubah path tersimpan" : "Simpan path"}
      onClose={onClose}
      wide={browsing}
      footer={
        <>
          <span className="spacer" />
          <button type="button" className="btn" onClick={onClose}>
            Batal
          </button>
          <button type="submit" form="bookmark-form" className="btn btn-primary" disabled={busy || !path.trim()}>
            {bookmark ? "Simpan perubahan" : "Simpan path"}
          </button>
        </>
      }
    >
      <form id="bookmark-form" className="form" onSubmit={submit}>
        <label className="field">
          <span>Server</span>
          <select
            value={serverId}
            onChange={(e) => {
              setServerId(Number(e.target.value));
              setListing(null);
              setBrowsing(false);
            }}
          >
            {servers.map((s) => (
              <option key={s.id} value={s.id}>
                {s.name} ({s.username}@{s.host})
              </option>
            ))}
          </select>
        </label>

        <div className="field-row">
          <label className="field grow">
            <span>Path di server</span>
            <input
              className="mono"
              value={path}
              onChange={(e) => setPath(e.target.value)}
              placeholder="/var/www/aplikasi atau ~/proyek"
              required
              spellCheck={false}
            />
          </label>
          <button type="button" className="btn align-end" onClick={() => void browse(path)}>
            <Icon name="folder" size={14} /> Jelajahi
          </button>
        </div>

        <div className="common-paths-chips" aria-label="Path populer">
          {COMMON_SERVER_PATHS.map((preset) => (
            <button
              key={preset.path}
              type="button"
              className={`path-chip${path.trim() === preset.path ? " is-active" : ""}`}
              onClick={() => {
                setPath(preset.path);
                if (!name.trim()) setName(preset.path === "~" ? "Home" : lastSegment(preset.path));
              }}
              title={`${preset.description} (${preset.path})`}
            >
              <code>{preset.path}</code>
            </button>
          ))}
        </div>

        {browsing && (
          <div className="picker">
            <div className="picker-head">
              <button
                type="button"
                className="icon-btn"
                disabled={!listing || listing.path === "/" || loadingDir}
                onClick={() => listing && void browse(parentOf(listing.path))}
                aria-label="Naik satu folder"
                title="Naik satu folder"
              >
                <Icon name="up" size={14} />
              </button>
              <code className="picker-path">{listing?.path ?? "…"}</code>
              <button
                type="button"
                className="btn btn-small btn-primary"
                disabled={!listing || loadingDir}
                onClick={() => listing && choose(listing.path)}
              >
                Pilih folder ini
              </button>
            </div>
            <ul className="picker-list" aria-busy={loadingDir}>
              {loadingDir && <li className="picker-note">Membaca folder…</li>}
              {!loadingDir && listing?.entries.length === 0 && <li className="picker-note">Folder kosong.</li>}
              {!loadingDir &&
                listing?.entries.map((entry) => (
                  <li key={entry.path}>
                    {entry.is_dir ? (
                      <button
                        type="button"
                        className="picker-item"
                        onClick={() => void browse(entry.path)}
                        onDoubleClick={() => choose(entry.path)}
                      >
                        <Icon name="folder" size={14} /> {entry.name}
                      </button>
                    ) : (
                      <span className="picker-item is-file">{entry.name}</span>
                    )}
                  </li>
                ))}
            </ul>
          </div>
        )}

        <label className="field">
          <span>Nama</span>
          <input value={name} onChange={(e) => setName(e.target.value)} placeholder={lastSegment(path) || "api-backend"} />
        </label>

        <label className="field">
          <span>
            Perintah setelah masuk <em className="hint">opsional</em>
          </span>
          <input
            className="mono"
            value={startup}
            onChange={(e) => setStartup(e.target.value)}
            placeholder="source venv/bin/activate && git status"
            spellCheck={false}
          />
        </label>

        {error && <p className="notice bad">{error}</p>}
      </form>
    </Modal>
  );
}
