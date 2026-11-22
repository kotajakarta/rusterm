import { useState } from "react";
import { api, errorMessage } from "../../api";
import type { DbCreateColumnSpec, DbEngine } from "../../types";
import { Modal } from "../Modal";

interface DbCreateTableModalProps {
  connId: number;
  engine: DbEngine;
  database: string;
  onCreated: (tableName: string) => void;
  onClose: () => void;
}

const DEFAULT_TYPES: Record<DbEngine, string[]> = {
  postgres: ["SERIAL", "INTEGER", "BIGINT", "TEXT", "VARCHAR(255)", "BOOLEAN", "NUMERIC(12,2)", "JSONB", "TIMESTAMPTZ"],
  mysql: ["INT AUTO_INCREMENT", "INT", "BIGINT", "VARCHAR(255)", "TEXT", "TINYINT(1)", "DECIMAL(12,2)", "JSON", "DATETIME"],
  sqlite: ["INTEGER", "TEXT", "REAL", "BLOB", "NUMERIC"],
};

export function DbCreateTableModal({
  connId,
  engine,
  database,
  onCreated,
  onClose,
}: DbCreateTableModalProps): JSX.Element {
  const [tableName, setTableName] = useState("");
  const [columns, setColumns] = useState<DbCreateColumnSpec[]>([
    { name: "id", data_type: engine === "postgres" ? "SERIAL" : "INTEGER", nullable: false, primary_key: true, default_value: "" },
    { name: "name", data_type: engine === "sqlite" ? "TEXT" : "VARCHAR(255)", nullable: false, primary_key: false, default_value: "" },
  ]);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const updateCol = (idx: number, patch: Partial<DbCreateColumnSpec>) => {
    setColumns((prev) => prev.map((c, i) => (i === idx ? { ...c, ...patch } : c)));
  };

  const addCol = () => {
    setColumns((prev) => [
      ...prev,
      { name: "", data_type: engine === "sqlite" ? "TEXT" : "VARCHAR(255)", nullable: true, primary_key: false, default_value: "" },
    ]);
  };

  const removeCol = (idx: number) => {
    setColumns((prev) => prev.filter((_, i) => i !== idx));
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    const trimmedTable = tableName.trim();
    if (!trimmedTable) {
      setError("Nama tabel wajib diisi.");
      return;
    }
    const validCols = columns
      .map((c) => ({
        ...c,
        name: c.name.trim(),
        data_type: c.data_type.trim(),
        default_value: c.default_value?.trim() || null,
      }))
      .filter((c) => c.name.length > 0);
    if (validCols.length === 0) {
      setError("Minimal satu kolom harus diisi.");
      return;
    }

    setSaving(true);
    try {
      await api.dbCreateTable(connId, {
        db: database,
        table: trimmedTable,
        columns: validCols,
      });
      onCreated(trimmedTable);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setSaving(false);
    }
  };

  return (
    <Modal
      title={`Buat Tabel Baru (${database})`}
      onClose={onClose}
      wide
      footer={
        <>
          <button type="button" className="btn" onClick={onClose} disabled={saving}>
            Batal
          </button>
          <button type="submit" form="db-create-table-form" className="btn btn-primary" disabled={saving}>
            {saving ? "Membuat..." : "Buat Tabel"}
          </button>
        </>
      }
    >
      <form id="db-create-table-form" className="db-form" onSubmit={(e) => void handleSubmit(e)}>
        <label className="field">
          <span>Nama Tabel</span>
          <input
            type="text"
            value={tableName}
            onChange={(e) => setTableName(e.target.value)}
            placeholder="contoh: users"
            required
          />
        </label>

        <div className="db-cols-header">
          <span>Kolom</span>
          <button type="button" className="btn btn-sm" onClick={addCol}>
            + Tambah Kolom
          </button>
        </div>

        <datalist id="db-col-types">
          {DEFAULT_TYPES[engine].map((t) => (
            <option key={t} value={t} />
          ))}
        </datalist>

        <div className="db-cols-editor">
          {columns.map((col, idx) => (
            <div key={idx} className="db-col-row">
              <input
                type="text"
                placeholder="nama_kolom"
                value={col.name}
                onChange={(e) => updateCol(idx, { name: e.target.value })}
                required
              />
              <input
                type="text"
                list="db-col-types"
                placeholder="tipe data"
                value={col.data_type}
                onChange={(e) => updateCol(idx, { data_type: e.target.value })}
                required
              />
              <input
                type="text"
                placeholder="default (opsional)"
                value={col.default_value ?? ""}
                onChange={(e) => updateCol(idx, { default_value: e.target.value })}
              />
              <label className="db-mini-check" title="Primary Key">
                <input
                  type="checkbox"
                  checked={col.primary_key}
                  onChange={(e) =>
                    updateCol(idx, {
                      primary_key: e.target.checked,
                      nullable: e.target.checked ? false : col.nullable,
                    })
                  }
                />
                PK
              </label>
              <label className="db-mini-check" title="Nullable">
                <input
                  type="checkbox"
                  checked={col.nullable}
                  disabled={col.primary_key}
                  onChange={(e) => updateCol(idx, { nullable: e.target.checked })}
                />
                NULL
              </label>
              <button
                type="button"
                className="icon-btn"
                onClick={() => removeCol(idx)}
                disabled={columns.length <= 1}
                title="Hapus kolom"
                aria-label="Hapus kolom"
              >
                ×
              </button>
            </div>
          ))}
        </div>

        {error && <div className="db-banner is-err">{error}</div>}
      </form>
    </Modal>
  );
}
