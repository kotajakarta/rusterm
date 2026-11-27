import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import {
  STORAGE_PROVIDER_PRESETS,
  buildBreadcrumbs,
  formatTransferDirection,
  getProviderPreset,
  validateBucketName,
} from "../src/storageHelpers.ts";

test("STORAGE_PROVIDER_PRESETS provides R2, AWS S3, MinIO, and Custom S3", () => {
  assert.deepEqual(
    STORAGE_PROVIDER_PRESETS.map((p) => p.id),
    ["r2", "aws", "minio", "custom"],
  );
  const r2 = getProviderPreset("r2");
  assert.equal(r2.defaultRegion, "auto");
  assert.equal(r2.defaultPathStyle, true);
  const minio = getProviderPreset("minio");
  assert.ok(minio.defaultEndpoint.startsWith("http://"));
  assert.equal(minio.defaultPathStyle, true);
  const aws = getProviderPreset("aws");
  assert.equal(aws.defaultRegion, "ap-southeast-1");
});

test("validateBucketName validates S3 bucket names properly", () => {
  assert.equal(validateBucketName("my-bucket-01"), null);
  assert.notEqual(validateBucketName("ab"), null);
  assert.notEqual(validateBucketName("UpperCase"), null);
  assert.notEqual(validateBucketName("-starts-dash"), null);
  assert.notEqual(validateBucketName("double..dot"), null);
});

test("buildBreadcrumbs builds prefix segments accurately", () => {
  assert.deepEqual(buildBreadcrumbs(""), []);
  assert.deepEqual(buildBreadcrumbs("backups/2026/db/"), [
    { label: "backups", prefix: "backups/" },
    { label: "2026", prefix: "backups/2026/" },
    { label: "db", prefix: "backups/2026/db/" },
  ]);
});

test("formatTransferDirection returns readable labels", () => {
  assert.ok(formatTransferDirection("ssh_to_s3").includes("SSH"));
  assert.ok(formatTransferDirection("s3_to_ssh").includes("SSH"));
  assert.ok(formatTransferDirection("s3_to_s3").includes("S3"));
});

test("storageApi sends X-Token header expected by request_guard", () => {
  const src = readFileSync(new URL("../src/storageApi.ts", import.meta.url), "utf8");
  assert.match(src, /headers\["X-Token"\]\s*=\s*token/);
  assert.doesNotMatch(src, /X-RusTerm-Token/);
});
