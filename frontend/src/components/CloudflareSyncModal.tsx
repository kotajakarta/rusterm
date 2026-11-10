import { useEffect, useState, type FormEvent } from "react";
import { api, errorMessage } from "../api";
import {
  formatSyncStats,
  formatSyncTime,
  getDefaultDeviceName,
  validateSyncPin,
} from "../syncLogic";
import type { CFAccount, CFSyncStatusOut } from "../types";
import { Icon } from "./Icon";
import { Modal } from "./Modal";

// Keep active sync PIN in memory for the current session
let sessionSyncPin = "";

interface CloudflareSyncModalProps {
  onClose: () => void;
  onSyncSuccess: () => void;
  onOpenAccountModal?: () => void;
}

export function CloudflareSyncModal({
  onClose,
  onSyncSuccess,
  onOpenAccountModal,
}: CloudflareSyncModalProps) {
  const [accounts, setAccounts] = useState<CFAccount[]>([]);
  const [selectedAccountId, setSelectedAccountId] = useState<number | null>(null);
  const [loadingAccounts, setLoadingAccounts] = useState(true);

  const [status, setStatus] = useState<CFSyncStatusOut | null>(null);
  const [loadingStatus, setLoadingStatus] = useState(false);

  const [pin, setPin] = useState(sessionSyncPin);
  const [showPin, setShowPin] = useState(false);
  const [deviceName, setDeviceName] = useState(getDefaultDeviceName());

  const [syncing, setSyncing] = useState(false);
  const [syncError, setSyncError] = useState<string | null>(null);
  const [syncSuccessMsg, setSyncSuccessMsg] = useState<string | null>(null);

  const [showAdvanced, setShowAdvanced] = useState(false);
  const [replaceLocal, setReplaceLocal] = useState(false);

  // Load Cloudflare accounts on mount
  useEffect(() => {
    let active = true;
    setLoadingAccounts(true);
    api
      .cfListAccounts()
      .then((accs) => {
        if (!active) return;
        setAccounts(accs);
        if (accs.length > 0) {
          const firstId = accs[0].id;
          setSelectedAccountId(firstId);
          fetchStatus(firstId);
        }
      })
      .catch((err) => {
        if (active) setSyncError(errorMessage(err));
      })
      .finally(() => {
        if (active) setLoadingAccounts(false);
      });

    return () => {
      active = false;
    };
  }, []);

  const fetchStatus = async (accId?: number) => {
    setLoadingStatus(true);
    setSyncError(null);
    try {
      const res = await api.cfGetSyncStatus(accId);
      setStatus(res);
    } catch (err) {
      setSyncError(errorMessage(err));
    } finally {
      setLoadingStatus(false);
    }
  };

  const handleAccountChange = (id: number) => {
    setSelectedAccountId(id);
    fetchStatus(id);
  };

  const handleSyncNow = async (e?: FormEvent) => {
    if (e) e.preventDefault();
    const pinErr = validateSyncPin(pin);
    if (pinErr) {
      setSyncError(pinErr);
      return;
    }

    setSyncing(true);
    setSyncError(null);
    setSyncSuccessMsg(null);

    try {
      const res = await api.cfSyncNow({
        account_id: selectedAccountId,
        pin,
        device_name: deviceName.trim() || getDefaultDeviceName(),
      });
      sessionSyncPin = pin;
      const statsStr = formatSyncStats(res.stats);
      setSyncSuccessMsg(`✓ Sinkronisasi berhasil! ${statsStr}`);
      onSyncSuccess();
      if (selectedAccountId) fetchStatus(selectedAccountId);
    } catch (err) {
      setSyncError(errorMessage(err));
    } finally {
      setSyncing(false);
    }
  };

  const handlePush = async () => {
    const pinErr = validateSyncPin(pin);
    if (pinErr) {
      setSyncError(pinErr);
      return;
    }

    setSyncing(true);
    setSyncError(null);
    setSyncSuccessMsg(null);

    try {
      await api.cfSyncPush({
        account_id: selectedAccountId,
        pin,
        device_name: deviceName.trim() || getDefaultDeviceName(),
      });
      sessionSyncPin = pin;
      setSyncSuccessMsg("✓ Berhasil mengunggah (push) seluruh database lokal ke Cloudflare KV!");
      if (selectedAccountId) fetchStatus(selectedAccountId);
    } catch (err) {
      setSyncError(errorMessage(err));
    } finally {
      setSyncing(false);
    }
  };

  const handlePull = async () => {
    const pinErr = validateSyncPin(pin);
    if (pinErr) {
      setSyncError(pinErr);
      return;
    }

    setSyncing(true);
    setSyncError(null);
    setSyncSuccessMsg(null);

    try {
      const res = await api.cfSyncPull({
        account_id: selectedAccountId,
        pin,
        replace_local: replaceLocal,
      });
      if (!res.success) {
        setSyncError("Belum ada data vault di Cloudflare KV untuk diunduh.");
        return;
      }
      sessionSyncPin = pin;
      const statsStr = formatSyncStats(res.stats);
      setSyncSuccessMsg(`✓ Berhasil mengunduh (pull) data dari cloud! ${statsStr}`);
      onSyncSuccess();
      if (selectedAccountId) fetchStatus(selectedAccountId);
    } catch (err) {
      setSyncError(errorMessage(err));
    } finally {
      setSyncing(false);
    }
  };

  return (
    <Modal
      title="Sinkronisasi Cloudflare Workers KV"
      onClose={onClose}
      footer={
        <>
          <span className="spacer" />
          <button type="button" className="btn" onClick={onClose} disabled={syncing}>
            Tutup
          </button>
        </>
      }
    >
      {loadingAccounts ? (
        <div style={{ padding: "24px 0", textAlign: "center", color: "var(--muted)" }}>
          Memeriksa konfigurasi Cloudflare...
        </div>
      ) : accounts.length === 0 ? (
        <div style={{ textAlign: "center", padding: "16px 0" }}>
          <div style={{ fontSize: "32px", marginBottom: "8px" }}>☁️</div>
          <h4 style={{ margin: "0 0 8px 0" }}>Belum Ada Akun Cloudflare</h4>
          <p className="backup-desc" style={{ maxWidth: "420px", margin: "0 auto 16px auto" }}>
            Sinkronisasi antar perangkat menggunakan penyimpanan Workers KV terenkripsi (Zero-Knowledge AES-256-GCM) pada akun Cloudflare Free Tier Anda.
            <br />
            Silakan hubungkan akun Cloudflare terlebih dahulu melalui tombol <strong>+</strong> pada bagian <strong>Cloudflare Tunnels</strong> di sidebar.
          </p>
          {onOpenAccountModal && (
            <button
              type="button"
              className="btn btn-primary"
              onClick={() => {
                onClose();
                onOpenAccountModal();
              }}
            >
              <Icon name="plus" size={14} /> Hubungkan Akun Cloudflare
            </button>
          )}
        </div>
      ) : (
        <form className="backup-form" onSubmit={handleSyncNow}>
          {/* Cloud Status Card */}
          <div
            style={{
              padding: "12px",
              background: "var(--bg-card)",
              border: "1px solid var(--line)",
              borderRadius: "var(--r-md)",
            }}
          >
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "6px" }}>
              <div style={{ display: "flex", alignItems: "center", gap: "6px", fontWeight: 600, fontSize: "13px" }}>
                <Icon name="cloud" size={16} /> Status Cloud Vault
              </div>
              <button
                type="button"
                className="btn btn-sm btn-ghost"
                onClick={() => selectedAccountId && fetchStatus(selectedAccountId)}
                disabled={loadingStatus || syncing}
                title="Muat ulang status remote"
              >
                <Icon name="refresh" size={13} />
              </button>
            </div>

            {loadingStatus ? (
              <p style={{ margin: 0, fontSize: "12px", color: "var(--muted)" }}>Memeriksa Cloudflare KV...</p>
            ) : status?.has_remote && status.remote_metadata ? (
              <div style={{ fontSize: "12px", display: "flex", flexDirection: "column", gap: "4px" }}>
                <div>
                  <span style={{ color: "var(--muted)" }}>Device terakhir: </span>
                  <strong>{status.remote_metadata.device_name}</strong>
                </div>
                <div>
                  <span style={{ color: "var(--muted)" }}>Waktu sinkron: </span>
                  <span>{formatSyncTime(status.remote_metadata.updated_at)}</span>
                </div>
                <div>
                  <span style={{ color: "var(--muted)" }}>Data di Cloud: </span>
                  <span>
                    <strong>{status.remote_metadata.servers_count}</strong> server, <strong>{status.remote_metadata.snippets_count}</strong> snippet
                  </span>
                </div>
              </div>
            ) : (
              <p style={{ margin: 0, fontSize: "12px", color: "var(--muted)" }}>
                Belum ada vault di Cloudflare KV. Klik <strong>Sinkronkan Sekarang</strong> untuk mengunggah konfigurasi lokal pertama kali.
              </p>
            )}
          </div>

          {/* Account Selector (if multiple) */}
          {accounts.length > 1 && (
            <div className="form-group">
              <label className="form-label" htmlFor="cf-sync-account">
                Pilih Akun Cloudflare
              </label>
              <select
                id="cf-sync-account"
                className="form-control"
                value={selectedAccountId ?? ""}
                onChange={(e) => handleAccountChange(Number(e.target.value))}
                disabled={syncing}
              >
                {accounts.map((acc) => (
                  <option key={acc.id} value={acc.id}>
                    {acc.name} ({acc.account_name || acc.account_id})
                  </option>
                ))}
              </select>
            </div>
          )}

          {/* Device Name Field */}
          <div className="form-group">
            <label className="form-label" htmlFor="cf-sync-device">
              Nama Perangkat Ini
            </label>
            <input
              id="cf-sync-device"
              type="text"
              className="form-control"
              value={deviceName}
              onChange={(e) => setDeviceName(e.target.value)}
              placeholder="Contoh: Linux Workstation, HP Android"
              disabled={syncing}
              maxLength={64}
            />
          </div>

          {/* PIN Input with Zero-Knowledge Note */}
          <div className="form-group">
            <label className="form-label" htmlFor="cf-sync-pin">
              PIN Sinkronisasi (E2EE)
            </label>
            <div style={{ display: "flex", gap: "8px" }}>
              <input
                id="cf-sync-pin"
                type={showPin ? "text" : "password"}
                className="form-control"
                value={pin}
                onChange={(e) => setPin(e.target.value)}
                placeholder="Masukkan PIN enkripsi sync"
                disabled={syncing}
                style={{ flex: 1 }}
              />
              <button
                type="button"
                className="btn btn-secondary"
                onClick={() => setShowPin(!showPin)}
                title={showPin ? "Sembunyikan PIN" : "Tampilkan PIN"}
              >
                {showPin ? "Sembunyikan" : "Tampilkan"}
              </button>
            </div>
            <p className="backup-desc" style={{ marginTop: "4px" }}>
              Semua data dienkripsi dengan <strong>AES-256-GCM</strong> sebelum dikirim. Cloudflare tidak dapat membaca server atau sandi Anda.
            </p>
          </div>

          {/* Error Message */}
          {syncError && (
            <div
              style={{
                padding: "8px 12px",
                borderRadius: "var(--r-sm)",
                background: "rgba(239, 68, 68, 0.15)",
                border: "1px solid var(--danger)",
                color: "var(--danger)",
                fontSize: "12px",
              }}
            >
              {syncError}
            </div>
          )}

          {/* Success Message */}
          {syncSuccessMsg && (
            <div
              style={{
                padding: "8px 12px",
                borderRadius: "var(--r-sm)",
                background: "rgba(34, 197, 94, 0.15)",
                border: "1px solid var(--success)",
                color: "var(--success)",
                fontSize: "12px",
              }}
            >
              {syncSuccessMsg}
            </div>
          )}

          {/* Main Action: Two-way sync */}
          <div style={{ marginTop: "4px" }}>
            <button
              type="submit"
              className="btn btn-primary"
              style={{ width: "100%", padding: "10px", fontSize: "14px", display: "flex", justifyContent: "center", alignItems: "center", gap: "8px" }}
              disabled={syncing || !pin}
            >
              <Icon name="refresh" size={16} />
              {syncing ? "Sedang Menyinkronkan..." : "Sinkronkan Sekarang (Dua Arah)"}
            </button>
          </div>

          {/* Advanced toggle */}
          <div style={{ marginTop: "8px", borderTop: "1px solid var(--line)", paddingTop: "8px" }}>
            <button
              type="button"
              className="btn btn-sm btn-ghost"
              style={{ width: "100%", justifyContent: "space-between" }}
              onClick={() => setShowAdvanced(!showAdvanced)}
            >
              <span>Opsi Sinkronisasi Terarah (Push / Pull)</span>
              <Icon name={showAdvanced ? "chevron" : "chevronLeft"} size={13} />
            </button>

            {showAdvanced && (
              <div style={{ marginTop: "10px", display: "flex", flexDirection: "column", gap: "10px" }}>
                <div style={{ display: "flex", gap: "8px" }}>
                  <button
                    type="button"
                    className="btn btn-secondary"
                    style={{ flex: 1, display: "flex", alignItems: "center", justifyContent: "center", gap: "6px" }}
                    onClick={handlePush}
                    disabled={syncing || !pin}
                    title="Unggah langsung data lokal ke Cloudflare KV (menimpa remote)"
                  >
                    <Icon name="upload" size={14} /> Unggah Saja (Push)
                  </button>
                  <button
                    type="button"
                    className="btn btn-secondary"
                    style={{ flex: 1, display: "flex", alignItems: "center", justifyContent: "center", gap: "6px" }}
                    onClick={handlePull}
                    disabled={syncing || !pin}
                    title="Unduh data dari Cloudflare KV dan gabungkan ke lokal"
                  >
                    <Icon name="download" size={14} /> Unduh Saja (Pull)
                  </button>
                </div>

                <label style={{ display: "flex", alignItems: "center", gap: "6px", fontSize: "12px", color: "var(--muted)", cursor: "pointer" }}>
                  <input
                    type="checkbox"
                    checked={replaceLocal}
                    onChange={(e) => setReplaceLocal(e.target.checked)}
                    disabled={syncing}
                  />
                  <span>Ganti seluruh data lokal saat Unduh (Replace Local Store)</span>
                </label>
              </div>
            )}
          </div>
        </form>
      )}
    </Modal>
  );
}
