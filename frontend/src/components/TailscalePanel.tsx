import { useEffect, useState } from "react";
import { api, errorMessage } from "../api";
import { usePersistentFlag } from "../storage";
import type { TailscaleAccount, TailscaleDevice } from "../types";
import { Icon } from "./Icon";
import { TailscaleAccountModal } from "./TailscaleAccountModal";
import { TailscaleDeviceModal } from "./TailscaleDeviceModal";

interface Props {
  onQuickSSH?: (host: string, name: string) => void;
}

export function TailscalePanel({ onQuickSSH }: Props) {
  const [collapsed, setCollapsed] = usePersistentFlag("termdeck.tailscale.collapsed", false);
  const [accounts, setAccounts] = useState<TailscaleAccount[]>([]);
  const [selectedAccountId, setSelectedAccountId] = useState<number | null>(null);
  const [devices, setDevices] = useState<TailscaleDevice[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [search, setSearch] = useState("");

  const [showAccountModal, setShowAccountModal] = useState(false);
  const [selectedDevice, setSelectedDevice] = useState<TailscaleDevice | null>(null);
  const [copiedIp, setCopiedIp] = useState<string | null>(null);

  const loadAccounts = async () => {
    try {
      const accs = await api.tailscaleListAccounts();
      setAccounts(accs);
      if (accs.length > 0 && (selectedAccountId === null || !accs.some((a) => a.id === selectedAccountId))) {
        setSelectedAccountId(accs[0].id);
      }
    } catch (err) {
      setError(errorMessage(err));
    }
  };

  useEffect(() => {
    void loadAccounts();
  }, []);

  const loadDevices = async () => {
    if (selectedAccountId === null) {
      setDevices([]);
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const devs = await api.tailscaleListDevices(selectedAccountId);
      setDevices(devs);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    if (selectedAccountId !== null) {
      void loadDevices();
    }
  }, [selectedAccountId]);

  const copyToClipboard = async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopiedIp(text);
      setTimeout(() => setCopiedIp(null), 1500);
    } catch {
      // ignore
    }
  };

  const activeAccount = accounts.find((a) => a.id === selectedAccountId);

  const filteredDevices = devices.filter((d) => {
    if (!search.trim()) return true;
    const q = search.toLowerCase();
    const nameMatch = d.name.toLowerCase().includes(q) || d.hostname.toLowerCase().includes(q);
    const ipMatch = d.addresses?.some((a) => a.toLowerCase().includes(q));
    const userMatch = d.user?.toLowerCase().includes(q);
    const osMatch = d.os?.toLowerCase().includes(q);
    return nameMatch || ipMatch || userMatch || osMatch;
  });

  const onlineCount = devices.filter((d) => d.online).length;

  return (
    <section className={`cf-section${collapsed ? " is-collapsed" : ""}`}>
      <header className="section-head">
        <button
          type="button"
          className="section-toggle"
          onClick={() => setCollapsed(!collapsed)}
          aria-expanded={!collapsed}
          title={collapsed ? "Tampilkan Tailscale" : "Sembunyikan Tailscale"}
        >
          <span className={`chev-icon${collapsed ? "" : " is-open"}`}>
            <Icon name="chevron" size={13} />
          </span>
          <h3>
            Tailscale
            {devices.length > 0 && (
              <span className="cf-count-tag" title={`${onlineCount} online dari ${devices.length} mesin`}>
                {onlineCount}/{devices.length}
              </span>
            )}
          </h3>
        </button>
        <div className="cf-head-actions">
          {activeAccount && !collapsed && (
            <button
              type="button"
              className="icon-btn"
              onClick={() => void loadDevices()}
              disabled={loading}
              title="Refresh daftar mesin"
              aria-label="Refresh daftar mesin"
            >
              <Icon name="refresh" size={13} />
            </button>
          )}
          <button
            type="button"
            className="icon-btn"
            onClick={() => setShowAccountModal(true)}
            aria-label="Kelola Akun Tailscale"
            title="Kelola Akun Tailscale"
          >
            <Icon name="key" size={13} />
          </button>
        </div>
      </header>

      {!collapsed && (
        <div className="cf-body">
          {error && <div className="cf-error">{error}</div>}

          {accounts.length === 0 ? (
            <div className="cf-empty-box">
              <p>Belum ada akun Tailscale terhubung.</p>
              <button
                type="button"
                className="btn primary sm"
                onClick={() => setShowAccountModal(true)}
              >
                + Tambah Akun Tailscale
              </button>
            </div>
          ) : (
            <>
              {accounts.length > 1 && (
                <div className="cf-account-selector" style={{ marginBottom: "0.5rem" }}>
                  <select
                    value={selectedAccountId ?? ""}
                    onChange={(e) => setSelectedAccountId(Number(e.target.value))}
                    className="select-input"
                    style={{ width: "100%", fontSize: "0.8rem", padding: "3px 6px" }}
                  >
                    {accounts.map((acc) => (
                      <option key={acc.id} value={acc.id}>
                        {acc.name} ({acc.tailnet})
                      </option>
                    ))}
                  </select>
                </div>
              )}

              {devices.length > 4 && (
                <div style={{ marginBottom: "0.5rem" }}>
                  <input
                    type="text"
                    placeholder="Cari mesin, IP, atau OS..."
                    value={search}
                    onChange={(e) => setSearch(e.target.value)}
                    style={{ width: "100%", fontSize: "0.78rem", padding: "3px 6px" }}
                  />
                </div>
              )}

              {loading ? (
                <div className="cf-loading" style={{ padding: "0.5rem 0", fontSize: "0.8rem" }}>
                  Memuat daftar mesin Tailnet...
                </div>
              ) : filteredDevices.length === 0 ? (
                <div className="cf-empty" style={{ fontSize: "0.8rem", color: "var(--text-muted)" }}>
                  {devices.length === 0 ? "Tidak ada mesin di Tailnet ini." : "Tidak ada mesin yang cocok dengan filter."}
                </div>
              ) : (
                <div className="cf-routes-list" style={{ display: "flex", flexDirection: "column", gap: "0.35rem" }}>
                  {filteredDevices.map((dev) => {
                    const ipv4 = dev.addresses?.find((a) => a.includes(".")) || dev.addresses?.[0] || "";
                    const isExitNode = dev.enabledRoutes?.some((r) => r === "0.0.0.0/0" || r === "::/0");
                    const hasSubnets = dev.enabledRoutes?.some((r) => r !== "0.0.0.0/0" && r !== "::/0");

                    return (
                      <div
                        key={dev.id}
                        className="cf-route-item"
                        style={{
                          display: "flex",
                          alignItems: "center",
                          justifyContent: "space-between",
                          padding: "0.4rem 0.5rem",
                          borderRadius: "4px",
                          background: "var(--bg-subtle, rgba(255,255,255,0.02))",
                          border: "1px solid var(--border-color, rgba(255,255,255,0.05))",
                        }}
                      >
                        <div style={{ minWidth: 0, flex: 1 }}>
                          <div style={{ display: "flex", alignItems: "center", gap: "0.35rem" }}>
                            <span
                              style={{
                                display: "inline-block",
                                width: "7px",
                                height: "7px",
                                borderRadius: "50%",
                                flexShrink: 0,
                                backgroundColor: dev.online ? "#22c55e" : "#64748b",
                              }}
                              title={dev.online ? "Online" : `Offline (Last seen: ${dev.lastSeen || "Unknown"})`}
                            />
                            <span
                              style={{
                                fontWeight: 500,
                                fontSize: "0.82rem",
                                overflow: "hidden",
                                textOverflow: "ellipsis",
                                whiteSpace: "nowrap",
                              }}
                              title={dev.name}
                            >
                              {dev.hostname || dev.name}
                            </span>
                            <span
                              style={{
                                fontSize: "0.68rem",
                                color: "var(--text-muted)",
                                textTransform: "capitalize",
                              }}
                            >
                              ({dev.os})
                            </span>
                          </div>

                          <div
                            style={{
                              display: "flex",
                              alignItems: "center",
                              gap: "0.3rem",
                              marginTop: "0.15rem",
                              fontSize: "0.75rem",
                              color: "var(--text-muted)",
                            }}
                          >
                            <code style={{ fontSize: "0.74rem" }}>{ipv4 || "No IPv4"}</code>
                            {ipv4 && (
                              <button
                                type="button"
                                className="icon-btn"
                                onClick={() => void copyToClipboard(ipv4)}
                                title="Salin IP"
                                style={{ padding: "1px" }}
                              >
                                <Icon name={copiedIp === ipv4 ? "check" : "copy"} size={11} />
                              </button>
                            )}

                            {isExitNode && (
                              <span
                                className="badge"
                                style={{
                                  fontSize: "0.65rem",
                                  padding: "1px 4px",
                                  backgroundColor: "rgba(59, 130, 246, 0.2)",
                                  color: "#60a5fa",
                                }}
                                title="Exit Node Aktif"
                              >
                                Exit
                              </span>
                            )}
                            {hasSubnets && (
                              <span
                                className="badge"
                                style={{
                                  fontSize: "0.65rem",
                                  padding: "1px 4px",
                                  backgroundColor: "rgba(168, 85, 247, 0.2)",
                                  color: "#c084fc",
                                }}
                                title="Subnet Router Aktif"
                              >
                                Subnet
                              </span>
                            )}
                          </div>
                        </div>

                        <div style={{ display: "flex", alignItems: "center", gap: "0.2rem", flexShrink: 0 }}>
                          {ipv4 && onQuickSSH && (
                            <button
                              type="button"
                              className="icon-btn"
                              onClick={() => onQuickSSH(ipv4, dev.hostname || dev.name)}
                              title="Buka / Tambah SSH ke mesin ini"
                              aria-label="Koneksi SSH"
                            >
                              <Icon name="terminal" size={13} />
                            </button>
                          )}
                          <button
                            type="button"
                            className="icon-btn"
                            onClick={() => setSelectedDevice(dev)}
                            title="Pengaturan Mesin (Otorisasi, Routes, Key Expiry, Tags, Hapus)"
                            aria-label="Pengaturan Mesin"
                          >
                            <Icon name="sliders" size={13} />
                          </button>
                        </div>
                      </div>
                    );
                  })}
                </div>
              )}
            </>
          )}
        </div>
      )}

      {showAccountModal && (
        <TailscaleAccountModal
          accounts={accounts}
          onAccountsChanged={() => {
            void loadAccounts();
          }}
          onClose={() => setShowAccountModal(false)}
        />
      )}

      {selectedDevice && selectedAccountId !== null && (
        <TailscaleDeviceModal
          accountId={selectedAccountId}
          device={selectedDevice}
          onClose={() => setSelectedDevice(null)}
          onDeviceUpdated={() => {
            void loadDevices();
          }}
        />
      )}
    </section>
  );
}
