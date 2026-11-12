import { useState, type FormEvent } from "react";

import { api, errorMessage } from "../api";
import { SERVER_COLORS } from "../format";
import type { AuthType, ConnectionCheck, Server, ServerInput } from "../types";
import { Modal } from "./Modal";

interface ServerFormProps {
  server: Server | null;
  groups: string[];
  initialValues?: { host?: string; name?: string };
  onClose: () => void;
  onSaved: (server: Server) => void;
}

export function ServerForm({ server, groups, initialValues, onClose, onSaved }: ServerFormProps) {
  const editing = server !== null;
  const [name, setName] = useState(server?.name ?? initialValues?.name ?? "");
  const [host, setHost] = useState(server?.host ?? initialValues?.host ?? "");
  const [port, setPort] = useState(String(server?.port ?? 22));
  const [username, setUsername] = useState(server?.username ?? "root");
  const [authType, setAuthType] = useState<AuthType>(server?.auth_type ?? "password");
  const [password, setPassword] = useState("");
  const [privateKey, setPrivateKey] = useState("");
  const [passphrase, setPassphrase] = useState("");
  const [groupName, setGroupName] = useState(server?.group_name ?? "");
  const [color, setColor] = useState(server?.color ?? "");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [check, setCheck] = useState<ConnectionCheck | null>(null);

  // On edit an empty secret field means "keep what is stored".
  const secret = (value: string): string | null => (editing && value === "" ? null : value);

  const payload = (): ServerInput => ({
    name: name.trim() || host.trim(),
    host: host.trim(),
    port: Number(port) || 22,
    username: username.trim(),
    auth_type: authType,
    password: authType === "password" ? secret(password) : null,
    private_key: authType === "key" ? secret(privateKey) : null,
    passphrase: authType === "key" ? secret(passphrase) : null,
    group_name: groupName.trim(),
    color,
  });

  const loadKeyFile = async (file: File | undefined) => {
    if (!file) return;
    setPrivateKey(await file.text());
  };

  const runTest = async () => {
    setBusy(true);
    setCheck(null);
    setError(null);
    try {
      setCheck(await api.testServer({ ...payload(), id: server?.id ?? null }));
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    setBusy(true);
    setError(null);
    try {
      const saved = editing ? await api.updateServer(server.id, payload()) : await api.createServer(payload());
      onSaved(saved);
    } catch (err) {
      setError(errorMessage(err));
      setBusy(false);
    }
  };

  const resetHostKey = async () => {
    if (!server) return;
    try {
      onSaved(await api.resetHostKey(server.id));
    } catch (err) {
      setError(errorMessage(err));
    }
  };

  const keepHint = editing ? "Kosongkan jika tidak diubah" : "";

  return (
    <Modal
      title={editing ? `Ubah ${server.name}` : "Tambah server"}
      onClose={onClose}
      footer={
        <>
          <button type="button" className="btn" onClick={runTest} disabled={busy || !host || !username}>
            Tes koneksi
          </button>
          <span className="spacer" />
          <button type="button" className="btn" onClick={onClose}>
            Batal
          </button>
          <button type="submit" form="server-form" className="btn btn-primary" disabled={busy}>
            {editing ? "Simpan perubahan" : "Tambah server"}
          </button>
        </>
      }
    >
      <form id="server-form" className="form" onSubmit={submit}>
        <div className="field-row">
          <label className="field grow">
            <span>Host atau IP</span>
            <input value={host} onChange={(e) => setHost(e.target.value)} placeholder="203.0.113.10" required />
          </label>
          <label className="field port">
            <span>Port</span>
            <input
              value={port}
              onChange={(e) => setPort(e.target.value.replace(/\D/g, ""))}
              inputMode="numeric"
              required
            />
          </label>
        </div>
        <div className="field-row">
          <label className="field grow">
            <span>Username</span>
            <input value={username} onChange={(e) => setUsername(e.target.value)} required autoComplete="off" />
          </label>
          <label className="field grow">
            <span>Nama tampilan</span>
            <input value={name} onChange={(e) => setName(e.target.value)} placeholder={host || "web-production"} />
          </label>
        </div>

        <fieldset className="segmented" aria-label="Metode login">
          {(["password", "key"] as const).map((type) => (
            <label key={type} className={authType === type ? "is-on" : ""}>
              <input
                type="radio"
                name="auth"
                value={type}
                checked={authType === type}
                onChange={() => setAuthType(type)}
              />
              {type === "password" ? "Password" : "Private key"}
            </label>
          ))}
        </fieldset>

        {authType === "password" ? (
          <label className="field">
            <span>Password {editing && server.has_password && <em className="hint">tersimpan</em>}</span>
            <input
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              placeholder={keepHint}
              autoComplete="new-password"
            />
          </label>
        ) : (
          <>
            <label className="field">
              <span>
                Private key (OpenSSH / PEM){" "}
                {editing && server.has_private_key && <em className="hint">tersimpan</em>}
              </span>
              <textarea
                className="mono"
                rows={5}
                value={privateKey}
                onChange={(e) => setPrivateKey(e.target.value)}
                placeholder={editing ? keepHint : "-----BEGIN OPENSSH PRIVATE KEY-----"}
                spellCheck={false}
              />
            </label>
            <div className="field-row">
              <label className="btn file-btn">
                Pilih file key…
                <input type="file" onChange={(e) => void loadKeyFile(e.target.files?.[0])} />
              </label>
              <label className="field grow">
                <span>Passphrase key</span>
                <input
                  type="password"
                  value={passphrase}
                  onChange={(e) => setPassphrase(e.target.value)}
                  placeholder={editing ? keepHint : "Jika ada"}
                  autoComplete="new-password"
                />
              </label>
            </div>
          </>
        )}

        <div className="field-row">
          <label className="field grow">
            <span>Grup</span>
            <input
              value={groupName}
              onChange={(e) => setGroupName(e.target.value)}
              list="server-groups"
              placeholder="Mis. Klien A"
            />
            <datalist id="server-groups">
              {groups.map((g) => (
                <option key={g} value={g} />
              ))}
            </datalist>
          </label>
        </div>

        <div className="field">
          <span>Warna penanda</span>
          <div className="swatches" role="radiogroup" aria-label="Warna penanda">
            {SERVER_COLORS.map((c) => (
              <button
                key={c.value || "none"}
                type="button"
                role="radio"
                aria-checked={color === c.value}
                aria-label={c.label}
                title={c.label}
                className={`swatch${color === c.value ? " is-on" : ""}${c.value ? "" : " is-none"}`}
                style={c.value ? { background: c.value } : undefined}
                onClick={() => setColor(c.value)}
              />
            ))}
          </div>
        </div>

        {editing && server.host_key_fingerprint && (
          <div className="hostkey">
            <span>
              Host key dipercaya: <code>{server.host_key_fingerprint}</code>
            </span>
            <button type="button" className="link-btn" onClick={resetHostKey}>
              Reset host key
            </button>
          </div>
        )}

        {check && <p className={check.ok ? "notice ok" : "notice bad"}>{check.message}</p>}
        {error && <p className="notice bad">{error}</p>}
      </form>
    </Modal>
  );
}
