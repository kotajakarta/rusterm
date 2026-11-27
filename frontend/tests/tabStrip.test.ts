import test from "node:test";
import assert from "node:assert/strict";
import { computeTabScrollState, filterOpenTabs } from "../src/tabStrip.ts";

test("computeTabScrollState detects overflow and left/right scroll availability", () => {
  assert.deepEqual(computeTabScrollState(0, 500, 400), {
    hasOverflow: false,
    canScrollLeft: false,
    canScrollRight: false,
  });

  assert.deepEqual(computeTabScrollState(0, 500, 900), {
    hasOverflow: true,
    canScrollLeft: false,
    canScrollRight: true,
  });

  assert.deepEqual(computeTabScrollState(120, 500, 900), {
    hasOverflow: true,
    canScrollLeft: true,
    canScrollRight: true,
  });

  assert.deepEqual(computeTabScrollState(400, 500, 900), {
    hasOverflow: true,
    canScrollLeft: true,
    canScrollRight: false,
  });
});

test("filterOpenTabs filters by tab title or server name case-insensitively", () => {
  const tabs = [
    { id: "1", title: "prod-api", serverName: "Production VPS" },
    { id: "2", title: "db-replica", serverName: "Staging DB" },
    { id: "3", title: "logs", serverName: "Production VPS" },
  ];

  assert.equal(filterOpenTabs(tabs, "").length, 3);
  assert.deepEqual(
    filterOpenTabs(tabs, "PROD").map((t) => t.id),
    ["1", "3"],
  );
  assert.deepEqual(
    filterOpenTabs(tabs, "replica").map((t) => t.id),
    ["2"],
  );
});
