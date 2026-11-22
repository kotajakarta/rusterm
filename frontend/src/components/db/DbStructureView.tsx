import type { DbTableSchema } from "../../types";

interface DbStructureViewProps {
  schema: DbTableSchema | null;
  loading: boolean;
}

export function DbStructureView({ schema, loading }: DbStructureViewProps): JSX.Element {
  if (loading && !schema) {
    return <div className="db-empty">Memuat struktur tabel...</div>;
  }
  if (!schema) {
    return <div className="db-empty">Pilih tabel untuk melihat strukturnya.</div>;
  }

  return (
    <div className="db-structure">
      <div className="db-section-title">
        Kolom ({schema.columns.length})
        {schema.primary_keys.length > 0 && (
          <span className="db-pill">PK: {schema.primary_keys.join(", ")}</span>
        )}
      </div>
      <div className="db-table-wrap">
        <table className="db-table">
          <thead>
            <tr>
              <th>Kolom</th>
              <th>Tipe Data</th>
              <th>Nullable</th>
              <th>Default</th>
              <th>Key</th>
            </tr>
          </thead>
          <tbody>
            {schema.columns.map((c) => (
              <tr key={c.name}>
                <td className="db-mono">{c.name}</td>
                <td className="db-mono db-type">{c.data_type}</td>
                <td>{c.nullable ? "YES" : "NO"}</td>
                <td className="db-mono">{c.default_value ?? <span className="db-null">NULL</span>}</td>
                <td>{c.is_primary_key ? <span className="db-pk-badge">PK</span> : "—"}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      {schema.indexes.length > 0 && (
        <>
          <div className="db-section-title" style={{ marginTop: 16 }}>
            Index ({schema.indexes.length})
          </div>
          <div className="db-table-wrap">
            <table className="db-table">
              <thead>
                <tr>
                  <th>Nama Index</th>
                  <th>Kolom</th>
                  <th>Unique</th>
                </tr>
              </thead>
              <tbody>
                {schema.indexes.map((idx) => (
                  <tr key={idx.name}>
                    <td className="db-mono">{idx.name}</td>
                    <td className="db-mono">{idx.columns.join(", ")}</td>
                    <td>{idx.unique ? "UNIQUE" : "NO"}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </>
      )}
    </div>
  );
}
