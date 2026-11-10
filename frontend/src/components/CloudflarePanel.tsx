import { useEffect, useState } from "react";
import { api, errorMessage } from "../api";
import { usePersistentFlag } from "../storage";
import type { CFAccount, CFRoute, CFTunnel, CFZone } from "../types";
import { CloudflareAccountModal } from "./CloudflareAccountModal";
import { CloudflareRouteModal } from "./CloudflareRouteModal";
import { Icon } from "./Icon";

export function CloudflarePanel() {
  const [collapsed, setCollapsed] = usePersistentFlag("termdeck.cf.collapsed", false);
  const [accounts, setAccounts] = useState<CFAccount[]>([]);
  const [selectedAccountId, setSelectedAccountId] = useState<number | null>(null);
  const [tunnels, setTunnels] = useState<CFTunnel[]>([]);
  const [selectedTunnelId, setSelectedTunnelId] = useState<string>("");
  const [zones, setZones] = useState<CFZone[]>([]);
  const [routes, setRoutes] = useState<CFRoute[]>([]);
  const [loading, setLoading] = useState(false);
  const [loadingRoutes, setLoadingRoutes] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const [showAccountModal, setShowAccountModal] = useState(false);
  const [showRouteModal, setShowRouteModal] = useState(false);
  const [editingRoute, setEditingRoute] = useState<CFRoute | null>(null);
  const [copiedToken, setCopiedToken] = useState(false);
  const [copiedHost, setCopiedHost] = useState<string | null>(null);

  const loadAccounts = async () => {
    try {
      const accs = await api.cfListAccounts();
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

  useEffect(() => {
    if (selectedAccountId === null) {
      setTunnels([]);
      setZones([]);
      setRoutes([]);
      return;
    }
    let canceled = false;
    const fetchTunnelsAndZones = async () => {
      setLoading(true);
      setError(null);
      try {
        const [tuns, zns] = await Promise.all([
          api.cfListTunnels(selectedAccountId),
          api.cfListZones(selectedAccountId),
        ]);
        if (canceled) return;
        setTunnels(tuns);
        setZones(zns);
        if (tuns.length > 0) {
          setSelectedTunnelId(tuns[0].id);
        } else {
          setSelectedTunnelId("");
          setRoutes([]);
        }
      } catch (err) {
        if (!canceled) setError(errorMessage(err));
      } finally {
        if (!canceled) setLoading(false);
      }
    };
    void fetchTunnelsAndZones();
    return () => {
      canceled = true;
    };
  }, [selectedAccountId]);

  const loadRoutes = async () => {
    if (selectedAccountId === null || !selectedTunnelId) {
      setRoutes([]);
      return;
    }
    setLoadingRoutes(true);
    try {
      const rts = await api.cfListRoutes(selectedAccountId, selectedTunnelId);
      setRoutes(rts);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setLoadingRoutes(false);
    }
  };

  useEffect(() => {
    void loadRoutes();
  }, [selectedAccountId, selectedTunnelId]);

  const activeTunnel = tunnels.find((t) => t.id === selectedTunnelId);

  const handleCopyRunCmd = async () => {
    if (selectedAccountId === null || !selectedTunnelId) return;
    try {
      const { token } = await api.cfGetTunnelToken(selectedAccountId, selectedTunnelId);
      if (token) {
        await navigator.clipboard.writeText(`cloudflared tunnel run --token ${token}`);
        setCopiedToken(true);
        setTimeout(() => setCopiedToken(false), 1800);
      }
    } catch (err) {
      alert(errorMessage(err));
    }
  };

  const handleCopyUrl = async (hostname: string) => {
    try {
      await navigator.clipboard.writeText(`https://${hostname}`);
      setCopiedHost(hostname);
      setTimeout(() => setCopiedHost(null), 1500);
    } catch {
      // ignore
    }
  };

  const handleDeleteRoute = async (hostname: string) => {
    if (!window.confirm(`Hapus rute ${hostname} dan record DNS CNAME terkait?`)) return;
    if (selectedAccountId === null || !selectedTunnelId) return;
    const matchingZone = zones.find((z) => hostname.endsWith(z.name));
    try {
      await api.cfDeleteRoute(selectedAccountId, selectedTunnelId, hostname, matchingZone?.id);
      void loadRoutes();
    } catch (err) {
      alert(errorMessage(err));
    }
  };

  const getServiceProtocol = (svc: string) => {
    if (svc.startsWith("https://")) return "HTTPS";
    if (svc.startsWith("ssh://")) return "SSH";
    if (svc.startsWith("tcp://")) return "TCP";
    return "HTTP";
  };

  return (
    <section className={`cf-section${collapsed ? " is-collapsed" : ""}`}>
      <header className="section-head">
        <button
          type="button"
          className="section-toggle"
          onClick={() => setCollapsed(!collapsed)}
          aria-expanded={!collapsed}
          title={collapsed ? "Tampilkan Cloudflare Tunnel" : "Sembunyikan Cloudflare Tunnel"}
        >
          <span className={`chev-icon${collapsed ? "" : " is-open"}`}>
            <Icon name="chevron" size={13} />
          </span>
          <h3>
            Cloudflare Tunnels
            {tunnels.length > 0 && <span className="cf-count-tag">{tunnels.length}</span>}
          </h3>
        </button>
        <div className="cf-head-actions">
          {activeTunnel && !collapsed && (
            <button
              type="button"
              className="icon-btn"
              onClick={() => setShowRouteModal(true)}
              aria-label="Tambah Rute Hostname"
              title="Tambah rute hostname baru"
            >
              <Icon name="plus" size={14} />
            </button>
          )}
          <button
            type="button"
            className="icon-btn"
            onClick={() => setShowAccountModal(true)}
            aria-label="Kelola Akun Cloudflare"
            title="Kelola akun & API token Cloudflare"
          >
            <Icon name="sliders" size={13} />
          </button>
        </div>
      </header>

      {!collapsed && (
        <div className="cf-body">
          {accounts.length === 0 ? (
            <div className="cf-empty-state">
              <p className="empty-note">Hubungkan API token Cloudflare untuk mengelola tunnel dan domain.</p>
              <button
                type="button"
                className="btn btn-small btn-primary cf-cta-btn"
                onClick={() => setShowAccountModal(true)}
              >
                <Icon name="cloud" size={13} /> Setup Akun CF
              </button>
            </div>
          ) : (
            <>
              {/* Account & Tunnel Compact Bar */}
              <div className="cf-selector-grid">
                <div className="cf-select-wrapper" title="Akun Cloudflare">
                  <select
                    className="cf-compact-select"
                    value={selectedAccountId ?? ""}
                    onChange={(e) => setSelectedAccountId(Number(e.target.value))}
                  >
                    {accounts.map((a) => (
                      <option key={a.id} value={a.id}>
                        {a.name}
                      </option>
                    ))}
                  </select>
                </div>

                <div className="cf-tunnel-picker-row">
                  <select
                    className="cf-compact-select cf-tunnel-select"
                    value={selectedTunnelId}
                    onChange={(e) => setSelectedTunnelId(e.target.value)}
                    disabled={loading || tunnels.length === 0}
                    title="Pilih Tunnel"
                  >
                    {tunnels.map((t) => (
                      <option key={t.id} value={t.id}>
                        {t.name}
                      </option>
                    ))}
                  </select>

                  {activeTunnel && (
                    <button
                      type="button"
                      className="icon-btn cf-copy-btn"
                      onClick={() => void handleCopyRunCmd()}
                      title={copiedToken ? "Tersalin!" : "Salin perintah runner cloudflared"}
                    >
                      <Icon name={copiedToken ? "check" : "copy"} size={13} />
                    </button>
                  )}
                </div>
              </div>

              {/* Tunnel Status Pill */}
              {activeTunnel && (
                <div className="cf-status-bar">
                  <span
                    className={`cf-status-dot ${activeTunnel.status === "healthy" ? "is-healthy" : "is-down"}`}
                  />
                  <span className="cf-status-text">
                    {activeTunnel.status === "healthy" ? "Terhubung" : activeTunnel.status} &bull;{" "}
                    {activeTunnel.conns_count} konektor
                  </span>
                  <span className="cf-remote-tag">remote</span>
                </div>
              )}

              {/* Route List */}
              <div className="cf-routes-container">
                <div className="cf-routes-header">
                  <span className="cf-routes-title">
                    Public Hostnames {routes.length > 0 && `(${routes.length})`}
                  </span>
                  {activeTunnel && (
                    <button
                      type="button"
                      className="cf-text-btn"
                      onClick={() => setShowRouteModal(true)}
                    >
                      + Rute
                    </button>
                  )}
                </div>

                {loadingRoutes ? (
                  <div className="cf-skeleton-list">
                    <div className="cf-skeleton-row" />
                    <div className="cf-skeleton-row" />
                  </div>
                ) : routes.length === 0 ? (
                  <p className="empty-note" style={{ margin: "4px 0 2px" }}>
                    Belum ada hostname publik. Klik <strong>+ Rute</strong> untuk menghubungkan domain ke app.
                  </p>
                ) : (
                  <ul className="cf-route-list">
                    {routes.map((r) => {
                      const proto = getServiceProtocol(r.service);
                      const cleanService = r.service.replace(/^[a-z]+:\/\//, "");
                      return (
                        <li key={r.hostname} className="cf-route-item">
                          <div className="cf-route-main">
                            <div className="cf-route-head">
                              <span className={`cf-proto-badge proto-${proto.toLowerCase()}`}>{proto}</span>
                              <a
                                href={`https://${r.hostname}`}
                                target="_blank"
                                rel="noreferrer"
                                className="cf-hostname-link"
                                title={`Buka https://${r.hostname}`}
                              >
                                {r.hostname}
                              </a>
                            </div>
                            <div className="cf-route-target" title={`Origin: ${r.service}`}>
                              &rarr; {cleanService}
                            </div>
                          </div>

                          <div className="cf-route-actions">
                            <button
                              type="button"
                              className="icon-btn"
                              onClick={() => void handleCopyUrl(r.hostname)}
                              title={copiedHost === r.hostname ? "URL Tersalin!" : "Salin URL Domain"}
                            >
                              <Icon name={copiedHost === r.hostname ? "check" : "clipboard"} size={12} />
                            </button>
                            <button
                              type="button"
                              className="icon-btn"
                              onClick={() => setEditingRoute(r)}
                              title="Edit Service Origin (Aplikasi Target)"
                            >
                              <Icon name="edit" size={12} />
                            </button>
                            <button
                              type="button"
                              className="icon-btn danger"
                              onClick={() => void handleDeleteRoute(r.hostname)}
                              title="Hapus rute"
                            >
                              <Icon name="trash" size={12} />
                            </button>
                          </div>
                        </li>
                      );
                    })}
                  </ul>
                )}
              </div>
            </>
          )}

          {error && <p className="notice bad small cf-inline-notice">{error}</p>}
        </div>
      )}

      {showAccountModal && (
        <CloudflareAccountModal
          accounts={accounts}
          onAccountsChanged={() => {
            void loadAccounts();
          }}
          onClose={() => setShowAccountModal(false)}
        />
      )}

      {showRouteModal && selectedAccountId !== null && activeTunnel && (
        <CloudflareRouteModal
          accountId={selectedAccountId}
          tunnelId={activeTunnel.id}
          tunnelName={activeTunnel.name}
          zones={zones}
          onRouteAdded={() => {
            void loadRoutes();
          }}
          onClose={() => setShowRouteModal(false)}
        />
      )}

      {editingRoute && selectedAccountId !== null && activeTunnel && (
        <CloudflareRouteModal
          accountId={selectedAccountId}
          tunnelId={activeTunnel.id}
          tunnelName={activeTunnel.name}
          zones={zones}
          editRoute={editingRoute}
          onRouteAdded={() => {
            void loadRoutes();
          }}
          onClose={() => setEditingRoute(null)}
        />
      )}
    </section>
  );
}
