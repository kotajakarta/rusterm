import { useState, type FormEvent } from "react";
import { api, errorMessage } from "../api";
import type { TailscaleAccount } from "../types";
import { Icon } from "./Icon";
import { Modal } from "./Modal";

interface Props {
  accounts: TailscaleAccount[];
  onAccountsChanged: () => void;
  onClose: () => void;
}

export function TailscaleAccountModal({ accounts, onAccountsChanged, onClose }: Props) {
  const [activeTab, setActiveTab] = useState<"list" | "add" | "edit">(accounts.length === 0 ? "add" : "list");
  const [editingAccount, setEditingAccount] = useState<TailscaleAccount | null>(null);
  const [name, setName] = useState("");
  const [tailnet, setTailnet] = useState("-");
  const [apiKey, setApiKey] = useState("");
  const [showToken, setShowToken] = useState(false);
  const [testing, setTesting] = useState(false);
  const [saving, setSaving] = useState(false);
  const [verifyMsg, setVerifyMsg] = useState<{ ok: boolean; title: string; detail?: string } | null>(null);
  const [deletingId, setDeletingId] = useState<number | null>(null);

  const handleTest = async () => {
    const trimmed = apiKey.trim();
    if (!trimmed) {
      setVerifyMsg({ ok: false, title: "Tailscale API Access Token wajib diisi." });
      return;
    }
    setTesting(true);
    setVerifyMsg(null);
    try {
      const res = await api.tailscaleVerifyToken(trimmed, tailnet.trim() || "-");
      setVerifyMsg({
        ok: true,
        title: "Koneksi Berhasil",
        detail: `Tailnet: ${res.tailnet} • Terdeteksi ${res.devices_count} mesin`,
      });
      if (!name) {
        setName(res.tailnet !== "-" ? res.tailnet : "Tailnet Akun");
      }
    } catch (err) {
      setVerifyMsg({ ok: false, title: "Koneksi Gagal", detail: errorMessage(err) });
    } finally {
      setTesting(false);
    }
  };

  const handleSave = async (e: FormEvent) => {
    e.preventDefault();
    const trimmedKey = apiKey.trim();
    const trimmedName = name.trim();
    const trimmedTailnet = tailnet.trim() || "-";

    if (!trimmedName) {
      setVerifyMsg({ ok: false, title: "Nama akun wajib diisi." });
      return;
    }

    if (activeTab === "add" && !trimmedKey) {
      setVerifyMsg({ ok: false, title: "API Access Token wajib diisi." });
      return;
    }

    setSaving(true);
    setVerifyMsg(null);

    try {
      if (activeTab === "edit" && editingAccount) {
        await api.tailscaleUpdateAccount(editingAccount.id, {
          name: trimmedName,
          tailnet: trimmedTailnet,
          ...(trimmedKey ? { api_key: trimmedKey } : {}),
        });
      } else {
        await api.tailscaleCreateAccount({
          name: trimmedName,
          api_key: trimmedKey,
          tailnet: trimmedTailnet,
        });
      }
      onAccountsChanged();
      resetForm();
      setActiveTab("list");
    } catch (err) {
      setVerifyMsg({ ok: false, title: "Gagal menyimpan akun", detail: errorMessage(err) });
    } finally {
      setSaving(false);
    }
  };

  const startEdit = (acc: TailscaleAccount) => {
    setEditingAccount(acc);
    setName(acc.name);
    setTailnet(acc.tailnet);
    setApiKey("");
    setVerifyMsg(null);
    setActiveTab("edit");
  };

  const resetForm = () => {
    setEditingAccount(null);
    setName("");
    setTailnet("-");
    setApiKey("");
    setVerifyMsg(null);
  };

  const handleDelete = async (id: number) => {
    try {
      await api.tailscaleDeleteAccount(id);
      setDeletingId(null);
      onAccountsChanged();
    } catch (err) {
      alert(`Gagal menghapus: ${errorMessage(err)}`);
    }
  };

  return (
    <Modal title="Manajemen Akun Tailscale API" onClose={onClose} wide>
      <div className="cf-modal-tabs">
        <button
          type="button"
          className={`cf-tab-btn ${activeTab === "list" ? "active" : ""}`}
          onClick={() => {
            resetForm();
            setActiveTab("list");
          }}
        >
          Daftar Akun ({accounts.length})
        </button>
        <button
          type="button"
          className={`cf-tab-btn ${activeTab === "add" || activeTab === "edit" ? "active" : ""}`}
          onClick={() => {
            if (activeTab !== "edit") resetForm();
            setActiveTab(editingAccount ? "edit" : "add");
          }}
        >
          {activeTab === "edit" ? "Edit Akun" : "+ Tambah Akun Baru"}
        </button>
      </div>

      {activeTab === "list" ? (
        <div className="cf-modal-content">
          {accounts.length === 0 ? (
            <div className="cf-empty-box">
              <p>Belum ada akun Tailscale yang terdaftar.</p>
              <button
                type="button"
                className="btn primary sm"
                style={{ marginTop: "0.5rem" }}
                onClick={() => {
                  resetForm();
                  setActiveTab("add");
                }}
              >
                + Tambah Akun Sekarang
              </button>
            </div>
          ) : (
            <div className="cf-account-list">
              {accounts.map((acc) => (
                <div key={acc.id} className="cf-account-card">
                  <div className="cf-account-info">
                    <div className="cf-acc-name">
                      <strong>{acc.name}</strong>
                    </div>
                    <div className="cf-acc-details" style={{ display: "flex", flexDirection: "column", gap: "0.2rem" }}>
                      <span>
                        Tailnet: <code>{acc.tailnet}</code>
                      </span>
                      <span>
                        Token: <code>{acc.token_masked}</code>
                      </span>
                      <small style={{ color: "var(--text-muted)" }}>Ditambahkan: {acc.created_at}</small>
                    </div>
                  </div>
                  <div className="cf-account-actions">
                    <button
                      type="button"
                      className="icon-btn"
                      onClick={() => startEdit(acc)}
                      title="Edit Akun"
                    >
                      <Icon name="edit" size={14} />
                    </button>
                    {deletingId === acc.id ? (
                      <div className="cf-confirm-del">
                        <span>Hapus?</span>
                        <button
                          type="button"
                          className="btn danger sm"
                          onClick={() => void handleDelete(acc.id)}
                        >
                          Ya
                        </button>
                        <button
                          type="button"
                          className="btn sm"
                          onClick={() => setDeletingId(null)}
                        >
                          Batal
                        </button>
                      </div>
                    ) : (
                      <button
                        type="button"
                        className="icon-btn danger"
                        onClick={() => setDeletingId(acc.id)}
                        title="Hapus Akun"
                      >
                        <Icon name="trash" size={14} />
                      </button>
                    )}
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
      ) : (
        <form onSubmit={handleSave} className="cf-modal-form">
          <div className="form-group">
            <label htmlFor="ts-name">
              Nama Label Akun <span className="req">*</span>
            </label>
            <input
              id="ts-name"
              type="text"
              placeholder="Contoh: My Tailnet / Server Kantor"
              value={name}
              onChange={(e) => setName(e.target.value)}
              required
              autoFocus
            />
          </div>

          <div className="form-group">
            <label htmlFor="ts-tailnet">
              Tailnet Name / Org (Gunakan <code>-</code> untuk default user token)
            </label>
            <input
              id="ts-tailnet"
              type="text"
              placeholder="default: -"
              value={tailnet}
              onChange={(e) => setTailnet(e.target.value)}
            />
            <small style={{ color: "var(--text-muted)" }}>
              Isi dengan nama Tailnet Anda (misal <code>example.com</code> atau <code>user@gmail.com</code>), atau biarkan <code>-</code> jika menggunakan Personal Access Token.
            </small>
          </div>

          <div className="form-group">
            <label htmlFor="ts-token">
              Tailscale API Access Token <span className="req">*</span>
            </label>
            <div className="cf-token-input-wrap">
              <input
                id="ts-token"
                type={showToken ? "text" : "password"}
                placeholder={
                  activeTab === "edit"
                    ? "(Kosongkan jika tidak ingin mengubah token)"
                    : "tskey-api-k..."
                }
                value={apiKey}
                onChange={(e) => setApiKey(e.target.value)}
                required={activeTab === "add"}
              />
              <button
                type="button"
                className="btn sm"
                onClick={() => setShowToken(!showToken)}
                style={{ whiteSpace: "nowrap" }}
              >
                {showToken ? "Sembunyikan" : "Tampilkan"}
              </button>
            </div>
            <small style={{ color: "var(--text-muted)" }}>
              Dapatkan di Tailscale Admin Console &gt; <b>Settings &gt; Keys &gt; Generate API access token</b>.
            </small>
          </div>

          <div style={{ margin: "0.8rem 0" }}>
            <button
              type="button"
              className="btn sm"
              disabled={testing || !apiKey.trim()}
              onClick={() => void handleTest()}
            >
              <Icon name="bolt" size={13} /> {testing ? "Memeriksa..." : "Uji Koneksi Token"}
            </button>
          </div>

          {verifyMsg && (
            <div
              className={`cf-alert ${verifyMsg.ok ? "success" : "danger"}`}
              style={{
                padding: "0.6rem 0.8rem",
                borderRadius: "6px",
                marginBottom: "1rem",
                fontSize: "0.85rem",
                backgroundColor: verifyMsg.ok ? "rgba(34, 197, 94, 0.15)" : "rgba(239, 68, 68, 0.15)",
                border: `1px solid ${verifyMsg.ok ? "rgba(34, 197, 94, 0.4)" : "rgba(239, 68, 68, 0.4)"}`,
              }}
            >
              <div style={{ fontWeight: 600 }}>{verifyMsg.title}</div>
              {verifyMsg.detail && <div style={{ marginTop: "0.2rem" }}>{verifyMsg.detail}</div>}
            </div>
          )}

          <div className="form-actions" style={{ display: "flex", justifyContent: "flex-end", gap: "0.5rem" }}>
            <button
              type="button"
              className="btn"
              onClick={() => {
                resetForm();
                setActiveTab("list");
              }}
            >
              Batal
            </button>
            <button type="submit" className="btn primary" disabled={saving}>
              {saving ? "Menyimpan..." : activeTab === "edit" ? "Perbarui Akun" : "Simpan Akun"}
            </button>
          </div>
        </form>
      )}
    </Modal>
  );
}
