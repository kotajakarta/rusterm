import { useState, type FormEvent } from "react";

import { api, errorMessage } from "../api";
import { usePersistentFlag } from "../storage";
import type { Snippet } from "../types";
import { Icon } from "./Icon";
import { Modal } from "./Modal";

interface SnippetsProps {
  snippets: Snippet[];
  canRun: boolean;
  onRun: (snippet: Snippet) => void;
  onChanged: () => void;
}

export function Snippets({ snippets, canRun, onRun, onChanged }: SnippetsProps) {
  const [editing, setEditing] = useState<Snippet | "new" | null>(null);
  const [collapsed, setCollapsed] = usePersistentFlag("termdeck.snippets.collapsed", false);

  const remove = async (snippet: Snippet) => {
    try {
      await api.deleteSnippet(snippet.id);
    } catch {
      // Refreshing the list below shows whatever state the backend has.
    } finally {
      onChanged();
    }
  };

  return (
    <section className={`snippets${collapsed ? " is-collapsed" : ""}`}>
      <header className="section-head">
        <button
          type="button"
          className="section-toggle"
          onClick={() => setCollapsed(!collapsed)}
          aria-expanded={!collapsed}
          title={collapsed ? "Tampilkan snippets" : "Sembunyikan snippets"}
        >
          <span className={`chev-icon${collapsed ? "" : " is-open"}`}>
            <Icon name="chevron" size={13} />
          </span>
          <h3>
            Snippets{collapsed && snippets.length > 0 ? ` (${snippets.length})` : ""}
          </h3>
        </button>
        <button type="button" className="icon-btn" onClick={() => setEditing("new")} aria-label="Tambah snippet" title="Tambah snippet">
          <Icon name="plus" size={14} />
        </button>
      </header>
      {!collapsed && snippets.length === 0 && (
        <p className="empty-note">Simpan perintah yang sering diketik, lalu jalankan di panel aktif sekali klik.</p>
      )}
      <ul hidden={collapsed}>
        {snippets.map((s) => (
          <li key={s.id} className="snippet-row">
            <button
              type="button"
              className="snippet-main"
              disabled={!canRun}
              onClick={() => onRun(s)}
              title={canRun ? `Kirim ke panel aktif: ${s.command}` : "Buka terminal dulu untuk menjalankan snippet"}
            >
              <Icon name="bolt" size={14} />
              <span className="snippet-name">{s.name}</span>
            </button>
            <span className="row-actions">
              <button type="button" className="icon-btn" onClick={() => setEditing(s)} aria-label={`Ubah snippet ${s.name}`}>
                <Icon name="edit" size={14} />
              </button>
              <button
                type="button"
                className="icon-btn danger"
                onClick={() => void remove(s)}
                aria-label={`Hapus snippet ${s.name}`}
              >
                <Icon name="trash" size={14} />
              </button>
            </span>
          </li>
        ))}
      </ul>
      {editing && (
        <SnippetForm
          snippet={editing === "new" ? null : editing}
          onClose={() => setEditing(null)}
          onSaved={() => {
            setEditing(null);
            onChanged();
          }}
        />
      )}
    </section>
  );
}

function SnippetForm({ snippet, onClose, onSaved }: { snippet: Snippet | null; onClose: () => void; onSaved: () => void }) {
  const [name, setName] = useState(snippet?.name ?? "");
  const [command, setCommand] = useState(snippet?.command ?? "");
  const [runNow, setRunNow] = useState(snippet ? snippet.command.endsWith("\n") : true);
  const [error, setError] = useState<string | null>(null);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    const data = { name: name.trim(), command: command.replace(/\n+$/, "") + (runNow ? "\n" : "") };
    try {
      if (snippet) await api.updateSnippet(snippet.id, data);
      else await api.createSnippet(data);
      onSaved();
    } catch (err) {
      setError(errorMessage(err));
    }
  };

  return (
    <Modal
      title={snippet ? "Ubah snippet" : "Snippet baru"}
      onClose={onClose}
      footer={
        <>
          <span className="spacer" />
          <button type="button" className="btn" onClick={onClose}>
            Batal
          </button>
          <button type="submit" form="snippet-form" className="btn btn-primary" disabled={!name.trim() || !command.trim()}>
            Simpan snippet
          </button>
        </>
      }
    >
      <form id="snippet-form" className="form" onSubmit={submit}>
        <label className="field">
          <span>Nama</span>
          <input value={name} onChange={(e) => setName(e.target.value)} placeholder="Deploy ulang" required />
        </label>
        <label className="field">
          <span>Perintah</span>
          <textarea
            className="mono"
            rows={4}
            value={command.replace(/\n$/, "")}
            onChange={(e) => setCommand(e.target.value)}
            placeholder="git pull && php artisan migrate --force"
            spellCheck={false}
            required
          />
        </label>
        <label className="check">
          <input type="checkbox" checked={runNow} onChange={(e) => setRunNow(e.target.checked)} />
          Langsung jalankan (tekan Enter otomatis)
        </label>
        {error && <p className="notice bad">{error}</p>}
      </form>
    </Modal>
  );
}
