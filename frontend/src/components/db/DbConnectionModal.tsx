import { useState } from "react";
import { api, errorMessage } from "../../api";
import { defaultPortForEngine } from "../../dbClient";
import type {
  DbConnection,
  DbConnectionInput,
  DbConnectionTestResult,
  DbEngine,
  DbSslMode,
  Server,
} from "../../types";
import { Modal } from "../Modal";

interface DbConnectionModalProps {
  initial?: DbConnection | null;
  defaultSshServerId?: number | null;
  servers: Server[];
  onSaved: (conn: DbConnection) => void;
  onClose: () => void;
}

export function DbConnectionModal({
  initial,
  defaultSshServerId = null,
  servers,
  onSaved,
  onClose,
}: DbConnectionModalProps): JSX.Element {
  const [name, setName] = useState(initial?.name ?? "");
  const [engine, setEngine] = useState<DbEngine>(initial?.engine ?? "postgres");
  const [host, setHost] = useState(initial?.host ?? "127.0.0.1");
  const [port, setPort] = useState(
    initial ? String(initial.port || defaultPortForEngine(initial.engine)) : "5432",
  );
  const [username, setUsername] = useState(initial?.username ?? "");
  const [password, setPassword] = useState("");
  const [clearPassword, setClearPassword] = useState(false);
  const [defaultDatabase, setDefaultDatabase] = useState(initial?.default_database ?? "");
  const [filePath, setFilePath] = useState(initial?.file_path ?? "");
  const [sslMode, setSslMode] = useState<DbSslMode>(initial?.ssl_mode ?? "prefer");
  const [sshServerId, setSshServerId] = useState<string>(
    initial?.ssh_server_id
      ? String(initial.ssh_server_id)
      : defaultSshServerId
        ? String(defaultSshServerId)
        : "",
  );
  const [readOnly, setReadOnly] = useState(initial?.read_only ?? false);
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<DbConnectionTestResult | null>(null);
  const [error, setError] = useState<string | null>(null);

  const handleEngineChange = (next: DbEngine) => {
    setEngine(next);
    setTestResult(null);
    if (!initial || next !== initial.engine) {
      setPort(String(defaultPortForEngine(next)));
    }
  };

  const buildPayload = (): DbConnectionInput => ({
    name: name.trim(),
    engine,
    host: engine === "sqlite" ? "" : host.trim() || "127.0.0.1",
    port: engine === "sqlite" ? 0 : Number(port) || defaultPortForEngine(engine),
    username: engine === "sqlite" ? "" : username.trim(),
    password: clearPassword ? "" : password.length > 0 ? password : null,
    default_database: engine === "sqlite" ? "" : defaultDatabase.trim(),
    file_path: engine === "sqlite" ? filePath.trim() : "",
    ssl_mode: sslMode,
    ssh_server_id: engine === "sqlite" ? null : sshServerId ? Number(sshServerId) : null,
    read_only: readOnly,
    color: initial?.color ?? "",
  });

  const handleTest = async () => {
    setTesting(true);
    setTestResult(null);
    setError(null);
    try {
      const payload = buildPayload();
      const res = await api.dbTestConnection({
        id: initial?.id ?? null,
        engine: payload.engine,
        host: payload.host,
        port: payload.port,
        username: payload.username,
        password: payload.password,
        default_database: payload.default_database,
        file_path: payload.file_path,
        ssl_mode: payload.ssl_mode,
        ssh_server_id: payload.ssh_server_id,
        read_only: payload.read_only,
      });
      setTestResult(res);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setTesting(false);
    }
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    const payload = buildPayload();
    if (!payload.name) {
      setError("Nama koneksi wajib diisi.");
      return;
    }
    if (payload.engine === "sqlite" && !payload.file_path) {
      setError("Path file SQLite wajib diisi.");
      return;
    }
    setSaving(true);
    try {
      const saved = initial
        ? await api.dbUpdateConnection(initial.id, payload)
        : await api.dbCreateConnection(payload);
      onSaved(saved);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setSaving(false);
    }
  };

  const isSqlite = engine === "sqlite";

  return (
    <Modal
      title={initial ? "Edit Koneksi Database" : "Tambah Koneksi Database"}
      onClose={onClose}
      wide
      footer={
        <>
          <button
            type="button"
            className="btn"
            onClick={() => void handleTest()}
            disabled={testing || saving}
          >
            {testing ? "Menguji..." : "Test Koneksi"}
          </button>
          <div style={{ flex: 1 }} />
          <button type="button" className="btn" onClick={onClose} disabled={saving}>
            Batal
          </button>
          <button
            type="submit"
            form="db-conn-form"
            className="btn btn-primary"
            disabled={saving}
          >
            {saving ? "Menyimpan..." : "Simpan"}
          </button>
        </>
      }
    >
      <form id="db-conn-form" className="db-form" onSubmit={(e) => void handleSubmit(e)}>
        <div className="db-form-grid">
          <label className="field">
            <span>Nama Koneksi</span>
            <input
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="Contoh: Prod Postgres"
              required
            />
          </label>
          <label className="field">
            <span>Engine</span>
            <select value={engine} onChange={(e) => handleEngineChange(e.target.value as DbEngine)}>
              <option value="postgres">PostgreSQL</option>
              <option value="mysql">MySQL / MariaDB</option>
              <option value="sqlite">SQLite (File)</option>
            </select>
          </label>
        </div>

        {isSqlite ? (
          <label className="field">
            <span>Path File SQLite</span>
            <input
              type="text"
              value={filePath}
              onChange={(e) => setFilePath(e.target.value)}
              placeholder="/path/to/database.sqlite"
              required
            />
          </label>
        ) : (
          <>
            <div className="db-form-grid">
              <label className="field">
                <span>Host</span>
                <input
                  type="text"
                  value={host}
                  onChange={(e) => setHost(e.target.value)}
                  placeholder="127.0.0.1"
                  required
                />
              </label>
              <label className="field">
                <span>Port</span>
                <input
                  type="number"
                  min={1}
                  max={65535}
                  value={port}
                  onChange={(e) => setPort(e.target.value)}
                  required
                />
              </label>
            </div>

            <div className="db-form-grid">
              <label className="field">
                <span>Username</span>
                <input
                  type="text"
                  value={username}
                  onChange={(e) => setUsername(e.target.value)}
                  placeholder={engine === "postgres" ? "postgres" : "root"}
                />
              </label>
              <label className="field">
                <span>
                  Password{" "}
                  {initial?.has_password && !clearPassword && (
                    <small className="db-muted">(tersimpan — kosongkan untuk tetap)</small>
                  )}
                </span>
                <input
                  type="password"
                  value={password}
                  onChange={(e) => {
                    setPassword(e.target.value);
                    setClearPassword(false);
                  }}
                  placeholder={initial?.has_password ? "••••••••" : "Password database"}
                />
              </label>
            </div>

            <div className="db-form-grid">
              <label className="field">
                <span>Database Default (opsional)</span>
                <input
                  type="text"
                  value={defaultDatabase}
                  onChange={(e) => setDefaultDatabase(e.target.value)}
                  placeholder={engine === "postgres" ? "postgres" : "nama_db"}
                />
              </label>
              <label className="field">
                <span>SSL Mode</span>
                <select
                  value={sslMode}
                  onChange={(e) => setSslMode(e.target.value as DbSslMode)}
                >
                  <option value="prefer">Prefer (otomatis TLS jika didukung)</option>
                  <option value="disable">Disable (tanpa TLS)</option>
                  <option value="require">Require (wajib TLS)</option>
                </select>
              </label>
            </div>

            <label className="field">
              <span>SSH Tunnel (opsional — gunakan server SSH RusTerm)</span>
              <select value={sshServerId} onChange={(e) => setSshServerId(e.target.value)}>
                <option value="">Direct (tanpa SSH tunnel)</option>
                {servers.map((s) => (
                  <option key={s.id} value={String(s.id)}>
                    {s.name} ({s.username}@{s.host}:{s.port})
                  </option>
                ))}
              </select>
            </label>
          </>
        )}

        <label className="db-checkbox">
          <input
            type="checkbox"
            checked={readOnly}
            onChange={(e) => setReadOnly(e.target.checked)}
          />
          <span>
            <strong>Mode Read-Only</strong> — blokir semua edit data, insert, delete, dan DDL di backend
          </span>
        </label>

        {testResult && (
          <div className={`db-banner ${testResult.ok ? "is-ok" : "is-err"}`}>
            <strong>{testResult.ok ? "Berhasil" : "Gagal"}</strong>: {testResult.message}
            {testResult.version ? ` — ${testResult.version}` : ""} ({testResult.elapsed_ms} ms)
          </div>
        )}
        {error && <div className="db-banner is-err">{error}</div>}
      </form>
    </Modal>
  );
}
