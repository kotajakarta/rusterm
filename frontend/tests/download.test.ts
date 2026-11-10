import test from "node:test";
import assert from "node:assert/strict";
import { downloadBlob } from "../src/download.ts";

test("downloadBlob routes to window.TermDeckApp.saveBlob when available", async () => {
  let calledWith: { base64Data: string; fileName: string; mimeType: string } | null = null;

  (globalThis as any).window = {
    TermDeckApp: {
      saveBlob: (base64Data: string, fileName: string, mimeType: string) => {
        calledWith = { base64Data, fileName, mimeType };
      },
    },
  };

  const sampleJson = JSON.stringify({ hello: "world" });
  const blob = new Blob([sampleJson], { type: "application/json" });
  await downloadBlob(blob, "backup.tdk");

  assert.ok(calledWith !== null, "window.TermDeckApp.saveBlob should be called");
  assert.equal(calledWith.fileName, "backup.tdk");
  assert.equal(calledWith.mimeType, "application/json");

  const decoded = Buffer.from(calledWith.base64Data, "base64").toString("utf-8");
  assert.equal(decoded, sampleJson);

  delete (globalThis as any).window;
});
