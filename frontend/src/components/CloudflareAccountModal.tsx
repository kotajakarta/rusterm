import { useState, type FormEvent } from "react";
import { api, errorMessage } from "../api";
import type { CFAccount } from "../types";
import { Icon } from "./Icon";
import { Modal } from "./Modal";

interface Props {
  accounts: CFAccount[];
  onAccountsChanged: () => void;
  onClose: () => void;
}

export function CloudflareAccountModal({ accounts, onAccountsChanged, onClose }: Props) {
  const [activeTab, setActiveTab] = useState<"list" | "add" | "edit">(accounts.length === 0 ? "add" : "list");
  const [editingAccount, setEditingAccount] = useState<CFAccount | null>(null);
  const [name, setName] = useState("");
  const [token, setToken] = useState("");
  const [showToken, setShowToken] = useState(false);
  const [testing, setTesting] = useState(false);
  const [saving, setSaving] = useState(false);
  const [verifyMsg, setVerifyMsg] = useState<{ ok: boolean; title: string; detail?: string } | null>(null);
  const [deletingId, setDeletingId] = useState<number | null>(null);
  const [copiedAccId, setCopiedAccId] = useState<string | null>(null);

  const handleTest = async () => {
    const trimmed = token.trim();
    if (!trimmed) {
      setVerifyMsg({ ok: false, title: "Token Cloudflare wajib diisi." });
      return;
    }
    setTesting(true);
    setVerifyMsg(null);
    try {
      const res = await api.cfVerifyToken(trimmed);
      const dnsStatus = res.has_dns_access
        ? "Sinkronisasi DNS CNAME Otomatis Aktif"
        : "Otomatisasi DNS Nonaktif (Token belum memiliki izin 'Zone > DNS > Edit')";
      setVerifyMsg({
        ok: true,
        title: "Koneksi Akun Berhasil",
        detail: `Terhubung ke: ${res.account_name} (ID: ${res.account_id}) • ${dnsStatus}`,
      });
      if (!name) {
        setName(res.account_name || "Akun Cloudflare");
      }
    } catch (err) {
      setVerifyMsg({ ok: false, title: "Koneksi Gagal", detail: errorMessage(err) });
    } finally {
      setTesting(false);
    }
  };

  const handleSave = async (e: FormEvent) => {
    e.preventDefault();
    const trimmedToken = token.trim();
    const trimmedName = name.trim();
    if (!trimmedToken) {
      setVerifyMsg({ ok: false, title: "Token Cloudflare tidak boleh kosong." });
      return;
    }
    setSaving(true);
    setVerifyMsg(null);
    try {
      await api.cfCreateAccount(trimmedName || "Akun Cloudflare", trimmedToken);
      setName("");
      setToken("");
      onAccountsChanged();
      setActiveTab("list");
    } catch (err) {
      setVerifyMsg({ ok: false, title: "Gagal Menyimpan Akun", detail: errorMessage(err) });
    } finally {
      setSaving(false);
    }
  };

  const handleStartEdit = (acc: CFAccount) => {
    setEditingAccount(acc);
    setName(acc.name);
    setToken("");
    setShowToken(false);
    setVerifyMsg(null);
    setActiveTab("edit");
  };

  const handleCancelEdit = () => {
    setEditingAccount(null);
    setName("");
    setToken("");
    setShowToken(false);
    setVerifyMsg(null);
    setActiveTab("list");
  };

  const handleUpdateSave = async (e: FormEvent) => {
    e.preventDefault();
    if (!editingAccount) return;

    const trimmedName = name.trim();
    const trimmedToken = token.trim();
    if (!trimmedName) {
      setVerifyMsg({ ok: false, title: "Nama akun tidak boleh kosong." });
      return;
    }

    setSaving(true);
    setVerifyMsg(null);
    try {
      await api.cfUpdateAccount(editingAccount.id, {
        name: trimmedName,
        ...(trimmedToken ? { token: trimmedToken } : {}),
      });
      setEditingAccount(null);
      setName("");
      setToken("");
      onAccountsChanged();
      setActiveTab("list");
    } catch (err) {
      setVerifyMsg({ ok: false, title: "Gagal Memperbarui Akun", detail: errorMessage(err) });
    } finally {
      setSaving(false);
    }
  };

  const handleDelete = async (id: number, accName: string) => {
    if (!window.confirm(`Hapus akun "${accName}" dari RusTerm? Tunnel di Cloudflare tidak akan terhapus.`)) return;
    setDeletingId(id);
    try {
      await api.cfDeleteAccount(id);
      onAccountsChanged();
      if (editingAccount?.id === id) {
        setEditingAccount(null);
      }
      if (accounts.length <= 1) {
        setActiveTab("add");
      }
    } catch (err) {
      alert(errorMessage(err));
    } finally {
      setDeletingId(null);
    }
  };

  const handleCopyId = async (id: string) => {
    try {
      await navigator.clipboard.writeText(id);
      setCopiedAccId(id);
      setTimeout(() => setCopiedAccId(null), 1500);
    } catch {
      // ignore
    }
  };

  return (
    <Modal
      title="Kelola Akun & Token API Cloudflare"
      onClose={onClose}
      wide
      footer={
        <>
          <span className="spacer" />
          <button type="button" className="btn" onClick={onClose}>
            Tutup
          </button>
        </>
      }
    >
      {/* Enterprise Tabs */}
      <div className="backup-modal-tabs" style={{ marginBottom: "16px" }}>
        <button
          type="button"
          className={`tab-btn${activeTab === "list" ? " is-active" : ""}`}
          onClick={() => {
            setActiveTab("list");
            setEditingAccount(null);
            setVerifyMsg(null);
          }}
          disabled={accounts.length === 0}
        >
          <Icon name="cloud" size={14} /> <span>Akun Tersimpan ({accounts.length})</span>
        </button>
        <button
          type="button"
          className={`tab-btn${activeTab === "add" ? " is-active" : ""}`}
          onClick={() => {
            setActiveTab("add");
            setEditingAccount(null);
            setName("");
            setToken("");
            setVerifyMsg(null);
          }}
        >
          <Icon name="plus" size={14} /> <span>Tambah / Hubungkan Token</span>
        </button>
        {activeTab === "edit" && editingAccount && (
          <button
            type="button"
            className="tab-btn is-active"
            onClick={() => {
              setVerifyMsg(null);
            }}
          >
            <Icon name="edit" size={14} /> <span>Edit: {editingAccount.name}</span>
          </button>
        )}
      </div>

      {activeTab === "list" && (
        <div className="cf-account-view">
          <p className="backup-desc">
            Daftar akun Cloudflare yang terhubung ke RusTerm. Kredensial disimpan secara terenkripsi (AES-256)
            di database vault lokal.
          </p>

          <div className="cf-acc-cards">
            {accounts.map((acc) => (
              <div key={acc.id} className="cf-acc-card">
                <div className="cf-acc-card-icon">
                  <Icon name="cloud" size={20} />
                </div>
                <div className="cf-acc-card-content">
                  <div className="cf-acc-card-top">
                    <strong className="cf-acc-card-name">{acc.name}</strong>
                    {acc.account_name && acc.account_name !== acc.name && (
                      <span className="cf-acc-org-name">{acc.account_name}</span>
                    )}
                  </div>
                  <div className="cf-acc-card-meta">
                    <div className="cf-acc-meta-item">
                      <span className="cf-meta-label">Account ID:</span>
                      <code className="cf-code-badge">{acc.account_id}</code>
                      <button
                        type="button"
                        className="icon-btn-compact"
                        onClick={() => void handleCopyId(acc.account_id)}
                        title="Salin Account ID"
                      >
                        <Icon name={copiedAccId === acc.account_id ? "check" : "copy"} size={11} />
                      </button>
                    </div>
                    <div className="cf-acc-meta-item">
                      <span className="cf-meta-label">Token:</span>
                      <code className="cf-code-badge">{acc.token_masked}</code>
                    </div>
                  </div>
                </div>

                <div className="cf-acc-card-actions">
                  <button
                    type="button"
                    className="btn btn-small btn-secondary"
                    onClick={() => handleStartEdit(acc)}
                    title="Edit akun ini"
                  >
                    <Icon name="edit" size={13} /> Edit
                  </button>
                  <button
                    type="button"
                    className="btn btn-small btn-danger"
                    onClick={() => void handleDelete(acc.id, acc.name)}
                    disabled={deletingId === acc.id}
                    title="Hapus akun ini"
                  >
                    <Icon name="trash" size={13} /> Hapus
                  </button>
                </div>
              </div>
            ))}
          </div>

          <div style={{ marginTop: "16px", display: "flex", justifyContent: "flex-end" }}>
            <button
              type="button"
              className="btn btn-small btn-secondary"
              onClick={() => setActiveTab("add")}
            >
              <Icon name="plus" size={13} /> Tambah Akun Baru
            </button>
          </div>
        </div>
      )}

      {activeTab === "edit" && editingAccount && (
        <form className="backup-form" onSubmit={handleUpdateSave}>
          <div className="cf-guide-box">
            <div className="cf-guide-title">
              <Icon name="edit" size={14} /> Edit Akun Cloudflare
            </div>
            <p className="cf-guide-text">
              Ubah label nama akun atau perbarui API Token. Jika Anda tidak ingin mengganti API Token, biarkan kolom token Cloudflare kosong.
            </p>
            <div style={{ marginTop: "8px", fontSize: "12px", color: "var(--muted)", display: "flex", flexWrap: "wrap", gap: "10px" }}>
              <span>
                Account ID saat ini: <code className="cf-code-badge">{editingAccount.account_id}</code>
              </span>
              {editingAccount.account_name && (
                <span>
                  Org: <strong>{editingAccount.account_name}</strong>
                </span>
              )}
              <span>
                Token saat ini: <code className="cf-code-badge">{editingAccount.token_masked}</code>
              </span>
            </div>
          </div>

          <label className="form-field">
            <span>Label / Nama Akun:</span>
            <input
              type="text"
              placeholder="Contoh: Akun Utama, Proyek Kantor, VPS Pribadi"
              value={name}
              onChange={(e) => setName(e.target.value)}
              disabled={saving}
              required
            />
          </label>

          <label className="form-field">
            <span>Cloudflare API Token Baru (Opsional):</span>
            <div className="cf-token-input-group">
              <input
                type={showToken ? "text" : "password"}
                placeholder="Biarkan kosong jika tidak ingin mengubah token"
                value={token}
                onChange={(e) => setToken(e.target.value)}
                disabled={saving}
                className="cf-token-input"
              />
              <button
                type="button"
                className="btn btn-secondary cf-peek-btn"
                onClick={() => setShowToken(!showToken)}
                title={showToken ? "Sembunyikan Token" : "Tampilkan Token"}
              >
                {showToken ? "Sembunyikan" : "Lihat"}
              </button>
            </div>
          </label>

          {verifyMsg && (
            <div
              className={`notice ${verifyMsg.ok ? "good" : "bad"}`}
              style={{ marginTop: "12px", padding: "8px 12px" }}
            >
              <strong style={{ display: "block", fontSize: "12.5px" }}>{verifyMsg.title}</strong>
              {verifyMsg.detail && (
                <div style={{ fontSize: "11.5px", marginTop: "2px", opacity: 0.9 }}>
                  {verifyMsg.detail}
                </div>
              )}
            </div>
          )}

          <div className="cf-form-actions">
            <button
              type="button"
              className="btn btn-secondary"
              onClick={handleCancelEdit}
              disabled={saving}
            >
              Batal
            </button>
            <button
              type="button"
              className="btn btn-secondary"
              onClick={() => void handleTest()}
              disabled={testing || saving || !token.trim()}
              title={!token.trim() ? "Masukkan token baru untuk menguji koneksi" : "Uji token baru"}
            >
              <Icon name="bolt" size={14} /> {testing ? "Menguji Koneksi..." : "Tes Koneksi Baru"}
            </button>
            <button
              type="submit"
              className="btn btn-primary"
              disabled={saving || !name.trim()}
            >
              <Icon name="check" size={14} /> {saving ? "Menyimpan..." : "Simpan Perubahan"}
            </button>
          </div>
        </form>
      )}

      {activeTab === "add" && (
        <form className="backup-form" onSubmit={handleSave}>
          <div className="cf-guide-box">
            <div className="cf-guide-title">
              <Icon name="bolt" size={14} /> Panduan Token API Cloudflare
            </div>
            <p className="cf-guide-text">
              Gunakan <strong>Account API Token</strong> (prefix <code>cfat_...</code>) atau API Token dengan izin:
            </p>
            <ul className="cf-guide-list">
              <li>
                <strong>Account &gt; Cloudflare Tunnel:</strong> <code>Read</code> &amp; <code>Edit</code>
              </li>
              <li>
                <strong>Zone &gt; DNS:</strong> <code>Read</code> &amp; <code>Edit</code>
              </li>
            </ul>
          </div>

          <label className="form-field">
            <span>Label / Nama Akun:</span>
            <input
              type="text"
              placeholder="Contoh: Akun Utama, Proyek Kantor, VPS Pribadi"
              value={name}
              onChange={(e) => setName(e.target.value)}
              disabled={saving}
            />
          </label>

          <label className="form-field">
            <span>Cloudflare API Token:</span>
            <div className="cf-token-input-group">
              <input
                type={showToken ? "text" : "password"}
                placeholder="cfat_..."
                value={token}
                onChange={(e) => setToken(e.target.value)}
                disabled={saving}
                required
                className="cf-token-input"
              />
              <button
                type="button"
                className="btn btn-secondary cf-peek-btn"
                onClick={() => setShowToken(!showToken)}
                title={showToken ? "Sembunyikan Token" : "Tampilkan Token"}
              >
                {showToken ? "Sembunyikan" : "Lihat"}
              </button>
            </div>
          </label>

          {verifyMsg && (
            <div
              className={`notice ${verifyMsg.ok ? "good" : "bad"}`}
              style={{ marginTop: "12px", padding: "8px 12px" }}
            >
              <strong style={{ display: "block", fontSize: "12.5px" }}>{verifyMsg.title}</strong>
              {verifyMsg.detail && (
                <div style={{ fontSize: "11.5px", marginTop: "2px", opacity: 0.9 }}>
                  {verifyMsg.detail}
                </div>
              )}
            </div>
          )}

          <div className="cf-form-actions">
            <button
              type="button"
              className="btn btn-secondary"
              onClick={() => void handleTest()}
              disabled={testing || saving || !token.trim()}
            >
              <Icon name="bolt" size={14} /> {testing ? "Menguji Koneksi..." : "Tes Koneksi"}
            </button>
            <button
              type="submit"
              className="btn btn-primary"
              disabled={saving || !token.trim()}
            >
              <Icon name="check" size={14} /> {saving ? "Menyimpan..." : "Simpan Akun"}
            </button>
          </div>
        </form>
      )}
    </Modal>
  );
}
