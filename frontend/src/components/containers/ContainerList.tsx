import { useMemo, useState } from "react";
import { api, errorMessage } from "../../api";
import type { ContainerActionBody, ContainerItem } from "../../types";
import { Icon } from "../Icon";

interface ContainerListProps {
  serverId: number;
  containers: ContainerItem[];
  loading: boolean;
  onRefresh: () => void;
  onOpenLogs: (containerId: string, containerName: string) => void;
  onExecTerminal: (containerId: string, containerName: string) => void;
  onRunNew: () => void;
}

export function ContainerList({
  serverId,
  containers,
  loading,
  onRefresh,
  onOpenLogs,
  onExecTerminal,
  onRunNew,
}: ContainerListProps) {
  const [query, setQuery] = useState<string>("");
  const [filterState, setFilterState] = useState<string>("all");
  const [actionBusy, setActionBusy] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [copiedId, setCopiedId] = useState<string | null>(null);

  const [viewMode, setViewMode] = useState<"table" | "cards">(() => {
    try {
      const saved = localStorage.getItem("td_containers_view");
      return saved === "cards" ? "cards" : "table";
    } catch {
      return "table";
    }
  });

  const handleSetViewMode = (mode: "table" | "cards") => {
    setViewMode(mode);
    try {
      localStorage.setItem("td_containers_view", mode);
    } catch {
      // ignore localstorage errors
    }
  };

  const handleCopyId = (id: string, e: React.MouseEvent) => {
    e.stopPropagation();
    try {
      if (navigator?.clipboard?.writeText) {
        void navigator.clipboard.writeText(id);
      }
      setCopiedId(id);
      setTimeout(() => {
        setCopiedId((curr) => (curr === id ? null : curr));
      }, 1500);
    } catch {
      // fallback
    }
  };

  const stats = useMemo(() => {
    let running = 0;
    let paused = 0;
    let exited = 0;
    for (const c of containers) {
      if (c.state === "running") running++;
      else if (c.state === "paused") paused++;
      else exited++;
    }
    return { total: containers.length, running, paused, exited };
  }, [containers]);

  const filtered = useMemo(() => {
    return containers.filter((c) => {
      if (filterState !== "all" && c.state !== filterState) return false;
      if (!query.trim()) return true;
      const q = query.toLowerCase();
      const matchName = c.names.some((n) => n.toLowerCase().includes(q));
      const matchImage = c.image.toLowerCase().includes(q);
      const matchId = c.id.toLowerCase().includes(q);
      return matchName || matchImage || matchId;
    });
  }, [containers, filterState, query]);

  const handleAction = async (containerId: string, action: ContainerActionBody["action"], force = false) => {
    if (action === "delete" && !confirm(`Hapus kontainer ${containerId}?`)) return;

    setActionBusy(`${containerId}-${action}`);
    setActionError(null);
    try {
      await api.containersAction({
        server_id: serverId,
        container_id: containerId,
        action,
        force,
      });
      onRefresh();
    } catch (err) {
      setActionError(errorMessage(err));
    } finally {
      setActionBusy(null);
    }
  };

  return (
    <div className="containers-subview">
      {actionError && (
        <div className="containers-error-banner">
          <span>{actionError}</span>
          <button type="button" onClick={() => setActionError(null)} className="icon-btn">
            ×
          </button>
        </div>
      )}

      {/* Summary Cards */}
      <div className="containers-summary-grid">
        <div
          className={`containers-stat-card${filterState === "all" ? " is-active" : ""}`}
          onClick={() => setFilterState("all")}
          role="button"
          tabIndex={0}
        >
          <div className="stat-label">Total Kontainer</div>
          <div className="stat-value">{stats.total}</div>
        </div>
        <div
          className={`containers-stat-card is-running${filterState === "running" ? " is-active" : ""}`}
          onClick={() => setFilterState(filterState === "running" ? "all" : "running")}
          role="button"
          tabIndex={0}
        >
          <div className="stat-label">Running</div>
          <div className="stat-value">{stats.running}</div>
        </div>
        <div
          className={`containers-stat-card is-paused${filterState === "paused" ? " is-active" : ""}`}
          onClick={() => setFilterState(filterState === "paused" ? "all" : "paused")}
          role="button"
          tabIndex={0}
        >
          <div className="stat-label">Paused</div>
          <div className="stat-value">{stats.paused}</div>
        </div>
        <div
          className={`containers-stat-card is-exited${filterState === "exited" ? " is-active" : ""}`}
          onClick={() => setFilterState(filterState === "exited" ? "all" : "exited")}
          role="button"
          tabIndex={0}
        >
          <div className="stat-label">Exited / Stopped</div>
          <div className="stat-value">{stats.exited}</div>
        </div>
      </div>

      {/* Filter & View Toolbar */}
      <div className="containers-filter-bar">
        <div className="containers-search-wrap">
          <Icon name="search" size={14} className="containers-search-icon" />
          <input
            type="text"
            className="input containers-search-input"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Cari kontainer berdasarkan nama, ID, atau image..."
          />
          {query && (
            <button
              type="button"
              className="icon-btn containers-search-clear"
              onClick={() => setQuery("")}
              aria-label="Bersihkan"
            >
              ×
            </button>
          )}
        </div>

        <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
          {/* View Mode Switcher */}
          <div className="containers-view-toggle">
            <button
              type="button"
              className={`containers-view-toggle-btn${viewMode === "table" ? " is-active" : ""}`}
              onClick={() => handleSetViewMode("table")}
              title="Tampilan Tabel Berkolom"
            >
              <Icon name="table" size={13} />
              <span className="hide-mobile">Tabel</span>
            </button>
            <button
              type="button"
              className={`containers-view-toggle-btn${viewMode === "cards" ? " is-active" : ""}`}
              onClick={() => handleSetViewMode("cards")}
              title="Tampilan Kartu"
            >
              <Icon name="grid" size={13} />
              <span className="hide-mobile">Kartu</span>
            </button>
          </div>

          <button type="button" className="btn btn-primary" onClick={onRunNew}>
            <Icon name="plus" size={14} /> <span>Buat Kontainer</span>
          </button>
        </div>
      </div>

      {/* Containers List or Table */}
      {filtered.length === 0 ? (
        <div className="containers-empty-state">
          <Icon name="box" size={36} />
          <p>{loading ? "Memuat kontainer..." : "Tidak ada kontainer yang sesuai."}</p>
          {!loading && (
            <button type="button" className="btn btn-primary" onClick={onRunNew}>
              <Icon name="plus" size={14} /> Jalankan Kontainer Pertama
            </button>
          )}
        </div>
      ) : viewMode === "table" ? (
        <div className="containers-table-wrap">
          <table className="containers-table">
            <thead>
              <tr>
                <th style={{ width: "130px" }}>Status</th>
                <th>Nama & ID</th>
                <th>Image</th>
                <th>Port Mapping</th>
                <th style={{ width: "140px" }}>Created</th>
                <th style={{ textAlign: "right", width: "230px" }}>Aksi</th>
              </tr>
            </thead>
            <tbody>
              {filtered.map((c) => {
                const primaryName = c.names[0] || c.id;
                const isBusy = !!actionBusy && actionBusy.startsWith(c.id);
                const shortId = c.id.length > 12 ? c.id.slice(0, 12) : c.id;
                const isCopied = copiedId === c.id;

                return (
                  <tr key={c.id}>
                    <td>
                      <div className="container-cell-status">
                        <span className={`status-pill pill-${c.state}`}>
                          <span className="status-dot" />
                          {c.state.toUpperCase()}
                        </span>
                        {c.status && (
                          <span className="container-status-text" title={c.status}>
                            {c.status}
                          </span>
                        )}
                      </div>
                    </td>

                    <td>
                      <div className="container-cell-name">
                        <span className="primary-name" title={primaryName}>
                          {primaryName}
                        </span>
                        <button
                          type="button"
                          className={`container-id-copy${isCopied ? " is-copied" : ""}`}
                          onClick={(e) => handleCopyId(c.id, e)}
                          title={`Klik untuk menyalin ID lengkap: ${c.id}`}
                        >
                          <Icon name={isCopied ? "check" : "copy"} size={11} />
                          <span>{isCopied ? "Tersalin!" : shortId}</span>
                        </button>
                      </div>
                    </td>

                    <td>
                      <div className="container-cell-image" title={c.image}>
                        {c.image}
                      </div>
                    </td>

                    <td>
                      <div className="container-cell-ports">
                        {c.ports.length > 0 ? (
                          <div className="ports-list">
                            {c.ports.map((p, idx) => (
                              <span key={idx} className="port-badge">
                                {p.host_port ? `${p.host_port} → ` : ""}
                                {p.container_port}/{p.protocol || "tcp"}
                              </span>
                            ))}
                          </div>
                        ) : (
                          <span style={{ color: "var(--muted)", fontSize: "12px" }}>-</span>
                        )}
                      </div>
                    </td>

                    <td style={{ color: "var(--muted)", fontSize: "12px", whiteSpace: "nowrap" }}>
                      {c.created || "-"}
                    </td>

                    <td style={{ textAlign: "right" }}>
                      <div className="container-cell-actions">
                        {c.state === "running" && (
                          <>
                            <button
                              type="button"
                              className="btn-action-sm btn-stop"
                              onClick={() => void handleAction(c.id, "stop")}
                              disabled={isBusy}
                              title="Stop kontainer"
                            >
                              <Icon
                                name={actionBusy === `${c.id}-stop` ? "refresh" : "stop"}
                                size={12}
                                className={actionBusy === `${c.id}-stop` ? "spin" : ""}
                              />
                              <span>Stop</span>
                            </button>
                            <button
                              type="button"
                              className="btn-action-sm"
                              onClick={() => void handleAction(c.id, "restart")}
                              disabled={isBusy}
                              title="Restart kontainer"
                            >
                              <Icon
                                name="refresh"
                                size={12}
                                className={actionBusy === `${c.id}-restart` ? "spin" : ""}
                              />
                              <span>Restart</span>
                            </button>
                          </>
                        )}

                        {c.state === "paused" && (
                          <>
                            <button
                              type="button"
                              className="btn-action-sm btn-start"
                              onClick={() => void handleAction(c.id, "unpause")}
                              disabled={isBusy}
                              title="Unpause kontainer"
                            >
                              <Icon
                                name={actionBusy === `${c.id}-unpause` ? "refresh" : "play"}
                                size={12}
                                className={actionBusy === `${c.id}-unpause` ? "spin" : ""}
                              />
                              <span>Unpause</span>
                            </button>
                            <button
                              type="button"
                              className="btn-action-sm btn-stop"
                              onClick={() => void handleAction(c.id, "stop")}
                              disabled={isBusy}
                              title="Stop kontainer"
                            >
                              <Icon
                                name={actionBusy === `${c.id}-stop` ? "refresh" : "stop"}
                                size={12}
                                className={actionBusy === `${c.id}-stop` ? "spin" : ""}
                              />
                              <span>Stop</span>
                            </button>
                          </>
                        )}

                        {c.state !== "running" && c.state !== "paused" && (
                          <button
                            type="button"
                            className="btn-action-sm btn-start"
                            onClick={() => void handleAction(c.id, "start")}
                            disabled={isBusy}
                            title="Start kontainer"
                          >
                            <Icon
                              name={actionBusy === `${c.id}-start` ? "refresh" : "play"}
                              size={12}
                              className={actionBusy === `${c.id}-start` ? "spin" : ""}
                            />
                            <span>Start</span>
                          </button>
                        )}

                        <button
                          type="button"
                          className="btn-action-sm"
                          onClick={() => onOpenLogs(c.id, primaryName)}
                          title="Lihat Logs"
                        >
                          <Icon name="file" size={12} />
                          <span>Logs</span>
                        </button>

                        <button
                          type="button"
                          className="btn-action-sm btn-exec"
                          onClick={() => onExecTerminal(c.id, primaryName)}
                          title="Buka terminal exec ke dalam kontainer"
                        >
                          <Icon name="terminal" size={12} />
                          <span>Exec</span>
                        </button>

                        <button
                          type="button"
                          className="btn-action-sm btn-delete"
                          onClick={() => void handleAction(c.id, "delete", c.state === "running")}
                          disabled={isBusy}
                          title="Hapus kontainer"
                        >
                          <Icon
                            name={actionBusy === `${c.id}-delete` ? "refresh" : "trash"}
                            size={12}
                            className={actionBusy === `${c.id}-delete` ? "spin" : ""}
                          />
                        </button>
                      </div>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      ) : (
        <div className="containers-card-list">
          {filtered.map((c) => {
            const primaryName = c.names[0] || c.id;
            const isBusy = !!actionBusy && actionBusy.startsWith(c.id);
            const shortId = c.id.length > 12 ? c.id.slice(0, 12) : c.id;
            const isCopied = copiedId === c.id;

            return (
              <div key={c.id} className={`container-card is-${c.state}`}>
                <div className="container-card-header">
                  <div className="container-title-group">
                    <span className={`status-pill pill-${c.state}`}>
                      <span className="status-dot" />
                      {c.state.toUpperCase()}
                    </span>
                    <h3 className="container-name" title={primaryName}>
                      {primaryName}
                    </h3>
                    <button
                      type="button"
                      className={`container-id-copy${isCopied ? " is-copied" : ""}`}
                      onClick={(e) => handleCopyId(c.id, e)}
                      title={`Klik untuk menyalin ID lengkap: ${c.id}`}
                    >
                      <Icon name={isCopied ? "check" : "copy"} size={11} />
                      <span>{isCopied ? "Tersalin!" : shortId}</span>
                    </button>
                  </div>

                  <div className="container-actions">
                    {c.state === "running" && (
                      <>
                        <button
                          type="button"
                          className="btn-action btn-stop"
                          onClick={() => void handleAction(c.id, "stop")}
                          disabled={isBusy}
                          title="Stop kontainer"
                        >
                          <Icon
                            name={actionBusy === `${c.id}-stop` ? "refresh" : "stop"}
                            size={12}
                            className={actionBusy === `${c.id}-stop` ? "spin" : ""}
                          />
                          <span>Stop</span>
                        </button>
                        <button
                          type="button"
                          className="btn-action"
                          onClick={() => void handleAction(c.id, "restart")}
                          disabled={isBusy}
                          title="Restart kontainer"
                        >
                          <Icon
                            name="refresh"
                            size={12}
                            className={actionBusy === `${c.id}-restart` ? "spin" : ""}
                          />
                          <span>Restart</span>
                        </button>
                        <button
                          type="button"
                          className="btn-action"
                          onClick={() => void handleAction(c.id, "pause")}
                          disabled={isBusy}
                          title="Pause kontainer"
                        >
                          <Icon
                            name={actionBusy === `${c.id}-pause` ? "refresh" : "pause"}
                            size={12}
                            className={actionBusy === `${c.id}-pause` ? "spin" : ""}
                          />
                          <span>Pause</span>
                        </button>
                      </>
                    )}

                    {c.state === "paused" && (
                      <>
                        <button
                          type="button"
                          className="btn-action btn-start"
                          onClick={() => void handleAction(c.id, "unpause")}
                          disabled={isBusy}
                          title="Unpause kontainer"
                        >
                          <Icon
                            name={actionBusy === `${c.id}-unpause` ? "refresh" : "play"}
                            size={12}
                            className={actionBusy === `${c.id}-unpause` ? "spin" : ""}
                          />
                          <span>Unpause</span>
                        </button>
                        <button
                          type="button"
                          className="btn-action btn-stop"
                          onClick={() => void handleAction(c.id, "stop")}
                          disabled={isBusy}
                          title="Stop kontainer"
                        >
                          <Icon
                            name={actionBusy === `${c.id}-stop` ? "refresh" : "stop"}
                            size={12}
                            className={actionBusy === `${c.id}-stop` ? "spin" : ""}
                          />
                          <span>Stop</span>
                        </button>
                      </>
                    )}

                    {c.state !== "running" && c.state !== "paused" && (
                      <button
                        type="button"
                        className="btn-action btn-start"
                        onClick={() => void handleAction(c.id, "start")}
                        disabled={isBusy}
                        title="Start kontainer"
                      >
                        <Icon
                          name={actionBusy === `${c.id}-start` ? "refresh" : "play"}
                          size={12}
                          className={actionBusy === `${c.id}-start` ? "spin" : ""}
                        />
                        <span>Start</span>
                      </button>
                    )}

                    <button
                      type="button"
                      className="btn-action"
                      onClick={() => onOpenLogs(c.id, primaryName)}
                      title="Lihat Logs"
                    >
                      <Icon name="file" size={12} /> <span>Logs</span>
                    </button>

                    <button
                      type="button"
                      className="btn-action btn-exec"
                      onClick={() => onExecTerminal(c.id, primaryName)}
                      title="Buka terminal exec ke dalam kontainer"
                    >
                      <Icon name="terminal" size={12} /> <span>Exec</span>
                    </button>

                    <button
                      type="button"
                      className="btn-action btn-delete"
                      onClick={() => void handleAction(c.id, "delete", c.state === "running")}
                      disabled={isBusy}
                      title="Hapus kontainer"
                    >
                      <Icon
                        name={actionBusy === `${c.id}-delete` ? "refresh" : "trash"}
                        size={12}
                        className={actionBusy === `${c.id}-delete` ? "spin" : ""}
                      />
                    </button>
                  </div>
                </div>

                <div className="container-card-body">
                  <div className="container-prop">
                    <span className="prop-name">Image:</span>
                    <span className="prop-val mono" title={c.image}>
                      {c.image}
                    </span>
                  </div>

                  {c.ports.length > 0 && (
                    <div className="container-prop">
                      <span className="prop-name">Ports:</span>
                      <span className="prop-val ports-list">
                        {c.ports.map((p, idx) => (
                          <span key={idx} className="port-badge">
                            {p.host_port ? `${p.host_port} → ` : ""}
                            {p.container_port}/{p.protocol || "tcp"}
                          </span>
                        ))}
                      </span>
                    </div>
                  )}

                  <div className="container-prop">
                    <span className="prop-name">Status:</span>
                    <span className="prop-val">{c.status}</span>
                  </div>

                  {c.created && (
                    <div className="container-prop">
                      <span className="prop-name">Created:</span>
                      <span className="prop-val">{c.created}</span>
                    </div>
                  )}
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
