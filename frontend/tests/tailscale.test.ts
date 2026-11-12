import assert from "node:assert/strict";
import { test } from "node:test";

import { getTailscaleDeviceName } from "../src/types.ts";

test("getTailscaleDeviceName: extracts machine name from MagicDNS FQDN when hostname is localhost", () => {
  const dev = {
    name: "oracle-server.tail4e1750.ts.net",
    hostname: "localhost",
  };
  assert.equal(getTailscaleDeviceName(dev), "oracle-server");
});

test("getTailscaleDeviceName: uses MagicDNS prefix when renamed even if OS hostname is generic ubuntu", () => {
  const dev = {
    name: "prod-web-01.my-tailnet.ts.net",
    hostname: "ubuntu-2204",
  };
  assert.equal(getTailscaleDeviceName(dev), "prod-web-01");
});

test("getTailscaleDeviceName: falls back to hostname if MagicDNS is localhost and hostname is meaningful", () => {
  const dev = {
    name: "localhost.tailnet.ts.net",
    hostname: "workstation-pc",
  };
  assert.equal(getTailscaleDeviceName(dev), "workstation-pc");
});

test("getTailscaleDeviceName: falls back to hostname if name is empty", () => {
  const dev = {
    name: "",
    hostname: "nas-storage",
  };
  assert.equal(getTailscaleDeviceName(dev), "nas-storage");
});

test("getTailscaleDeviceName: returns localhost if both are localhost", () => {
  const dev = {
    name: "localhost.tailnet.ts.net",
    hostname: "localhost",
  };
  assert.equal(getTailscaleDeviceName(dev), "localhost");
});

test("getTailscaleDeviceName: handles simple node name without domain", () => {
  const dev = {
    name: "vps-singapore",
    hostname: "localhost",
  };
  assert.equal(getTailscaleDeviceName(dev), "vps-singapore");
});
