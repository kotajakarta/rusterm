import type { DbEngine, DbValue } from "./types";

const HISTORY_PREFIX = "rusterm.db.history.";
const MAX_HISTORY = 30;

export function defaultPortForEngine(engine: DbEngine): number {
  if (engine === "postgres") return 5432;
  if (engine === "mysql") return 3306;
  return 0;
}

export function engineLabel(engine: DbEngine): string {
  if (engine === "postgres") return "PostgreSQL";
  if (engine === "mysql") return "MySQL / MariaDB";
  return "SQLite";
}

export function formatDbValue(v: DbValue): string {
  switch (v.type) {
    case "null":
      return "NULL";
    case "bool":
      return v.value ? "true" : "false";
    case "int":
    case "float":
      return String(v.value);
    case "text":
      return v.value.truncated ? `${v.value.value}…` : v.value.value;
    case "json":
      return JSON.stringify(v.value);
    case "bytes":
      return `<BLOB ${v.value.size} B: 0x${v.value.preview_hex}>`;
  }
}

export function dbValueToRaw(v: DbValue): unknown {
  switch (v.type) {
    case "null":
      return null;
    case "bool":
    case "int":
    case "float":
    case "json":
      return v.value;
    case "text":
      return v.value.value;
    case "bytes":
      return v.value.preview_hex;
  }
}

export function parseCellInput(raw: string, dataType: string, nullable: boolean): unknown {
  const trimmed = raw.trim();
  if (nullable && (trimmed === "" || trimmed.toUpperCase() === "NULL")) {
    return null;
  }
  const lower = dataType.toLowerCase();
  if (lower.includes("bool")) {
    if (trimmed === "true" || trimmed === "1") return true;
    if (trimmed === "false" || trimmed === "0") return false;
  }
  if (
    lower.includes("int") ||
    lower.includes("serial") ||
    lower.includes("bigint") ||
    lower.includes("smallint")
  ) {
    if (/^-?\d+$/.test(trimmed)) {
      const n = Number(trimmed);
      if (Number.isSafeInteger(n)) return n;
    }
  }
  if (
    lower.includes("float") ||
    lower.includes("double") ||
    lower.includes("real") ||
    lower.includes("numeric") ||
    lower.includes("decimal")
  ) {
    if (trimmed !== "" && !Number.isNaN(Number(trimmed))) {
      return Number(trimmed);
    }
  }
  if (lower.includes("json")) {
    try {
      return JSON.parse(raw);
    } catch {
      return raw;
    }
  }
  return raw;
}

function quoteIdentPreview(engine: DbEngine, ident: string): string {
  if (engine === "mysql") return `\`${ident.replace(/`/g, "``")}\``;
  return `"${ident.replace(/"/g, '""')}"`;
}

export function qualifyTableName(engine: DbEngine, table: string, schema?: string): string {
  if (engine !== "postgres") return table;
  const s = schema?.trim();
  if (!s || table.includes(".")) return table;
  return `${s}.${table}`;
}

export function quoteTablePreview(engine: DbEngine, table: string): string {
  if (engine === "postgres") {
    const dotIdx = table.indexOf(".");
    if (dotIdx > 0 && dotIdx < table.length - 1) {
      const schemaPart = table.slice(0, dotIdx);
      const tablePart = table.slice(dotIdx + 1);
      return `${quoteIdentPreview(engine, schemaPart)}.${quoteIdentPreview(engine, tablePart)}`;
    }
  }
  return quoteIdentPreview(engine, table);
}

function formatLiteralPreview(val: unknown): string {
  if (val === null || val === undefined) return "NULL";
  if (typeof val === "number" || typeof val === "boolean") return String(val);
  const str = typeof val === "string" ? val : JSON.stringify(val);
  return `'${str.replace(/'/g, "''")}'`;
}

export function buildSqlPreview(
  engine: DbEngine,
  op: "insert" | "update" | "delete",
  table: string,
  pk: Record<string, unknown>,
  values: Record<string, unknown>,
): string {
  const qt = quoteTablePreview(engine, table);
  if (op === "insert") {
    const entries = Object.entries(values);
    const cols = entries.map(([k]) => quoteIdentPreview(engine, k)).join(", ");
    const vals = entries.map(([, v]) => formatLiteralPreview(v)).join(", ");
    return `INSERT INTO ${qt} (${cols}) VALUES (${vals});`;
  }
  const where = Object.entries(pk)
    .map(([k, v]) =>
      v === null
        ? `${quoteIdentPreview(engine, k)} IS NULL`
        : `${quoteIdentPreview(engine, k)} = ${formatLiteralPreview(v)}`,
    )
    .join(" AND ");
  if (op === "delete") {
    return `DELETE FROM ${qt} WHERE ${where};`;
  }
  const sets = Object.entries(values)
    .map(([k, v]) => `${quoteIdentPreview(engine, k)} = ${formatLiteralPreview(v)}`)
    .join(", ");
  return `UPDATE ${qt} SET ${sets} WHERE ${where};`;
}

function escapeCsvCell(s: string): string {
  if (/[",\r\n]/.test(s)) {
    return `"${s.replace(/"/g, '""')}"`;
  }
  return s;
}

export function rowsToCsv(columns: string[], rows: DbValue[][]): string {
  const header = columns.map(escapeCsvCell).join(",");
  const lines = rows.map((row) =>
    row
      .map((cell) => {
        if (cell.type === "null") return "";
        return escapeCsvCell(formatDbValue(cell));
      })
      .join(","),
  );
  return [header, ...lines].join("\r\n");
}

export function rowsToJson(columns: string[], rows: DbValue[][]): string {
  const objs = rows.map((row) => {
    const obj: Record<string, unknown> = {};
    columns.forEach((col, i) => {
      obj[col] = row[i] ? dbValueToRaw(row[i]) : null;
    });
    return obj;
  });
  return JSON.stringify(objs, null, 2);
}

export function loadQueryHistory(connId: number): string[] {
  try {
    if (typeof localStorage === "undefined") return [];
    const raw = localStorage.getItem(`${HISTORY_PREFIX}${connId}`);
    if (!raw) return [];
    const parsed: unknown = JSON.parse(raw);
    return Array.isArray(parsed) ? parsed.filter((x): x is string => typeof x === "string") : [];
  } catch {
    return [];
  }
}

export function pushQueryHistory(connId: number, sql: string): string[] {
  const trimmed = sql.trim();
  if (!trimmed) return loadQueryHistory(connId);
  const prev = loadQueryHistory(connId).filter((q) => q !== trimmed);
  const next = [trimmed, ...prev].slice(0, MAX_HISTORY);
  try {
    if (typeof localStorage !== "undefined") {
      localStorage.setItem(`${HISTORY_PREFIX}${connId}`, JSON.stringify(next));
    }
  } catch {
    // ignore storage errors
  }
  return next;
}
