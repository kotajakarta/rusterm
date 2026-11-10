import { useEffect, useRef, useState } from "react";
import { api } from "../../api";
import type { FileActionBody, FileEntry, FilePrincipalsOut } from "../../types";
import { XIcon } from "./FileIcons";
import {
  type Access,
  accessOf,
  buildMode,
  formatDate,
  formatSize,
  isDirLike,
  joinPath,
  TYPE_LABEL
} from "./fileUtils";

// --- Create directory / Create file / Rename ---

export type NameModalKind = "mkdir" | "create_file" | "rename";

interface NameDialogProps {
  kind: NameModalKind;
  initial?: string;
  existing: Set<string>;
  onSubmit: (name: string) => Promise<void>;
  onCancel: () => void;
}

const NAME_TITLES: Record<NameModalKind, string> = {
  mkdir: "Create directory",
  create_file: "Create file",
  rename: "Rename"
};

const NAME_LABELS: Record<NameModalKind, string> = {
  mkdir: "Directory name",
  create_file: "File name",
  rename: "New name"
};

const NAME_OK: Record<NameModalKind, string> = {
  mkdir: "Create",
  create_file: "Create",
  rename: "Rename"
};

export function NameDialog({ kind, initial = "", existing, onSubmit, onCancel }: NameDialogProps) {
  const [name, setName] = useState(initial);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    const t = setTimeout(() => {
      const el = inputRef.current;
      if (!el) return;
      el.focus();
      if (kind === "rename" && initial) {
        const dot = initial.lastIndexOf(".");
        el.setSelectionRange(0, dot > 0 ? dot : initial.length);
      } else {
        el.select();
      }
    }, 30);
    return () => clearTimeout(t);
  }, [kind, initial]);

  const trimmed = name.trim();
  let validationError: string | null = null;
  if (!trimmed) validationError = "Name cannot be empty";
  else if (trimmed === "." || trimmed === "..") validationError = "Reserved name";
  else if (trimmed.includes("/")) validationError = "Name cannot contain '/'";
  else if (trimmed !== initial && existing.has(trimmed)) {
    validationError = `"${trimmed}" already exists in this directory`;
  }

  const submit = async () => {
    if (validationError || (kind === "rename" && trimmed === initial)) return;
    setBusy(true);
    setErr(null);
    try {
      await onSubmit(trimmed);
    } catch (e) {
      setErr(e instanceof Error ? e.message : "Operation failed");
      setBusy(false);
    }
  };

  return (
    <div
      className="fm-modal-backdrop"
      onMouseDown={(ev) => {
        if (ev.target === ev.currentTarget && !busy) onCancel();
      }}
    >
      <div className="fm-modal fm-modal-sm" role="dialog" aria-modal="true">
        <div className="fm-modal-header">
          <span className="fm-modal-title">{NAME_TITLES[kind]}</span>
          <button type="button" className="fm-icon-btn" onClick={onCancel} disabled={busy}>
            <XIcon size={15} />
          </button>
        </div>
        <div className="fm-modal-body">
          <div className="fm-field">
            <label className="fm-label">{NAME_LABELS[kind]}</label>
            <input
              ref={inputRef}
              className="fm-input"
              value={name}
              onChange={(e) => {
                setName(e.target.value);
                setErr(null);
              }}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  e.preventDefault();
                  void submit();
                } else if (e.key === "Escape") {
                  e.preventDefault();
                  onCancel();
                }
              }}
              placeholder={kind === "mkdir" ? "new-folder" : "filename.txt"}
            />
          </div>
          {(err || (name !== "" && validationError)) && (
            <div className="fm-alert fm-alert-error">{err || validationError}</div>
          )}
        </div>
        <div className="fm-modal-footer">
          <button type="button" className="fm-btn" onClick={onCancel} disabled={busy}>
            Cancel
          </button>
          <button
            type="button"
            className="fm-btn fm-btn-primary"
            onClick={() => void submit()}
            disabled={busy || Boolean(validationError) || (kind === "rename" && trimmed === initial)}
          >
            {busy ? "Working…" : NAME_OK[kind]}
          </button>
        </div>
      </div>
    </div>
  );
}

// --- Create Symbolic Link ---

interface LinkDialogProps {
  dirPath: string;
  initialTarget?: string;
  initialName?: string;
  existing: Set<string>;
  onSubmit: (linkPath: string, target: string) => Promise<void>;
  onCancel: () => void;
}

export function LinkDialog({
  dirPath,
  initialTarget = "",
  initialName = "",
  existing,
  onSubmit,
  onCancel
}: LinkDialogProps) {
  const [target, setTarget] = useState(initialTarget);
  const [name, setName] = useState(initialName);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  const trimmedName = name.trim();
  const trimmedTarget = target.trim();
  const valid =
    trimmedName.length > 0 &&
    trimmedTarget.length > 0 &&
    !trimmedName.includes("/") &&
    !existing.has(trimmedName);

  const submit = async () => {
    if (!valid) return;
    setBusy(true);
    setErr(null);
    try {
      await onSubmit(joinPath(dirPath, trimmedName), trimmedTarget);
    } catch (e) {
      setErr(e instanceof Error ? e.message : "Failed to create symbolic link");
      setBusy(false);
    }
  };

  return (
    <div
      className="fm-modal-backdrop"
      onMouseDown={(ev) => {
        if (ev.target === ev.currentTarget && !busy) onCancel();
      }}
    >
      <div className="fm-modal fm-modal-md" role="dialog" aria-modal="true">
        <div className="fm-modal-header">
          <span className="fm-modal-title">Create link</span>
          <button type="button" className="fm-icon-btn" onClick={onCancel} disabled={busy}>
            <XIcon size={15} />
          </button>
        </div>
        <div className="fm-modal-body">
          <div className="fm-field">
            <label className="fm-label">Original (target path)</label>
            <input
              className="fm-input fm-mono"
              value={target}
              onChange={(e) => setTarget(e.target.value)}
              placeholder="/path/to/target or relative/path"
              autoFocus
            />
          </div>
          <div className="fm-field">
            <label className="fm-label">Symbolic link name in {dirPath}</label>
            <input
              className="fm-input"
              value={name}
              onChange={(e) => setName(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  e.preventDefault();
                  void submit();
                }
              }}
              placeholder="shortcut-name"
            />
          </div>
          {existing.has(trimmedName) && (
            <div className="fm-alert fm-alert-warn">
              &quot;{trimmedName}&quot; already exists in this directory
            </div>
          )}
          {err && <div className="fm-alert fm-alert-error">{err}</div>}
        </div>
        <div className="fm-modal-footer">
          <button type="button" className="fm-btn" onClick={onCancel} disabled={busy}>
            Cancel
          </button>
          <button
            type="button"
            className="fm-btn fm-btn-primary"
            onClick={() => void submit()}
            disabled={!valid || busy}
          >
            {busy ? "Creating…" : "Create link"}
          </button>
        </div>
      </div>
    </div>
  );
}

// --- Delete confirmation ---

interface DeleteDialogProps {
  entries: FileEntry[];
  onConfirm: () => Promise<void>;
  onCancel: () => void;
}

export function DeleteDialog({ entries, onConfirm, onCancel }: DeleteDialogProps) {
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const single = entries.length === 1 ? entries[0] : null;

  const title = single
    ? `Delete ${TYPE_LABEL[single.type].toLowerCase()} "${single.name}"?`
    : `Delete ${entries.length} items?`;

  const confirm = async () => {
    setBusy(true);
    setErr(null);
    try {
      await onConfirm();
    } catch (e) {
      setErr(e instanceof Error ? e.message : "Delete failed");
      setBusy(false);
    }
  };

  return (
    <div
      className="fm-modal-backdrop"
      onMouseDown={(ev) => {
        if (ev.target === ev.currentTarget && !busy) onCancel();
      }}
    >
      <div className="fm-modal fm-modal-md" role="dialog" aria-modal="true">
        <div className="fm-modal-header">
          <span className="fm-modal-title">{title}</span>
          <button type="button" className="fm-icon-btn" onClick={onCancel} disabled={busy}>
            <XIcon size={15} />
          </button>
        </div>
        <div className="fm-modal-body">
          {single ? (
            <div className="fm-text">
              {single.type === "directory"
                ? "This directory and all of its contents will be permanently deleted."
                : "This item will be permanently deleted from the server."}
            </div>
          ) : (
            <ul className="fm-delete-list">
              {entries.slice(0, 12).map((e) => (
                <li key={e.name}>
                  <span className="fm-mono">{e.name}</span>{" "}
                  <span className="fm-muted">({TYPE_LABEL[e.type]})</span>
                </li>
              ))}
              {entries.length > 12 && (
                <li className="fm-muted">…and {entries.length - 12} more</li>
              )}
            </ul>
          )}
          {err && <div className="fm-alert fm-alert-error">{err}</div>}
        </div>
        <div className="fm-modal-footer">
          <button type="button" className="fm-btn" onClick={onCancel} disabled={busy}>
            Cancel
          </button>
          <button
            type="button"
            className="fm-btn fm-btn-danger"
            onClick={() => void confirm()}
            disabled={busy}
          >
            {busy ? "Deleting…" : "Delete"}
          </button>
        </div>
      </div>
    </div>
  );
}

// --- Edit Permissions & Ownership (Cockpit dialog) ---

interface PermissionsDialogProps {
  serverId: number;
  dirPath: string;
  entry: FileEntry;
  onSaved: () => void;
  onCancel: () => void;
}

const ACCESS_OPTIONS: Array<{ value: Access; label: string }> = [
  { value: "rw", label: "Read and write" },
  { value: "r", label: "Read-only" },
  { value: "none", label: "No access" }
];

export function PermissionsDialog({
  serverId,
  dirPath,
  entry,
  onSaved,
  onCancel
}: PermissionsDialogProps) {
  const isDir = isDirLike(entry);
  const [owner, setOwner] = useState(entry.owner);
  const [group, setGroup] = useState(entry.group);
  const [ownerAcc, setOwnerAcc] = useState<Access>(() => accessOf(entry.mode, 6));
  const [groupAcc, setGroupAcc] = useState<Access>(() => accessOf(entry.mode, 3));
  const [otherAcc, setOtherAcc] = useState<Access>(() => accessOf(entry.mode, 0));
  const [exec, setExec] = useState<boolean>(() => (entry.mode & 0o111) !== 0);
  const [recursive, setRecursive] = useState(false);
  const [principals, setPrincipals] = useState<FilePrincipalsOut>({
    users: [],
    groups: [],
    home: "/root"
  });
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    api
      .filesPrincipals(serverId)
      .then((res) => {
        if (alive) setPrincipals(res);
      })
      .catch(() => {});
    return () => {
      alive = false;
    };
  }, [serverId]);

  const computedMode = buildMode(entry.mode, ownerAcc, groupAcc, otherAcc, isDir, exec);
  const octal = `0${computedMode.toString(8).padStart(3, "0")}`;
  const fullPath = entry.path || (entry.name === "" ? dirPath : joinPath(dirPath, entry.name));

  const save = async () => {
    setBusy(true);
    setErr(null);
    const body: FileActionBody = {
      op: "chmod",
      paths: [fullPath],
      mode: computedMode,
      recursive
    };
    if (owner !== entry.owner || recursive) body.owner = owner;
    if (group !== entry.group || recursive) body.group = group;
    try {
      await api.filesAction(serverId, body);
      onSaved();
    } catch (e) {
      setErr(e instanceof Error ? e.message : "Failed to update permissions");
      setBusy(false);
    }
  };

  const userList = principals.users.includes(owner)
    ? principals.users
    : [owner, ...principals.users];
  const groupList = principals.groups.includes(group)
    ? principals.groups
    : [group, ...principals.groups];

  return (
    <div
      className="fm-modal-backdrop"
      onMouseDown={(ev) => {
        if (ev.target === ev.currentTarget && !busy) onCancel();
      }}
    >
      <div className="fm-modal fm-modal-md" role="dialog" aria-modal="true">
        <div className="fm-modal-header">
          <span className="fm-modal-title">
            {entry.name || dirPath} permissions
          </span>
          <button type="button" className="fm-icon-btn" onClick={onCancel} disabled={busy}>
            <XIcon size={15} />
          </button>
        </div>
        <div className="fm-modal-body">
          <div className="fm-perm-section">
            <div className="fm-perm-heading">Ownership</div>
            <div className="fm-perm-grid">
              <div className="fm-field">
                <label className="fm-label">Owner</label>
                <select
                  className="fm-select"
                  value={owner}
                  onChange={(e) => setOwner(e.target.value)}
                >
                  {userList.map((u) => (
                    <option key={u} value={u}>
                      {u}
                    </option>
                  ))}
                </select>
              </div>
              <div className="fm-field">
                <label className="fm-label">Group</label>
                <select
                  className="fm-select"
                  value={group}
                  onChange={(e) => setGroup(e.target.value)}
                >
                  {groupList.map((g) => (
                    <option key={g} value={g}>
                      {g}
                    </option>
                  ))}
                </select>
              </div>
            </div>
          </div>

          <div className="fm-perm-section">
            <div className="fm-perm-heading-row">
              <span className="fm-perm-heading">Access</span>
              <span className="fm-tag fm-mono">
                Mode {octal} · {entry.perms}
              </span>
            </div>
            <div className="fm-field">
              <label className="fm-label">Owner access</label>
              <select
                className="fm-select"
                value={ownerAcc}
                onChange={(e) => setOwnerAcc(e.target.value as Access)}
              >
                {ACCESS_OPTIONS.map((o) => (
                  <option key={o.value} value={o.value}>
                    {o.label}
                  </option>
                ))}
              </select>
            </div>
            <div className="fm-field">
              <label className="fm-label">Group access</label>
              <select
                className="fm-select"
                value={groupAcc}
                onChange={(e) => setGroupAcc(e.target.value as Access)}
              >
                {ACCESS_OPTIONS.map((o) => (
                  <option key={o.value} value={o.value}>
                    {o.label}
                  </option>
                ))}
              </select>
            </div>
            <div className="fm-field">
              <label className="fm-label">Others access</label>
              <select
                className="fm-select"
                value={otherAcc}
                onChange={(e) => setOtherAcc(e.target.value as Access)}
              >
                {ACCESS_OPTIONS.map((o) => (
                  <option key={o.value} value={o.value}>
                    {o.label}
                  </option>
                ))}
              </select>
            </div>

            {!isDir && (
              <label className="fm-checkbox">
                <input
                  type="checkbox"
                  checked={exec}
                  onChange={(e) => setExec(e.target.checked)}
                />
                <span>Set executable as program</span>
              </label>
            )}

            {isDir && (
              <label className="fm-checkbox">
                <input
                  type="checkbox"
                  checked={recursive}
                  onChange={(e) => setRecursive(e.target.checked)}
                />
                <span>Apply permissions and ownership to all contents</span>
              </label>
            )}
          </div>

          {err && <div className="fm-alert fm-alert-error">{err}</div>}
        </div>
        <div className="fm-modal-footer">
          <button type="button" className="fm-btn" onClick={onCancel} disabled={busy}>
            Cancel
          </button>
          <button
            type="button"
            className="fm-btn fm-btn-primary"
            onClick={() => void save()}
            disabled={busy}
          >
            {busy ? "Saving…" : "Change"}
          </button>
        </div>
      </div>
    </div>
  );
}

// --- Text File Editor / Viewer ---

interface EditorDialogProps {
  serverId: number;
  dirPath: string;
  entry: FileEntry;
  onSaved: () => void;
  onClose: () => void;
}

export function EditorDialog({
  serverId,
  dirPath,
  entry,
  onSaved,
  onClose
}: EditorDialogProps) {
  const fullPath = entry.path || joinPath(dirPath, entry.name);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [content, setContent] = useState("");
  const [initial, setInitial] = useState("");
  const [mtime, setMtime] = useState<number>(entry.mtime);
  const [binary, setBinary] = useState(false);
  const [tooLarge, setTooLarge] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const [savedNotice, setSavedNotice] = useState(false);
  const [confirmDiscard, setConfirmDiscard] = useState(false);

  useEffect(() => {
    let alive = true;
    setLoading(true);
    setErr(null);
    setBinary(false);
    setTooLarge(false);
    api
      .filesRead(serverId, fullPath)
      .then((res) => {
        if (!alive) return;
        setContent(res.content ?? "");
        setInitial(res.content ?? "");
        setMtime(res.mtime);
      })
      .catch((e) => {
        if (!alive) return;
        const msg = e instanceof Error ? e.message : "Failed to read file";
        if (/binary|not valid utf-8/i.test(msg)) setBinary(true);
        else if (/too large/i.test(msg)) setTooLarge(true);
        else setErr(msg);
      })
      .finally(() => {
        if (alive) setLoading(false);
      });
    return () => {
      alive = false;
    };
  }, [serverId, fullPath]);

  const dirty = !binary && !tooLarge && content !== initial;
  const lineCount = content ? content.split("\n").length : 1;

  const save = async () => {
    if (binary || tooLarge) return;
    setSaving(true);
    setErr(null);
    try {
      const res = await api.filesAction(serverId, {
        op: "write",
        path: fullPath,
        content,
        expected_mtime: mtime
      });
      if (typeof res.mtime === "number") setMtime(res.mtime);
      setInitial(content);
      setSavedNotice(true);
      setTimeout(() => setSavedNotice(false), 1800);
      onSaved();
    } catch (e) {
      setErr(e instanceof Error ? e.message : "Failed to save file");
    } finally {
      setSaving(false);
    }
  };

  const requestClose = () => {
    if (dirty && !confirmDiscard) {
      setConfirmDiscard(true);
      return;
    }
    onClose();
  };

  return (
    <div
      className="fm-modal-backdrop"
      onMouseDown={(ev) => {
        if (ev.target === ev.currentTarget && !saving) requestClose();
      }}
    >
      <div className="fm-modal fm-modal-lg" role="dialog" aria-modal="true">
        <div className="fm-modal-header">
          <div className="fm-editor-title">
            <span className="fm-mono">{entry.name}</span>
            <span className="fm-tag fm-mono">{entry.perms}</span>
            <span className="fm-muted">{formatSize(entry.size)}</span>
            {dirty && <span className="fm-tag fm-tag-warn">• Modified</span>}
            {savedNotice && <span className="fm-tag fm-tag-ok">Saved</span>}
          </div>
          <button type="button" className="fm-icon-btn" onClick={requestClose} disabled={saving}>
            <XIcon size={15} />
          </button>
        </div>

        <div className="fm-modal-body">
          {loading ? (
            <div className="fm-editor-loading">Loading {entry.name}…</div>
          ) : binary ? (
            <div className="fm-alert fm-alert-info">
              <div>
                <strong>Binary file</strong> — &quot;{entry.name}&quot; ({formatSize(entry.size)})
                cannot be edited as text. Use Download to inspect it locally.
              </div>
              <button
                type="button"
                className="fm-btn fm-btn-primary fm-btn-sm"
                onClick={() => {
                  window.location.href = api.filesDownloadUrl(serverId, fullPath);
                }}
              >
                Download
              </button>
            </div>
          ) : tooLarge ? (
            <div className="fm-alert fm-alert-warn">
              <div>
                <strong>File too large for inline editor</strong> — {formatSize(entry.size)}{" "}
                (limit 2 MB). Download the file to edit it locally.
              </div>
              <button
                type="button"
                className="fm-btn fm-btn-primary fm-btn-sm"
                onClick={() => {
                  window.location.href = api.filesDownloadUrl(serverId, fullPath);
                }}
              >
                Download
              </button>
            </div>
          ) : (
            <>
              <textarea
                value={content}
                onChange={(e) => {
                  setContent(e.target.value);
                  if (confirmDiscard) setConfirmDiscard(false);
                }}
                onKeyDown={(e) => {
                  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "s") {
                    e.preventDefault();
                    void save();
                  } else if (e.key === "Tab") {
                    e.preventDefault();
                    const el = e.currentTarget;
                    const start = el.selectionStart;
                    const end = el.selectionEnd;
                    const next = content.slice(0, start) + "  " + content.slice(end);
                    setContent(next);
                    requestAnimationFrame(() => {
                      el.selectionStart = el.selectionEnd = start + 2;
                    });
                  }
                }}
                spellCheck={false}
                className="fm-editor-textarea"
              />
              <div className="fm-editor-statusbar">
                <span>
                  {lineCount} lines · {content.length.toLocaleString()} chars · UTF-8
                </span>
                <span>
                  Modified {formatDate(mtime)} · Ctrl+S to save
                </span>
              </div>
            </>
          )}
          {confirmDiscard && (
            <div className="fm-alert fm-alert-warn">
              <span>You have unsaved changes in {entry.name}. Discard changes?</span>
              <div style={{ display: "flex", gap: 6 }}>
                <button
                  type="button"
                  className="fm-btn fm-btn-sm"
                  onClick={() => setConfirmDiscard(false)}
                >
                  Keep editing
                </button>
                <button
                  type="button"
                  className="fm-btn fm-btn-danger fm-btn-sm"
                  onClick={onClose}
                >
                  Discard
                </button>
              </div>
            </div>
          )}
          {err && <div className="fm-alert fm-alert-error">{err}</div>}
        </div>

        <div className="fm-modal-footer">
          <button type="button" className="fm-btn" onClick={requestClose} disabled={saving}>
            Close
          </button>
          {!binary && !tooLarge && !loading && (
            <button
              type="button"
              className="fm-btn fm-btn-primary"
              disabled={!dirty || saving}
              onClick={() => void save()}
            >
              {saving ? "Saving…" : "Save"}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
