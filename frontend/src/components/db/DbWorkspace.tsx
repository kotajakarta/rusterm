import { useCallback, useEffect, useState } from "react";
import { api, errorMessage } from "../../api";
import { engineLabel } from "../../dbClient";
import type {
  DbConnection,
  DbTableInfo,
  DbTableSchema,
  Server,
} from "../../types";
import { Icon } from "../Icon";
import { ConfirmDialog, Modal } from "../Modal";
import { DbConnectionModal } from "./DbConnectionModal";
import { DbCreateTableModal } from "./DbCreateTableModal";
import { DbDataGrid } from "./DbDataGrid";
import { DbQueryRunner } from "./DbQueryRunner";
import { DbSqlPreviewModal } from "./DbSqlPreviewModal";
import { DbStructureView } from "./DbStructureView";

interface DbWorkspaceProps {
  servers: Server[];
  preferredSshServerId?: number | null;
}

type InnerTab = "data" | "structure" | "sql";

export function DbWorkspace({
  servers,
  preferredSshServerId = null,
}: DbWorkspaceProps): JSX.Element {
  const [connections, setConnections] = useState<DbConnection[]>([]);
  const [loadingConns, setLoadingConns] = useState(true);
  const [selectedConnId, setSelectedConnId] = useState<number | null>(null);

  const [databases, setDatabases] = useState<string[]>([]);
  const [selectedDb, setSelectedDb] = useState<string>("");
  const [loadingDbs, setLoadingDbs] = useState(false);

  const [tables, setTables] = useState<DbTableInfo[]>([]);
  const [tableFilter, setTableFilter] = useState("");
  const [selectedTable, setSelectedTable] = useState<string>("");
  const [loadingTables, setLoadingTables] = useState(false);

  const [schema, setSchema] = useState<DbTableSchema | null>(null);
  const [loadingSchema, setLoadingSchema] = useState(false);

  const [innerTab, setInnerTab] = useState<InnerTab>("data");
  const [error, setError] = useState<string | null>(null);

  // Modals
  const [connModal, setConnModal] = useState<
    | { kind: "create" }
    | { kind: "edit"; conn: DbConnection }
    | null
  >(null);
  const [deleteConnTarget, setDeleteConnTarget] = useState<DbConnection | null>(null);
  const [showCreateDb, setShowCreateDb] = useState(false);
  const [newDbName, setNewDbName] = useState("");
  const [dropDbTarget, setDropDbTarget] = useState<string | null>(null);
  const [showCreateTable, setShowCreateTable] = useState(false);
  const [dropTableTarget, setDropTableTarget] = useState<string | null>(null);
  const [ddlBusy, setDdlBusy] = useState(false);

  const activeConn = connections.find((c) => c.id === selectedConnId) ?? null;

  const loadConnections = useCallback(async () => {
    setLoadingConns(true);
    try {
      const list = await api.dbListConnections();
      setConnections(list);
      setSelectedConnId((prev) => {
        if (prev && list.some((c) => c.id === prev)) return prev;
        return list[0]?.id ?? null;
      });
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setLoadingConns(false);
    }
  }, []);

  useEffect(() => {
    void loadConnections();
  }, [loadConnections]);

  const loadDatabases = useCallback(async (conn: DbConnection) => {
    setLoadingDbs(true);
    setError(null);
    setDatabases([]);
    setTables([]);
    setSelectedTable("");
    setSchema(null);
    try {
      const dbs = await api.dbListDatabases(conn.id);
      setDatabases(dbs);
      const preferred =
        conn.default_database && dbs.includes(conn.default_database)
          ? conn.default_database
          : dbs[0] ?? "";
      setSelectedDb(preferred);
    } catch (err) {
      setError(errorMessage(err));
      setSelectedDb("");
    } finally {
      setLoadingDbs(false);
    }
  }, []);

  useEffect(() => {
    if (!activeConn) {
      setDatabases([]);
      setSelectedDb("");
      setTables([]);
      setSelectedTable("");
      setSchema(null);
      return;
    }
    void loadDatabases(activeConn);
  }, [activeConn?.id, loadDatabases]);

  const loadTables = useCallback(async (connId: number, db: string) => {
    if (!db) {
      setTables([]);
      setSelectedTable("");
      setSchema(null);
      return;
    }
    setLoadingTables(true);
    setError(null);
    try {
      const list = await api.dbListTables(connId, db);
      setTables(list);
      setSelectedTable((prev) => {
        if (prev && list.some((t) => t.name === prev)) return prev;
        return list[0]?.name ?? "";
      });
    } catch (err) {
      setError(errorMessage(err));
      setTables([]);
      setSelectedTable("");
    } finally {
      setLoadingTables(false);
    }
  }, []);

  useEffect(() => {
    if (!activeConn || !selectedDb) return;
    void loadTables(activeConn.id, selectedDb);
  }, [activeConn?.id, selectedDb, loadTables]);

  const loadSchema = useCallback(async (connId: number, db: string, table: string) => {
    if (!db || !table) {
      setSchema(null);
      return;
    }
    setLoadingSchema(true);
    try {
      const s = await api.dbDescribeTable(connId, db, table);
      setSchema(s);
    } catch (err) {
      setError(errorMessage(err));
      setSchema(null);
    } finally {
      setLoadingSchema(false);
    }
  }, []);

  useEffect(() => {
    if (!activeConn || !selectedDb || !selectedTable) {
      setSchema(null);
      return;
    }
    void loadSchema(activeConn.id, selectedDb, selectedTable);
  }, [activeConn?.id, selectedDb, selectedTable, loadSchema]);

  const handleDeleteConn = async () => {
    if (!deleteConnTarget) return;
    try {
      await api.dbDeleteConnection(deleteConnTarget.id);
      setDeleteConnTarget(null);
      await loadConnections();
    } catch (err) {
      setError(errorMessage(err));
      setDeleteConnTarget(null);
    }
  };

  const handleCreateDb = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!activeConn || !newDbName.trim()) return;
    setDdlBusy(true);
    setError(null);
    try {
      const name = newDbName.trim();
      await api.dbCreateDatabase(activeConn.id, name);
      setShowCreateDb(false);
      setNewDbName("");
      await loadDatabases(activeConn);
      setSelectedDb(name);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setDdlBusy(false);
    }
  };

  const handleDropDb = async () => {
    if (!activeConn || !dropDbTarget) return;
    setDdlBusy(true);
    setError(null);
    try {
      await api.dbDropDatabase(activeConn.id, dropDbTarget);
      setDropDbTarget(null);
      await loadDatabases(activeConn);
    } catch (err) {
      setError(errorMessage(err));
      setDropDbTarget(null);
    } finally {
      setDdlBusy(false);
    }
  };

  const handleDropTable = async () => {
    if (!activeConn || !selectedDb || !dropTableTarget) return;
    setDdlBusy(true);
    setError(null);
    try {
      await api.dbDropTable(activeConn.id, selectedDb, dropTableTarget);
      setDropTableTarget(null);
      await loadTables(activeConn.id, selectedDb);
    } catch (err) {
      setError(errorMessage(err));
      setDropTableTarget(null);
    } finally {
      setDdlBusy(false);
    }
  };

  const filteredTables = tableFilter.trim()
    ? tables.filter((t) => t.name.toLowerCase().includes(tableFilter.trim().toLowerCase()))
    : tables;

  const sshServerName = activeConn?.ssh_server_id
    ? servers.find((s) => s.id === activeConn.ssh_server_id)?.name ?? `SSH #${activeConn.ssh_server_id}`
    : null;

  return (
    <div className="db-workspace">
      <aside className="db-sidebar">
        <div className="db-sidebar-head">
          <span className="db-sidebar-title">
            <Icon name="database" size={14} />
            <span>Database</span>
          </span>
          <button
            type="button"
            className="btn btn-primary btn-sm"
            onClick={() => setConnModal({ kind: "create" })}
            title="Tambah koneksi database baru"
          >
            <Icon name="plus" size={13} />
            <span>Baru</span>
          </button>
        </div>

        <div className="db-conn-picker">
          <select
            value={selectedConnId ?? ""}
            onChange={(e) => setSelectedConnId(e.target.value ? Number(e.target.value) : null)}
            disabled={loadingConns || connections.length === 0}
            aria-label="Pilih koneksi database"
          >
            {connections.length === 0 ? (
              <option value="">Belum ada koneksi</option>
            ) : (
              connections.map((c) => (
                <option key={c.id} value={c.id}>
                  {c.name} ({c.engine})
                </option>
              ))
            )}
          </select>
          {activeConn && (
            <div className="db-conn-actions">
              <button
                type="button"
                className="icon-btn"
                onClick={() => void loadDatabases(activeConn)}
                title="Refresh koneksi"
                aria-label="Refresh koneksi"
              >
                <Icon name="refresh" size={13} />
              </button>
              <button
                type="button"
                className="icon-btn"
                onClick={() => setConnModal({ kind: "edit", conn: activeConn })}
                title="Edit koneksi"
                aria-label="Edit koneksi"
              >
                <Icon name="edit" size={13} />
              </button>
              <button
                type="button"
                className="icon-btn"
                onClick={() => setDeleteConnTarget(activeConn)}
                title="Hapus koneksi"
                aria-label="Hapus koneksi"
              >
                <Icon name="trash" size={13} />
              </button>
            </div>
          )}
        </div>

        {activeConn && (
          <div className="db-conn-meta">
            <span className="db-pill">{engineLabel(activeConn.engine)}</span>
            {sshServerName && <span className="db-pill db-pill-ssh">SSH: {sshServerName}</span>}
            {activeConn.read_only && <span className="db-ro-badge">RO</span>}
          </div>
        )}

        {activeConn && (
          <div className="db-db-section">
            <div className="db-subhead">
              <span>Database</span>
              {!activeConn.read_only && activeConn.engine !== "sqlite" && (
                <div className="db-subhead-acts">
                  <button
                    type="button"
                    className="icon-btn"
                    onClick={() => {
                      setNewDbName("");
                      setShowCreateDb(true);
                    }}
                    title="Buat database baru"
                    aria-label="Buat database baru"
                  >
                    <Icon name="plus" size={13} />
                  </button>
                  {selectedDb && (
                    <button
                      type="button"
                      className="icon-btn"
                      onClick={() => setDropDbTarget(selectedDb)}
                      title={`Drop database ${selectedDb}`}
                      aria-label="Drop database"
                    >
                      <Icon name="trash" size={12} />
                    </button>
                  )}
                </div>
              )}
            </div>
            <select
              className="db-select-full"
              value={selectedDb}
              onChange={(e) => setSelectedDb(e.target.value)}
              disabled={loadingDbs || databases.length === 0}
              aria-label="Pilih database"
            >
              {databases.length === 0 ? (
                <option value="">{loadingDbs ? "Memuat..." : "(kosong)"}</option>
              ) : (
                databases.map((db) => (
                  <option key={db} value={db}>
                    {db}
                  </option>
                ))
              )}
            </select>
          </div>
        )}

        {activeConn && selectedDb && (
          <div className="db-tables-section">
            <div className="db-subhead">
              <span>Tabel ({tables.length})</span>
              {!activeConn.read_only && (
                <button
                  type="button"
                  className="icon-btn"
                  onClick={() => setShowCreateTable(true)}
                  title="Buat tabel baru"
                  aria-label="Buat tabel baru"
                >
                  <Icon name="plus" size={13} />
                </button>
              )}
            </div>
            <input
              type="search"
              className="db-table-search"
              placeholder="Cari tabel..."
              value={tableFilter}
              onChange={(e) => setTableFilter(e.target.value)}
            />
            <ul className="db-table-list">
              {loadingTables && tables.length === 0 && (
                <li className="db-muted" style={{ padding: "8px 6px" }}>
                  Memuat tabel...
                </li>
              )}
              {!loadingTables && filteredTables.length === 0 && (
                <li className="db-muted" style={{ padding: "8px 6px" }}>
                  Tidak ada tabel.
                </li>
              )}
              {filteredTables.map((t) => (
                <li
                  key={t.name}
                  className={`db-table-item${selectedTable === t.name ? " is-active" : ""}`}
                >
                  <button
                    type="button"
                    className="db-table-btn"
                    onClick={() => {
                      setSelectedTable(t.name);
                      if (innerTab === "sql") setInnerTab("data");
                    }}
                    title={`${t.name} (${t.kind})`}
                  >
                    <Icon name="table" size={13} />
                    <span className="db-table-name">{t.name}</span>
                    {t.kind === "view" && <span className="db-view-tag">V</span>}
                  </button>
                  {!activeConn.read_only && t.kind === "table" && (
                    <button
                      type="button"
                      className="icon-btn db-table-del"
                      onClick={() => setDropTableTarget(t.name)}
                      title={`Drop tabel ${t.name}`}
                      aria-label={`Drop tabel ${t.name}`}
                    >
                      <Icon name="trash" size={12} />
                    </button>
                  )}
                </li>
              ))}
            </ul>
          </div>
        )}
      </aside>

      <section className="db-main">
        {error && <div className="db-banner is-err" style={{ margin: "10px 12px 0" }}>{error}</div>}

        {!activeConn ? (
          <div className="db-welcome">
            <Icon name="database" size={36} />
            <h3>Database Client Ringan</h3>
            <p>
              Kelola PostgreSQL, MySQL/MariaDB, atau SQLite langsung dari RusTerm — lengkap dengan SSH Tunnel otomatis via server yang sudah tersimpan.
            </p>
            <button
              type="button"
              className="btn btn-primary"
              onClick={() => setConnModal({ kind: "create" })}
            >
              <Icon name="plus" size={14} />
              <span>Tambah Koneksi Pertama</span>
            </button>
          </div>
        ) : (
          <>
            <header className="db-main-header">
              <div className="db-breadcrumb">
                <strong>{activeConn.name}</strong>
                {selectedDb && (
                  <>
                    <span className="db-sep">/</span>
                    <span>{selectedDb}</span>
                  </>
                )}
                {selectedTable && innerTab !== "sql" && (
                  <>
                    <span className="db-sep">/</span>
                    <span className="db-mono">{selectedTable}</span>
                  </>
                )}
              </div>
              <div className="db-inner-tabs" role="tablist">
                <button
                  type="button"
                  role="tab"
                  aria-selected={innerTab === "data"}
                  className={`db-inner-tab${innerTab === "data" ? " is-active" : ""}`}
                  onClick={() => setInnerTab("data")}
                >
                  Data
                </button>
                <button
                  type="button"
                  role="tab"
                  aria-selected={innerTab === "structure"}
                  className={`db-inner-tab${innerTab === "structure" ? " is-active" : ""}`}
                  onClick={() => setInnerTab("structure")}
                >
                  Struktur
                </button>
                <button
                  type="button"
                  role="tab"
                  aria-selected={innerTab === "sql"}
                  className={`db-inner-tab${innerTab === "sql" ? " is-active" : ""}`}
                  onClick={() => setInnerTab("sql")}
                >
                  SQL Runner
                </button>
              </div>
            </header>

            <div className="db-main-body">
              {innerTab === "sql" ? (
                <DbQueryRunner connection={activeConn} database={selectedDb} />
              ) : !selectedTable ? (
                <div className="db-empty">
                  Pilih tabel di panel kiri, atau buka tab <strong>SQL Runner</strong> untuk menjalankan query.
                </div>
              ) : innerTab === "data" ? (
                <DbDataGrid
                  connection={activeConn}
                  database={selectedDb}
                  table={selectedTable}
                  schema={schema}
                />
              ) : (
                <DbStructureView schema={schema} loading={loadingSchema} />
              )}
            </div>
          </>
        )}
      </section>

      {connModal && (
        <DbConnectionModal
          initial={connModal.kind === "edit" ? connModal.conn : null}
          defaultSshServerId={connModal.kind === "create" ? preferredSshServerId : null}
          servers={servers}
          onSaved={(saved) => {
            setConnModal(null);
            void loadConnections().then(() => setSelectedConnId(saved.id));
          }}
          onClose={() => setConnModal(null)}
        />
      )}

      {deleteConnTarget && (
        <ConfirmDialog
          title="Hapus Koneksi Database"
          message={`Hapus profil koneksi "${deleteConnTarget.name}" dari RusTerm? Database aslinya tidak akan terhapus.`}
          confirmLabel="Hapus"
          onConfirm={() => void handleDeleteConn()}
          onCancel={() => setDeleteConnTarget(null)}
        />
      )}

      {showCreateDb && activeConn && (
        <Modal
          title="Buat Database Baru"
          onClose={() => setShowCreateDb(false)}
          footer={
            <>
              <button type="button" className="btn" onClick={() => setShowCreateDb(false)} disabled={ddlBusy}>
                Batal
              </button>
              <button type="submit" form="db-create-db-form" className="btn btn-primary" disabled={ddlBusy}>
                {ddlBusy ? "Membuat..." : "Buat Database"}
              </button>
            </>
          }
        >
          <form id="db-create-db-form" className="db-form" onSubmit={(e) => void handleCreateDb(e)}>
            <label className="field">
              <span>Nama Database</span>
              <input
                type="text"
                value={newDbName}
                onChange={(e) => setNewDbName(e.target.value)}
                placeholder="contoh: app_staging"
                required
              />
            </label>
          </form>
        </Modal>
      )}

      {dropDbTarget && activeConn && (
        <DbSqlPreviewModal
          title="Drop Database"
          description={`Tindakan ini akan menghapus database "${dropDbTarget}" beserta seluruh isinya secara permanen.`}
          sqlPreview={`DROP DATABASE ${activeConn.engine === "mysql" ? `\`${dropDbTarget}\`` : `"${dropDbTarget}"`};`}
          confirmLabel="Drop Database"
          danger
          requireTypeText={dropDbTarget}
          busy={ddlBusy}
          onConfirm={() => void handleDropDb()}
          onCancel={() => setDropDbTarget(null)}
        />
      )}

      {showCreateTable && activeConn && selectedDb && (
        <DbCreateTableModal
          connId={activeConn.id}
          engine={activeConn.engine}
          database={selectedDb}
          onCreated={(tbl) => {
            setShowCreateTable(false);
            void loadTables(activeConn.id, selectedDb).then(() => setSelectedTable(tbl));
          }}
          onClose={() => setShowCreateTable(false)}
        />
      )}

      {dropTableTarget && activeConn && (
        <DbSqlPreviewModal
          title="Drop Tabel"
          description={`Hapus tabel "${dropTableTarget}" secara permanen dari database "${selectedDb}"?`}
          sqlPreview={`DROP TABLE ${activeConn.engine === "mysql" ? `\`${dropTableTarget}\`` : `"${dropTableTarget}"`};`}
          confirmLabel="Drop Tabel"
          danger
          requireTypeText={dropTableTarget}
          busy={ddlBusy}
          onConfirm={() => void handleDropTable()}
          onCancel={() => setDropTableTarget(null)}
        />
      )}
    </div>
  );
}
