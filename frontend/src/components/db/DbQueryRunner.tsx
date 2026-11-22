import { useEffect, useState } from "react";
import { api, errorMessage } from "../../api";
import {
  formatDbValue,
  loadQueryHistory,
  pushQueryHistory,
  rowsToCsv,
  rowsToJson,
} from "../../dbClient";
import type { DbConnection, DbQueryResult } from "../../types";
import { Icon } from "../Icon";

interface DbQueryRunnerProps {
  connection: DbConnection;
  database: string;
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

export function DbQueryRunner({ connection, database }: DbQueryRunnerProps): JSX.Element {
  const [sql, setSql] = useState("SELECT 1;");
  const [running, setRunning] = useState(false);
  const [result, setResult] = useState<DbQueryResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [history, setHistory] = useState<string[]>([]);
  const [showHistory, setShowHistory] = useState(false);

  useEffect(() => {
    setHistory(loadQueryHistory(connection.id));
  }, [connection.id]);

  const runQuery = async () => {
    const trimmed = sql.trim();
    if (!trimmed || running) return;
    setRunning(true);
    setError(null);
    try {
      const res = await api.dbExecuteQuery(connection.id, database, trimmed, 500);
      setResult(res);
      setHistory(pushQueryHistory(connection.id, trimmed));
    } catch (err) {
      setError(errorMessage(err));
      setResult(null);
    } finally {
      setRunning(false);
    }
  };

  return (
    <div className="db-query-runner">
      <div className="db-query-toolbar">
        <button
          type="button"
          className="btn btn-primary btn-sm"
          onClick={() => void runQuery()}
          disabled={running || !sql.trim()}
        >
          <Icon name="play" size={13} />
          <span>{running ? "Menjalankan..." : "Jalankan (Ctrl+Enter)"}</span>
        </button>
        <button
          type="button"
          className="btn btn-sm"
          onClick={() => setShowHistory((v) => !v)}
          disabled={history.length === 0}
        >
          Riwayat ({history.length})
        </button>
        {connection.read_only && (
          <span className="db-ro-badge">READ-ONLY</span>
        )}
        <div style={{ flex: 1 }} />
        {result && result.columns.length > 0 && (
          <>
            <button
              type="button"
              className="btn btn-sm"
              onClick={() =>
                triggerDownload(
                  rowsToCsv(result.columns, result.rows),
                  `query-${database || "db"}.csv`,
                  "text/csv;charset=utf-8",
                )
              }
            >
              Export CSV
            </button>
            <button
              type="button"
              className="btn btn-sm"
              onClick={() =>
                triggerDownload(
                  rowsToJson(result.columns, result.rows),
                  `query-${database || "db"}.json`,
                  "application/json",
                )
              }
            >
              Export JSON
            </button>
          </>
        )}
      </div>

      {showHistory && history.length > 0 && (
        <div className="db-history-list">
          {history.map((item, i) => (
            <button
              key={i}
              type="button"
              className="db-history-item"
              onClick={() => {
                setSql(item);
                setShowHistory(false);
              }}
              title={item}
            >
              {item}
            </button>
          ))}
        </div>
      )}

      <textarea
        className="db-sql-input"
        value={sql}
        onChange={(e) => setSql(e.target.value)}
        onKeyDown={(e) => {
          if ((e.ctrlKey || e.metaKey) && e.key === "Enter") {
            e.preventDefault();
            void runQuery();
          }
        }}
        placeholder="Tulis query SQL di sini... (Ctrl+Enter untuk menjalankan)"
        spellCheck={false}
        rows={6}
      />

      {error && <div className="db-banner is-err">{error}</div>}

      {result && (
        <div className="db-query-result">
          <div className="db-status-bar">
            {result.message ? (
              <span>{result.message}</span>
            ) : (
              <span>
                {result.rows.length} baris{result.truncated ? " (dibatasi 500 baris)" : ""}
              </span>
            )}
            <span className="db-muted">{result.elapsed_ms} ms</span>
          </div>

          {result.columns.length > 0 && (
            <div className="db-table-wrap">
              <table className="db-table">
                <thead>
                  <tr>
                    {result.columns.map((col, idx) => (
                      <th key={`${col}-${idx}`}>{col}</th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {result.rows.map((row, rIdx) => (
                    <tr key={rIdx}>
                      {row.map((cell, cIdx) => (
                        <td
                          key={cIdx}
                          className={cell.type === "null" ? "db-null" : "db-mono"}
                          title={formatDbValue(cell)}
                        >
                          {formatDbValue(cell)}
                        </td>
                      ))}
                    </tr>
                  ))}
                  {result.rows.length === 0 && (
                    <tr>
                      <td colSpan={result.columns.length} className="db-empty-cell">
                        Tidak ada baris hasil.
                      </td>
                    </tr>
                  )}
                </tbody>
              </table>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
