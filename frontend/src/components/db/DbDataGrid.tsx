import { useCallback, useEffect, useState } from "react";
import { api, errorMessage } from "../../api";
import {
  buildSqlPreview,
  dbValueToRaw,
  formatDbValue,
  parseCellInput,
  rowsToCsv,
  rowsToJson,
} from "../../dbClient";
import type {
  DbConnection,
  DbFilterOp,
  DbRowsPage,
  DbTableSchema,
  DbValue,
} from "../../types";
import { Icon } from "../Icon";
import { Modal } from "../Modal";
import { DbSqlPreviewModal } from "./DbSqlPreviewModal";

interface DbDataGridProps {
  connection: DbConnection;
  database: string;
  table: string;
  schema: DbTableSchema | null;
}

interface PendingMutation {
  op: "insert" | "update" | "delete";
  pk: Record<string, unknown>;
  values: Record<string, unknown>;
  sqlPreview: string;
}

function triggerDownload(content: string, filename: string, mime: string): void {
  const blob = new Blob([content], { type: mime });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  a.click();
  URL.revokeObjectURL(url);
}

export function DbDataGrid({
  connection,
  database,
  table,
  schema,
}: DbDataGridProps): JSX.Element {
  const [page, setPage] = useState(1);
  const [limit, setLimit] = useState(100);
  const [sortCol, setSortCol] = useState<string>("");
  const [sortDir, setSortDir] = useState<"asc" | "desc">("asc");
  const [filterCol, setFilterCol] = useState<string>("");
  const [filterOp, setFilterOp] = useState<DbFilterOp>("contains");
  const [filterVal, setFilterVal] = useState<string>("");
  const [appliedFilter, setAppliedFilter] = useState<{
    col: string;
    op: DbFilterOp;
    val: string;
  } | null>(null);

  const [data, setData] = useState<DbRowsPage | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Inline edit state
  const [editingCell, setEditingCell] = useState<{
    rowIdx: number;
    colIdx: number;
    draft: string;
  } | null>(null);

  // Insert row modal
  const [showInsert, setShowInsert] = useState(false);
  const [insertDraft, setInsertDraft] = useState<Record<string, string>>({});

  // Confirm SQL mutation
  const [pending, setPending] = useState<PendingMutation | null>(null);
  const [mutating, setMutating] = useState(false);

  // Reset saat pindah tabel/db
  useEffect(() => {
    setPage(1);
    setSortCol("");
    setSortDir("asc");
    setFilterCol("");
    setFilterVal("");
    setAppliedFilter(null);
    setEditingCell(null);
  }, [connection.id, database, table]);

  const loadRows = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const res = await api.dbFetchRows(connection.id, table, {
        db: database,
        page,
        limit,
        sort: sortCol || undefined,
        dir: sortCol ? sortDir : undefined,
        filter_col: appliedFilter?.col,
        filter_op: appliedFilter?.op,
        filter_val: appliedFilter?.val,
      });
      setData(res);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setLoading(false);
    }
  }, [connection.id, database, table, page, limit, sortCol, sortDir, appliedFilter]);

  useEffect(() => {
    void loadRows();
  }, [loadRows]);

  const hasPk = (schema?.primary_keys.length ?? 0) > 0;
  const canEdit = !connection.read_only && hasPk;

  const extractPk = (row: DbValue[]): Record<string, unknown> => {
    const pk: Record<string, unknown> = {};
    if (!schema || !data) return pk;
    for (const pkCol of schema.primary_keys) {
      const idx = data.columns.indexOf(pkCol);
      if (idx >= 0) {
        pk[pkCol] = dbValueToRaw(row[idx]);
      }
    }
    return pk;
  };

  const handleSort = (col: string) => {
    setPage(1);
    if (sortCol === col) {
      setSortDir((d) => (d === "asc" ? "desc" : "asc"));
    } else {
      setSortCol(col);
      setSortDir("asc");
    }
  };

  const handleApplyFilter = (e: React.FormEvent) => {
    e.preventDefault();
    setPage(1);
    if (!filterCol) {
      setAppliedFilter(null);
      return;
    }
    setAppliedFilter({ col: filterCol, op: filterOp, val: filterVal });
  };

  const startCellEdit = (rowIdx: number, colIdx: number) => {
    if (!canEdit || !data) return;
    const cell = data.rows[rowIdx][colIdx];
    const initial = cell.type === "null" ? "" : formatDbValue(cell);
    setEditingCell({ rowIdx, colIdx, draft: initial });
  };

  const commitCellEdit = () => {
    if (!editingCell || !data || !schema) return;
    const colName = data.columns[editingCell.colIdx];
    const colSchema = schema.columns.find((c) => c.name === colName);
    const parsed = parseCellInput(
      editingCell.draft,
      colSchema?.data_type ?? "text",
      colSchema?.nullable ?? true,
    );
    const pk = extractPk(data.rows[editingCell.rowIdx]);
    const values = { [colName]: parsed };
    const sqlPreview = buildSqlPreview(connection.engine, "update", table, pk, values);
    setEditingCell(null);
    setPending({ op: "update", pk, values, sqlPreview });
  };

  const requestDeleteRow = (rowIdx: number) => {
    if (!canEdit || !data) return;
    const pk = extractPk(data.rows[rowIdx]);
    const sqlPreview = buildSqlPreview(connection.engine, "delete", table, pk, {});
    setPending({ op: "delete", pk, values: {}, sqlPreview });
  };

  const openInsertModal = () => {
    const draft: Record<string, string> = {};
    for (const c of schema?.columns ?? []) {
      draft[c.name] = "";
    }
    setInsertDraft(draft);
    setShowInsert(true);
  };

  const submitInsertModal = (e: React.FormEvent) => {
    e.preventDefault();
    if (!schema) return;
    const values: Record<string, unknown> = {};
    for (const col of schema.columns) {
      const raw = insertDraft[col.name] ?? "";
      if (raw.trim() === "" && (col.default_value !== null || col.is_primary_key)) {
        continue;
      }
      values[col.name] = parseCellInput(raw, col.data_type, col.nullable);
    }
    if (Object.keys(values).length === 0) {
      setError("Isi minimal satu kolom untuk menambah baris.");
      return;
    }
    const sqlPreview = buildSqlPreview(connection.engine, "insert", table, {}, values);
    setShowInsert(false);
    setPending({ op: "insert", pk: {}, values, sqlPreview });
  };

  const executePending = async () => {
    if (!pending) return;
    setMutating(true);
    setError(null);
    try {
      await api.dbMutateRow(connection.id, table, {
        db: database,
        op: pending.op,
        pk: pending.op === "insert" ? undefined : pending.pk,
        values: pending.op === "delete" ? undefined : pending.values,
      });
      setPending(null);
      await loadRows();
    } catch (err) {
      setError(errorMessage(err));
      setPending(null);
    } finally {
      setMutating(false);
    }
  };

  return (
    <div className="db-grid-container">
      <form className="db-filter-bar" onSubmit={handleApplyFilter}>
        <select
          value={filterCol}
          onChange={(e) => setFilterCol(e.target.value)}
          aria-label="Kolom filter"
        >
          <option value="">Filter kolom...</option>
          {(data?.columns ?? schema?.columns.map((c) => c.name) ?? []).map((col) => (
            <option key={col} value={col}>
              {col}
            </option>
          ))}
        </select>
        <select
          value={filterOp}
          onChange={(e) => setFilterOp(e.target.value as DbFilterOp)}
          aria-label="Operator filter"
        >
          <option value="contains">contains</option>
          <option value="eq">=</option>
          <option value="neq">!=</option>
          <option value="gt">&gt;</option>
          <option value="gte">&gt;=</option>
          <option value="lt">&lt;</option>
          <option value="lte">&lt;=</option>
          <option value="is_null">IS NULL</option>
          <option value="is_not_null">IS NOT NULL</option>
        </select>
        {filterOp !== "is_null" && filterOp !== "is_not_null" && (
          <input
            type="text"
            value={filterVal}
            onChange={(e) => setFilterVal(e.target.value)}
            placeholder="Nilai filter..."
          />
        )}
        <button type="submit" className="btn btn-sm">
          Terapkan
        </button>
        {appliedFilter && (
          <button
            type="button"
            className="btn btn-sm"
            onClick={() => {
              setFilterCol("");
              setFilterVal("");
              setAppliedFilter(null);
              setPage(1);
            }}
          >
            Reset
          </button>
        )}

        <div style={{ flex: 1 }} />

        {!connection.read_only && (
          <button type="button" className="btn btn-primary btn-sm" onClick={openInsertModal}>
            <Icon name="plus" size={13} />
            <span>Baris</span>
          </button>
        )}
        <button
          type="button"
          className="btn btn-sm"
          onClick={() => void loadRows()}
          disabled={loading}
          title="Muat ulang"
        >
          <Icon name="refresh" size={13} />
        </button>
        {data && (
          <>
            <button
              type="button"
              className="btn btn-sm"
              onClick={() =>
                triggerDownload(
                  rowsToCsv(data.columns, data.rows),
                  `${table}-page${page}.csv`,
                  "text/csv;charset=utf-8",
                )
              }
            >
              CSV
            </button>
            <button
              type="button"
              className="btn btn-sm"
              onClick={() =>
                triggerDownload(
                  rowsToJson(data.columns, data.rows),
                  `${table}-page${page}.json`,
                  "application/json",
                )
              }
            >
              JSON
            </button>
          </>
        )}
      </form>

      {error && <div className="db-banner is-err">{error}</div>}

      {!connection.read_only && schema && !hasPk && (
        <div className="db-banner is-info">
          Tabel ini tidak memiliki Primary Key — edit/hapus baris inline dinonaktifkan demi keamanan. Gunakan tab SQL Runner bila perlu.
        </div>
      )}

      <div className="db-table-wrap">
        <table className="db-table">
          <thead>
            <tr>
              {canEdit && <th className="db-act-col" aria-label="Aksi" />}
              {(data?.columns ?? []).map((col) => (
                <th key={col} className="db-sortable" onClick={() => handleSort(col)}>
                  <span>{col}</span>
                  {sortCol === col && (
                    <span className="db-sort-ind">{sortDir === "asc" ? " ▲" : " ▼"}</span>
                  )}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {loading && !data && (
              <tr>
                <td colSpan={Math.max(1, (schema?.columns.length ?? 1) + (canEdit ? 1 : 0))} className="db-empty-cell">
                  Memuat data...
                </td>
              </tr>
            )}
            {data &&
              data.rows.map((row, rIdx) => (
                <tr key={rIdx}>
                  {canEdit && (
                    <td className="db-act-col">
                      <button
                        type="button"
                        className="icon-btn db-row-del"
                        onClick={() => requestDeleteRow(rIdx)}
                        title="Hapus baris ini"
                        aria-label="Hapus baris"
                      >
                        <Icon name="trash" size={12} />
                      </button>
                    </td>
                  )}
                  {row.map((cell, cIdx) => {
                    const isEditing =
                      editingCell?.rowIdx === rIdx && editingCell?.colIdx === cIdx;
                    const display = formatDbValue(cell);
                    return (
                      <td
                        key={cIdx}
                        className={`${cell.type === "null" ? "db-null" : "db-mono"}${
                          canEdit ? " db-editable" : ""
                        }`}
                        onDoubleClick={() => startCellEdit(rIdx, cIdx)}
                        title={canEdit ? `${display}\n(Double-click untuk edit)` : display}
                      >
                        {isEditing ? (
                          <input
                            type="text"
                            className="db-cell-input"
                            value={editingCell.draft}
                            autoFocus
                            onChange={(e) =>
                              setEditingCell({ ...editingCell, draft: e.target.value })
                            }
                            onKeyDown={(e) => {
                              if (e.key === "Enter") {
                                e.preventDefault();
                                commitCellEdit();
                              } else if (e.key === "Escape") {
                                setEditingCell(null);
                              }
                            }}
                            onBlur={() => setEditingCell(null)}
                          />
                        ) : (
                          display
                        )}
                      </td>
                    );
                  })}
                </tr>
              ))}
            {data && data.rows.length === 0 && (
              <tr>
                <td
                  colSpan={Math.max(1, data.columns.length + (canEdit ? 1 : 0))}
                  className="db-empty-cell"
                >
                  Tabel kosong atau tidak ada baris yang cocok.
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>

      <div className="db-pager">
        <span className="db-muted">
          Halaman {page}
          {data?.total_estimate !== null && data?.total_estimate !== undefined
            ? ` · Total ~${data.total_estimate} baris`
            : ""}
          {data ? ` · ${data.elapsed_ms} ms` : ""}
        </span>
        <div className="db-pager-controls">
          <select
            value={limit}
            onChange={(e) => {
              setLimit(Number(e.target.value));
              setPage(1);
            }}
            aria-label="Baris per halaman"
          >
            <option value={50}>50 / hal</option>
            <option value={100}>100 / hal</option>
            <option value={250}>250 / hal</option>
            <option value={500}>500 / hal</option>
          </select>
          <button
            type="button"
            className="btn btn-sm"
            disabled={page <= 1 || loading}
            onClick={() => setPage((p) => Math.max(1, p - 1))}
          >
            ← Prev
          </button>
          <button
            type="button"
            className="btn btn-sm"
            disabled={!data?.has_more || loading}
            onClick={() => setPage((p) => p + 1)}
          >
            Next →
          </button>
        </div>
      </div>

      {showInsert && schema && (
        <Modal
          title={`Tambah Baris ke ${table}`}
          onClose={() => setShowInsert(false)}
          footer={
            <>
              <button type="button" className="btn" onClick={() => setShowInsert(false)}>
                Batal
              </button>
              <button type="submit" form="db-insert-row-form" className="btn btn-primary">
                Preview SQL
              </button>
            </>
          }
        >
          <form id="db-insert-row-form" className="db-form" onSubmit={submitInsertModal}>
            {schema.columns.map((col) => (
              <label key={col.name} className="field">
                <span>
                  {col.name}{" "}
                  <small className="db-muted">
                    ({col.data_type}
                    {col.is_primary_key ? ", PK" : ""}
                    {col.nullable ? ", nullable" : ", NOT NULL"})
                  </small>
                </span>
                <input
                  type="text"
                  value={insertDraft[col.name] ?? ""}
                  onChange={(e) =>
                    setInsertDraft((prev) => ({ ...prev, [col.name]: e.target.value }))
                  }
                  placeholder={
                    col.default_value
                      ? `default: ${col.default_value}`
                      : col.nullable
                        ? "NULL (kosongkan)"
                        : ""
                  }
                />
              </label>
            ))}
          </form>
        </Modal>
      )}

      {pending && (
        <DbSqlPreviewModal
          title={
            pending.op === "delete"
              ? "Konfirmasi Hapus Baris"
              : pending.op === "insert"
                ? "Konfirmasi Tambah Baris"
                : "Konfirmasi Edit Sel"
          }
          description="Periksa preview query berikut sebelum diterapkan ke database:"
          sqlPreview={pending.sqlPreview}
          confirmLabel={
            pending.op === "delete"
              ? "Hapus Baris"
              : pending.op === "insert"
                ? "Tambah Baris"
                : "Simpan Perubahan"
          }
          danger={pending.op === "delete"}
          busy={mutating}
          onConfirm={() => void executePending()}
          onCancel={() => setPending(null)}
        />
      )}
    </div>
  );
}
