import { useCallback, useEffect, useState } from "react";

import { api, errorMessage } from "../api";
import { formatBytes, formatUptime, ratio, serverColor } from "../format";
import { usePersistentFlag } from "../storage";
import type { Server, ServerStats } from "../types";
import { Icon } from "./Icon";

const REFRESH_MS = 20000;

function Meter({ label, used, total }: { label: string; used: number | null; total: number | null }) {
  const value = ratio(used, total);
  const level = value === null ? "" : value > 0.9 ? " is-hot" : value > 0.75 ? " is-warm" : "";
  return (
    <div className="meter">
      <div className="meter-label">
        <span>{label}</span>
        <span className="meter-num">
          {formatBytes(used)} / {formatBytes(total)}
        </span>
      </div>
      <div
        className={`meter-track${level}`}
        role="meter"
        aria-label={label}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={value === null ? undefined : Math.round(value * 100)}
      >
        <div className="meter-fill" style={{ width: `${(value ?? 0) * 100}%` }} />
      </div>
    </div>
  );
}

export function ServerInfo({
  server,
  onOpen,
  onOpenContainers,
}: {
  server: Server;
  onOpen: () => void;
  onOpenContainers?: () => void;
}) {
  // Shared by every server: once minimized it stays out of the way of the server list.
  const [collapsed, setCollapsed] = usePersistentFlag("termdeck.info.collapsed", false);
  const [stats, setStats] = useState<ServerStats | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      setStats(await api.serverStats(server.id));
      setError(null);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setLoading(false);
    }
  }, [server.id]);

  useEffect(() => {
    // Minimized: no polling, so the server is not contacted just to fill a hidden panel.
    if (collapsed) return;
    void load();
    const timer = window.setInterval(() => void load(), REFRESH_MS);
    return () => window.clearInterval(timer);
  }, [load, collapsed]);

  const load1 = stats?.load[0];
  const loadRatio = load1 !== undefined && stats?.cpu_count ? load1 / stats.cpu_count : null;

  return (
    <section className={`info${collapsed ? " is-collapsed" : ""}`} style={{ ["--srv" as string]: serverColor(server.color) }}>
      <header className="info-head">
        <button
          type="button"
          className="info-toggle"
          onClick={() => setCollapsed(!collapsed)}
          aria-expanded={!collapsed}
          title={collapsed ? "Tampilkan detail server" : "Sembunyikan detail server"}
        >
          <span className={`chev-icon${collapsed ? "" : " is-open"}`}>
            <Icon name="chevron" size={12} />
          </span>
          <span className="info-title">
            <h3>{server.name}</h3>
            {!collapsed && (
              <span className="info-sub">{stats?.os || `${server.username}@${server.host}:${server.port}`}</span>
            )}
          </span>
        </button>
        {!collapsed && (
          <button
            type="button"
            className="icon-btn"
            onClick={() => void load()}
            disabled={loading}
            aria-label="Muat ulang info server"
            title="Muat ulang"
          >
            <Icon name="refresh" size={14} />
          </button>
        )}
      </header>

      {!collapsed && (
        <div className="info-body">
          {error && <p className="notice bad small">{error}</p>}
          {!stats && !error && <p className="info-loading">Mengambil info server…</p>}

          {stats && (
            <>
              <dl className="facts">
                <div>
                  <dt>Hostname</dt>
                  <dd>{stats.hostname || "–"}</dd>
                </div>
                <div>
                  <dt>Uptime</dt>
                  <dd>{formatUptime(stats.uptime_seconds)}</dd>
                </div>
                <div>
                  <dt>Load</dt>
                  <dd className={loadRatio !== null && loadRatio > 1 ? "is-hot-text" : ""}>
                    {stats.load.length ? stats.load.map((l) => l.toFixed(2)).join("  ") : "–"}
                  </dd>
                </div>
                <div>
                  <dt>CPU</dt>
                  <dd>{stats.cpu_count ? `${stats.cpu_count} core` : "–"}</dd>
                </div>
              </dl>
              <Meter label="RAM" used={stats.mem_used} total={stats.mem_total} />
              <Meter label="Disk /" used={stats.disk_used} total={stats.disk_total} />
              <p className="kernel">{stats.kernel}</p>
            </>
          )}

          <div style={{ display: "grid", gridTemplateColumns: onOpenContainers ? "1fr 1fr" : "1fr", gap: "6px" }}>
            <button type="button" className="btn btn-block" onClick={onOpen}>
              <Icon name="terminal" size={14} /> Terminal
            </button>
            {onOpenContainers && (
              <button
                type="button"
                className="btn btn-block btn-subtle"
                onClick={onOpenContainers}
                title="Kelola Containers (Podman / Docker)"
              >
                <Icon name="box" size={14} /> Containers
              </button>
            )}
          </div>
        </div>
      )}
    </section>
  );
}
