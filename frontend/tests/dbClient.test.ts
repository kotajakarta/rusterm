import assert from "node:assert/strict";
import test from "node:test";
import {
  buildSqlPreview,
  defaultPortForEngine,
  detectDbEngineFromContainer,
  formatDbValue,
  parseCellInput,
  qualifyTableName,
  quoteTablePreview,
  rowsToCsv,
  rowsToJson,
} from "../src/dbClient.ts";
import { formatSyncStats } from "../src/syncLogic.ts";
import type { DbValue } from "../src/types.ts";

test("defaultPortForEngine returns standard ports", () => {
  assert.equal(defaultPortForEngine("postgres"), 5432);
  assert.equal(defaultPortForEngine("mysql"), 3306);
  assert.equal(defaultPortForEngine("sqlite"), 0);
});

test("formatDbValue formats all DbValue variants", () => {
  assert.equal(formatDbValue({ type: "null" }), "NULL");
  assert.equal(formatDbValue({ type: "bool", value: true }), "true");
  assert.equal(formatDbValue({ type: "int", value: 42 }), "42");
  assert.equal(formatDbValue({ type: "text", value: { value: "hi", truncated: true } }), "hi…");
  assert.equal(formatDbValue({ type: "json", value: { a: 1 } }), '{"a":1}');
  assert.equal(
    formatDbValue({ type: "bytes", value: { size: 2, preview_hex: "cafe" } }),
    "<BLOB 2 B: 0xcafe>",
  );
});

test("parseCellInput handles null, numbers, booleans, and json", () => {
  assert.equal(parseCellInput("", "text", true), null);
  assert.equal(parseCellInput("NULL", "integer", true), null);
  assert.equal(parseCellInput("123", "integer", false), 123);
  assert.equal(parseCellInput("3.5", "numeric", false), 3.5);
  assert.equal(parseCellInput("true", "boolean", false), true);
  assert.deepEqual(parseCellInput('{"k":1}', "jsonb", false), { k: 1 });
});

test("buildSqlPreview quotes identifiers and escapes literals safely", () => {
  assert.equal(
    buildSqlPreview("postgres", "update", "users", { id: 5 }, { name: "O'Brien" }),
    `UPDATE "users" SET "name" = 'O''Brien' WHERE "id" = 5;`,
  );
  assert.equal(
    buildSqlPreview("postgres", "update", "auth.users", { id: 5 }, { name: "Ada" }),
    `UPDATE "auth"."users" SET "name" = 'Ada' WHERE "id" = 5;`,
  );
  assert.equal(
    buildSqlPreview("mysql", "delete", "items", { id: 9 }, {}),
    "DELETE FROM `items` WHERE `id` = 9;",
  );
});

test("qualifyTableName and quoteTablePreview handle Postgres schemas", () => {
  assert.equal(qualifyTableName("postgres", "users", "auth"), "auth.users");
  assert.equal(qualifyTableName("postgres", "auth.users", "public"), "auth.users");
  assert.equal(qualifyTableName("postgres", "users", ""), "users");
  assert.equal(qualifyTableName("mysql", "users", "auth"), "users");
  assert.equal(qualifyTableName("sqlite", "users", "auth"), "users");

  assert.equal(quoteTablePreview("postgres", "auth.users"), `"auth"."users"`);
  assert.equal(quoteTablePreview("postgres", "users"), `"users"`);
  assert.equal(quoteTablePreview("mysql", "a.b"), "`a.b`");
});

test("rowsToCsv and rowsToJson export properly", () => {
  const cols = ["id", "note"];
  const rows: DbValue[][] = [
    [
      { type: "int", value: 1 },
      { type: "text", value: { value: 'hello, "world"', truncated: false } },
    ],
    [{ type: "int", value: 2 }, { type: "null" }],
  ];

  const csv = rowsToCsv(cols, rows);
  assert.equal(csv, 'id,note\r\n1,"hello, ""world"""\r\n2,');

  const parsed = JSON.parse(rowsToJson(cols, rows)) as Record<string, unknown>[];
  assert.deepEqual(parsed, [
    { id: 1, note: 'hello, "world"' },
    { id: 2, note: null },
  ]);
});

test("detectDbEngineFromContainer identifies postgres, mysql/mariadb, and non-db containers", () => {
  assert.equal(
    detectDbEngineFromContainer({ image: "postgres:16-alpine", ports: [] }),
    "postgres",
  );
  assert.equal(
    detectDbEngineFromContainer({ image: "custom-img", ports: [{ container_port: 5432 }] }),
    "postgres",
  );
  assert.equal(
    detectDbEngineFromContainer({ image: "mariadb:11", ports: [] }),
    "mysql",
  );
  assert.equal(
    detectDbEngineFromContainer({ image: "mysql:8", ports: [{ container_port: 3306 }] }),
    "mysql",
  );
  assert.equal(
    detectDbEngineFromContainer({ image: "nginx:alpine", ports: [{ container_port: 80 }] }),
    null,
  );
});

test("formatSyncStats includes db_connections added and updated counts", () => {
  const msg = formatSyncStats({
    servers_added: 1,
    servers_updated: 0,
    bookmarks_added: 0,
    snippets_added: 0,
    db_connections_added: 2,
    db_connections_updated: 1,
  });
  assert.match(msg, /\+1 server ditambahkan/);
  assert.match(msg, /\+2 koneksi DB ditambahkan/);
  assert.match(msg, /1 koneksi DB diperbarui/);
});
