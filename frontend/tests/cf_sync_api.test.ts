import assert from "node:assert/strict";
import { test } from "node:test";
import { api } from "../src/api.ts";
import type {
  CFSyncMetadata,
  CFSyncNowIn,
  CFSyncNowOut,
  CFSyncPullIn,
  CFSyncPullOut,
  CFSyncPushIn,
  CFSyncPushOut,
  CFSyncStatusOut,
} from "../src/types.ts";

test("CFSync types validate correct schema contracts", () => {
  const meta: CFSyncMetadata = {
    device_name: "Linux Desktop",
    updated_at: 1730615000,
    servers_count: 5,
    snippets_count: 2,
    version: 1,
  };
  assert.equal(meta.device_name, "Linux Desktop");
  assert.equal(meta.servers_count, 5);

  const status: CFSyncStatusOut = {
    configured: true,
    has_remote: true,
    remote_metadata: meta,
    local_counts: { servers_count: 4, snippets_count: 2 },
  };
  assert.equal(status.has_remote, true);

  const syncNowIn: CFSyncNowIn = {
    account_id: 1,
    pin: "123456",
    device_name: "Linux Workstation",
  };
  assert.equal(syncNowIn.pin, "123456");

  const pullIn: CFSyncPullIn = {
    account_id: 1,
    pin: "123456",
    replace_local: false,
  };
  assert.equal(pullIn.replace_local, false);
});

test("api exposes CF sync methods", () => {
  assert.equal(typeof (api as any).cfGetSyncStatus, "function");
  assert.equal(typeof (api as any).cfSyncNow, "function");
  assert.equal(typeof (api as any).cfSyncPush, "function");
  assert.equal(typeof (api as any).cfSyncPull, "function");
});
