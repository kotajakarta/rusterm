import { useEffect, useState } from "react";
import { api, errorMessage } from "../api";
import type { TailscaleDevice, TailscaleDeviceRoutes } from "../types";
import { Icon } from "./Icon";
import { Modal } from "./Modal";

interface Props {
  accountId: number;
  device: TailscaleDevice;
  onClose: () => void;
  onDeviceUpdated: () => void;
}

export function TailscaleDeviceModal({ accountId, device: initialDevice, onClose, onDeviceUpdated }: Props) {
  const [device, setDevice] = useState<TailscaleDevice>(initialDevice);
  const [loading, setLoading] = useState(false);
  const [updating, setUpdating] = useState(false);
  const [routes, setRoutes] = useState<TailscaleDeviceRoutes>({ advertisedRoutes: [], enabledRoutes: [] });
  const [selectedRoutes, setSelectedRoutes] = useState<string[]>([]);
  const [routesLoading, setRoutesLoading] = useState(false);
  const [routesSaving, setRoutesSaving] = useState(false);

  const [tagInput, setTagInput] = useState("");
  const [tags, setTags] = useState<string[]>(initialDevice.tags || []);
  const [tagsSaving, setTagsSaving] = useState(false);

  const [confirmDelete, setConfirmDelete] = useState(false);
  const [deleting, setDeleting] = useState(false);

  const [message, setMessage] = useState<{ ok: boolean; text: string } | null>(null);
  const [copiedIp, setCopiedIp] = useState<string | null>(null);

  // Load fresh device details & routes
  const loadFreshData = async () => {
    setLoading(true);
    setRoutesLoading(true);
    try {
      const [freshDev, freshRoutes] = await Promise.all([
        api.tailscaleGetDevice(accountId, device.id),
        api.tailscaleGetDeviceRoutes(accountId, device.id).catch(() => ({
          advertisedRoutes: device.advertisedRoutes || [],
          enabledRoutes: device.enabledRoutes || [],
        })),
      ]);
      setDevice(freshDev);
      setTags(freshDev.tags || []);
      setRoutes(freshRoutes);
      setSelectedRoutes(freshRoutes.enabledRoutes || []);
    } catch (err) {
      setMessage({ ok: false, text: `Gagal memuat data terbaru: ${errorMessage(err)}` });
    } finally {
      setLoading(false);
      setRoutesLoading(false);
    }
  };

  useEffect(() => {
    void loadFreshData();
  }, [accountId, device.id]);

  const copyToClipboard = async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopiedIp(text);
      setTimeout(() => setCopiedIp(null), 2000);
    } catch {
      // Fallback
    }
  };

  const handleToggleAuthorized = async () => {
    setUpdating(true);
    setMessage(null);
    try {
      const newStatus = !device.authorized;
      await api.tailscaleSetDeviceAuthorized(accountId, device.id, newStatus);
      setDevice((prev) => ({ ...prev, authorized: newStatus }));
      setMessage({
        ok: true,
        text: newStatus
          ? "Mesin berhasil diotorisasi (Authorized)."
          : "Mesin telah dinonaktifkan (Unauthorized).",
      });
      onDeviceUpdated();
    } catch (err) {
      setMessage({ ok: false, text: `Gagal mengubah status otorisasi: ${errorMessage(err)}` });
    } finally {
      setUpdating(false);
    }
  };

  const handleToggleKeyExpiry = async () => {
    setUpdating(true);
    setMessage(null);
    try {
      const newDisabled = !device.keyExpiryDisabled;
      await api.tailscaleSetDeviceKeyExpiry(accountId, device.id, newDisabled);
      setDevice((prev) => ({ ...prev, keyExpiryDisabled: newDisabled }));
      setMessage({
        ok: true,
        text: newDisabled
          ? "Key Expiration berhasil dimatikan (Never Expire)."
          : "Key Expiration diaktifkan kembali.",
      });
      onDeviceUpdated();
    } catch (err) {
      setMessage({ ok: false, text: `Gagal mengubah key expiry: ${errorMessage(err)}` });
    } finally {
      setUpdating(false);
    }
  };

  const handleToggleRoute = (route: string) => {
    if (selectedRoutes.includes(route)) {
      setSelectedRoutes(selectedRoutes.filter((r) => r !== route));
    } else {
      setSelectedRoutes([...selectedRoutes, route]);
    }
  };

  const handleSaveRoutes = async () => {
    setRoutesSaving(true);
    setMessage(null);
    try {
      const res = await api.tailscaleSetDeviceRoutes(accountId, device.id, selectedRoutes);
      setRoutes(res);
      setSelectedRoutes(res.enabledRoutes);
      setMessage({ ok: true, text: "Pengaturan subnet / exit node routes berhasil disimpan." });
      onDeviceUpdated();
    } catch (err) {
      setMessage({ ok: false, text: `Gagal menyimpan routes: ${errorMessage(err)}` });
    } finally {
      setRoutesSaving(false);
    }
  };

  const handleAddTag = () => {
    const trimmed = tagInput.trim();
    if (!trimmed) return;
    const formatted = trimmed.startsWith("tag:") ? trimmed : `tag:${trimmed}`;
    if (!tags.includes(formatted)) {
      setTags([...tags, formatted]);
    }
    setTagInput("");
  };

  const handleRemoveTag = (t: string) => {
    setTags(tags.filter((item) => item !== t));
  };

  const handleSaveTags = async () => {
    setTagsSaving(true);
    setMessage(null);
    try {
      await api.tailscaleSetDeviceTags(accountId, device.id, tags);
      setDevice((prev) => ({ ...prev, tags }));
      setMessage({ ok: true, text: "Tags mesin berhasil disimpan." });
      onDeviceUpdated();
    } catch (err) {
      setMessage({ ok: false, text: `Gagal menyimpan tags: ${errorMessage(err)}` });
    } finally {
      setTagsSaving(false);
    }
  };

  const handleDeleteDevice = async () => {
    setDeleting(true);
    setMessage(null);
    try {
      await api.tailscaleDeleteDevice(accountId, device.id);
      onDeviceUpdated();
      onClose();
    } catch (err) {
      setMessage({ ok: false, text: `Gagal menghapus perangkat: ${errorMessage(err)}` });
      setDeleting(false);
    }
  };

  const ipv4 = device.addresses?.find((a) => a.includes(".")) || device.addresses?.[0] || "";
  const ipv6 = device.addresses?.find((a) => a.includes(":")) || "";

  return (
    <Modal title={`Pengaturan Mesin: ${device.hostname || device.name}`} onClose={onClose} wide>
      <div style={{ display: "flex", flexDirection: "column", gap: "1.2rem", padding: "0.2rem 0" }}>
        {message && (
          <div
            style={{
              padding: "0.6rem 0.8rem",
              borderRadius: "6px",
              fontSize: "0.85rem",
              backgroundColor: message.ok ? "rgba(34, 197, 94, 0.15)" : "rgba(239, 68, 68, 0.15)",
              border: `1px solid ${message.ok ? "rgba(34, 197, 94, 0.4)" : "rgba(239, 68, 68, 0.4)"}`,
            }}
          >
            {message.text}
          </div>
        )}

        {/* 1. Overview Information */}
        <div
          style={{
            background: "var(--bg-subtle, rgba(255,255,255,0.03))",
            borderRadius: "8px",
            padding: "1rem",
            border: "1px solid var(--border-color, rgba(255,255,255,0.08))",
          }}
        >
          <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start", marginBottom: "0.8rem" }}>
            <div>
              <div style={{ display: "flex", alignItems: "center", gap: "0.5rem" }}>
                <span
                  style={{
                    display: "inline-block",
                    width: "10px",
                    height: "10px",
                    borderRadius: "50%",
                    backgroundColor: device.online ? "#22c55e" : "#94a3b8",
                  }}
                  title={device.online ? "Online (Terhubung)" : "Offline"}
                />
                <h3 style={{ margin: 0, fontSize: "1.1rem" }}>{device.hostname || device.name}</h3>
                <span className="badge" style={{ fontSize: "0.75rem", textTransform: "capitalize" }}>
                  {device.os || "Unknown OS"}
                </span>
                {device.updateAvailable && (
                  <span className="badge warning" style={{ fontSize: "0.75rem" }}>
                    Update Tersedia
                  </span>
                )}
              </div>
              <div style={{ color: "var(--text-muted)", fontSize: "0.8rem", marginTop: "0.2rem" }}>
                FQDN: <code>{device.name}</code>
              </div>
            </div>
            <button
              type="button"
              className="btn sm"
              onClick={() => void loadFreshData()}
              disabled={loading}
              title="Refresh Data"
            >
              <Icon name="refresh" size={13} /> {loading ? "Menyegarkan..." : "Refresh"}
            </button>
          </div>

          <div
            style={{
              display: "grid",
              gridTemplateColumns: "repeat(auto-fit, minmax(200px, 1fr))",
              gap: "0.6rem",
              fontSize: "0.85rem",
            }}
          >
            <div>
              <span style={{ color: "var(--text-muted)" }}>Tailscale IPv4:</span>
              <div style={{ display: "flex", alignItems: "center", gap: "0.4rem", marginTop: "0.2rem" }}>
                <code>{ipv4 || "None"}</code>
                {ipv4 && (
                  <button
                    type="button"
                    className="icon-btn"
                    onClick={() => void copyToClipboard(ipv4)}
                    title="Salin IP"
                    style={{ padding: "2px" }}
                  >
                    <Icon name={copiedIp === ipv4 ? "check" : "copy"} size={13} />
                  </button>
                )}
              </div>
            </div>
            {ipv6 && (
              <div>
                <span style={{ color: "var(--text-muted)" }}>Tailscale IPv6:</span>
                <div style={{ display: "flex", alignItems: "center", gap: "0.4rem", marginTop: "0.2rem" }}>
                  <code style={{ fontSize: "0.75rem" }}>{ipv6}</code>
                </div>
              </div>
            )}
            <div>
              <span style={{ color: "var(--text-muted)" }}>Owner / Pengguna:</span>
              <div style={{ marginTop: "0.2rem" }}>{device.user || "N/A"}</div>
            </div>
            <div>
              <span style={{ color: "var(--text-muted)" }}>Versi Client:</span>
              <div style={{ marginTop: "0.2rem" }}>{device.clientVersion || "N/A"}</div>
            </div>
            <div>
              <span style={{ color: "var(--text-muted)" }}>Terakhir Terlihat:</span>
              <div style={{ marginTop: "0.2rem" }}>
                {device.online ? (
                  <span style={{ color: "#22c55e", fontWeight: 500 }}>Aktif Sekarang</span>
                ) : (
                  device.lastSeen || "N/A"
                )}
              </div>
            </div>
            <div>
              <span style={{ color: "var(--text-muted)" }}>Key Expiry:</span>
              <div style={{ marginTop: "0.2rem" }}>
                {device.keyExpiryDisabled ? (
                  <span style={{ color: "#22c55e" }}>Tidak pernah kadaluarsa (Disabled)</span>
                ) : (
                  <span>Kadaluarsa pada: {device.expires || "N/A"}</span>
                )}
              </div>
            </div>
          </div>
        </div>

        {/* 2. Device Settings & Controls */}
        <div style={{ display: "flex", flexDirection: "column", gap: "1rem" }}>
          <h4 style={{ margin: "0.2rem 0", fontSize: "0.95rem", borderBottom: "1px solid var(--border-color, rgba(255,255,255,0.08))", paddingBottom: "0.4rem" }}>
            Pengaturan Mesin
          </h4>

          {/* Setting A: Otorisasi Mesin */}
          <div
            style={{
              display: "flex",
              justifyContent: "space-between",
              alignItems: "center",
              padding: "0.8rem",
              borderRadius: "6px",
              background: "var(--bg-subtle, rgba(255,255,255,0.02))",
              border: "1px solid var(--border-color, rgba(255,255,255,0.06))",
            }}
          >
            <div>
              <div style={{ fontWeight: 600, fontSize: "0.9rem" }}>Status Otorisasi (Device Authorization)</div>
              <div style={{ color: "var(--text-muted)", fontSize: "0.8rem", marginTop: "0.2rem" }}>
                {device.authorized
                  ? "Mesin ini disetujui (Authorized) dan dapat terhubung ke jaringan Tailnet."
                  : "Mesin ini belum disetujui atau dikunci (Unauthorized)."}
              </div>
            </div>
            <button
              type="button"
              className={`btn sm ${device.authorized ? "danger" : "primary"}`}
              onClick={() => void handleToggleAuthorized()}
              disabled={updating}
            >
              {updating ? "Menyimpan..." : device.authorized ? "Tolak / Kunci (Deauthorize)" : "Setujui (Authorize)"}
            </button>
          </div>

          {/* Setting B: Key Expiry */}
          <div
            style={{
              display: "flex",
              justifyContent: "space-between",
              alignItems: "center",
              padding: "0.8rem",
              borderRadius: "6px",
              background: "var(--bg-subtle, rgba(255,255,255,0.02))",
              border: "1px solid var(--border-color, rgba(255,255,255,0.06))",
            }}
          >
            <div>
              <div style={{ fontWeight: 600, fontSize: "0.9rem" }}>Masa Berlaku Kunci (Key Expiration)</div>
              <div style={{ color: "var(--text-muted)", fontSize: "0.8rem", marginTop: "0.2rem" }}>
                {device.keyExpiryDisabled
                  ? "Kunci mesin ini tidak akan pernah kadaluarsa (Cocok untuk server / NAS)."
                  : "Kunci mesin mengikuti kebijakan rotasi berkala Tailscale."}
              </div>
            </div>
            <button
              type="button"
              className="btn sm"
              onClick={() => void handleToggleKeyExpiry()}
              disabled={updating}
            >
              {updating
                ? "Menyimpan..."
                : device.keyExpiryDisabled
                ? "Aktifkan Expiry Kunci"
                : "Matikan Expiry (Never Expire)"}
            </button>
          </div>

          {/* Setting C: Subnet & Exit Node Routes */}
          <div
            style={{
              padding: "0.8rem",
              borderRadius: "6px",
              background: "var(--bg-subtle, rgba(255,255,255,0.02))",
              border: "1px solid var(--border-color, rgba(255,255,255,0.06))",
            }}
          >
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "0.5rem" }}>
              <div>
                <div style={{ fontWeight: 600, fontSize: "0.9rem" }}>Subnet & Exit Node Routing</div>
                <div style={{ color: "var(--text-muted)", fontSize: "0.8rem" }}>
                  Aktifkan route jaringan yang diiklankan oleh perangkat ini.
                </div>
              </div>
              {routes.advertisedRoutes && routes.advertisedRoutes.length > 0 && (
                <button
                  type="button"
                  className="btn primary sm"
                  onClick={() => void handleSaveRoutes()}
                  disabled={routesSaving || routesLoading}
                >
                  {routesSaving ? "Menyimpan..." : "Simpan Routes"}
                </button>
              )}
            </div>

            {routesLoading ? (
              <div style={{ fontSize: "0.8rem", color: "var(--text-muted)" }}>Memuat rute jaringan...</div>
            ) : !routes.advertisedRoutes || routes.advertisedRoutes.length === 0 ? (
              <div style={{ fontSize: "0.8rem", color: "var(--text-muted)", fontStyle: "italic" }}>
                Perangkat ini tidak mengiklankan subnet route atau exit node (Gunakan <code>tailscale up --advertise-routes=...</code> atau <code>--advertise-exit-node</code> pada mesin).
              </div>
            ) : (
              <div style={{ display: "flex", flexDirection: "column", gap: "0.4rem", marginTop: "0.4rem" }}>
                {routes.advertisedRoutes.map((r) => {
                  const isExitNode = r === "0.0.0.0/0" || r === "::/0";
                  const isChecked = selectedRoutes.includes(r);
                  return (
                    <label
                      key={r}
                      style={{
                        display: "flex",
                        alignItems: "center",
                        gap: "0.5rem",
                        fontSize: "0.85rem",
                        cursor: "pointer",
                      }}
                    >
                      <input
                        type="checkbox"
                        checked={isChecked}
                        onChange={() => handleToggleRoute(r)}
                      />
                      <code>{r}</code>
                      {isExitNode && (
                        <span className="badge" style={{ fontSize: "0.7rem", backgroundColor: "rgba(59, 130, 246, 0.2)", color: "#60a5fa" }}>
                          Exit Node
                        </span>
                      )}
                    </label>
                  );
                })}
              </div>
            )}
          </div>

          {/* Setting D: Machine Tags */}
          <div
            style={{
              padding: "0.8rem",
              borderRadius: "6px",
              background: "var(--bg-subtle, rgba(255,255,255,0.02))",
              border: "1px solid var(--border-color, rgba(255,255,255,0.06))",
            }}
          >
            <div style={{ fontWeight: 600, fontSize: "0.9rem", marginBottom: "0.4rem" }}>Tags Mesin</div>
            <div style={{ display: "flex", flexWrap: "wrap", gap: "0.4rem", marginBottom: "0.6rem" }}>
              {tags.length === 0 ? (
                <span style={{ color: "var(--text-muted)", fontSize: "0.8rem" }}>Belum ada tag yang disematkan.</span>
              ) : (
                tags.map((t) => (
                  <span
                    key={t}
                    className="badge"
                    style={{
                      display: "flex",
                      alignItems: "center",
                      gap: "0.3rem",
                      fontSize: "0.75rem",
                    }}
                  >
                    {t}
                    <button
                      type="button"
                      style={{ background: "none", border: "none", color: "inherit", cursor: "pointer", padding: 0 }}
                      onClick={() => handleRemoveTag(t)}
                      title="Hapus Tag"
                    >
                      ×
                    </button>
                  </span>
                ))
              )}
            </div>

            <div style={{ display: "flex", gap: "0.4rem" }}>
              <input
                type="text"
                placeholder="tag:server, tag:prod"
                value={tagInput}
                onChange={(e) => setTagInput(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") {
                    e.preventDefault();
                    handleAddTag();
                  }
                }}
                style={{ flex: 1, fontSize: "0.85rem" }}
              />
              <button type="button" className="btn sm" onClick={handleAddTag} disabled={!tagInput.trim()}>
                + Tambah
              </button>
              <button
                type="button"
                className="btn primary sm"
                onClick={() => void handleSaveTags()}
                disabled={tagsSaving}
              >
                {tagsSaving ? "Menyimpan..." : "Simpan Tags"}
              </button>
            </div>
          </div>

          {/* Setting E: Danger Zone - Delete Device */}
          <div
            style={{
              padding: "0.8rem",
              borderRadius: "6px",
              background: "rgba(239, 68, 68, 0.05)",
              border: "1px solid rgba(239, 68, 68, 0.2)",
            }}
          >
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
              <div>
                <div style={{ fontWeight: 600, fontSize: "0.9rem", color: "#ef4444" }}>
                  Hapus Mesin dari Tailnet
                </div>
                <div style={{ color: "var(--text-muted)", fontSize: "0.8rem" }}>
                  Menghapus perangkat ini secara permanen dari jaringan Tailscale Tailnet Anda.
                </div>
              </div>
              {confirmDelete ? (
                <div style={{ display: "flex", gap: "0.4rem" }}>
                  <button
                    type="button"
                    className="btn danger sm"
                    onClick={() => void handleDeleteDevice()}
                    disabled={deleting}
                  >
                    {deleting ? "Menghapus..." : "Konfirmasi Hapus"}
                  </button>
                  <button
                    type="button"
                    className="btn sm"
                    onClick={() => setConfirmDelete(false)}
                    disabled={deleting}
                  >
                    Batal
                  </button>
                </div>
              ) : (
                <button
                  type="button"
                  className="btn danger sm"
                  onClick={() => setConfirmDelete(true)}
                >
                  <Icon name="trash" size={13} /> Hapus Perangkat
                </button>
              )}
            </div>
          </div>
        </div>
      </div>
    </Modal>
  );
}
