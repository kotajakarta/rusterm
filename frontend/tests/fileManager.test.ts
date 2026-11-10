import assert from "node:assert/strict";
import test from "node:test";

import {
  bitsToMode,
  filterAndSortEntries,
  joinRemotePath,
  modeToRwx,
  parentDirPath,
  parseModeBits,
  QUICK_TERMINAL_COMMANDS,
  splitBreadcrumbs,
} from "../src/fileManager.ts";
import {
  accessOf,
  buildMode,
  isWithin,
  linkDestination,
  parseJsonc,
  previewKind,
} from "../src/components/files/fileUtils.ts";
import type { DirEntry, FileEntry } from "../src/types.ts";

test("splitBreadcrumbs breaks absolute and home paths into navigable segments", () => {
  assert.deepEqual(splitBreadcrumbs("/"), [{ label: "/", path: "/" }]);
  assert.deepEqual(splitBreadcrumbs("/var/www/html"), [
    { label: "/", path: "/" },
    { label: "var", path: "/var" },
    { label: "www", path: "/var/www" },
    { label: "html", path: "/var/www/html" },
  ]);
  assert.deepEqual(splitBreadcrumbs("~/projects/app"), [
    { label: "~", path: "~" },
    { label: "projects", path: "~/projects" },
    { label: "app", path: "~/projects/app" },
  ]);
});

test("parentDirPath and joinRemotePath normalize POSIX paths", () => {
  assert.equal(parentDirPath("/var/www/html"), "/var/www");
  assert.equal(parentDirPath("/var"), "/");
  assert.equal(parentDirPath("/"), "/");
  assert.equal(parentDirPath("~/app"), "~");
  assert.equal(joinRemotePath("/", "etc"), "/etc");
  assert.equal(joinRemotePath("/var/www/", "html"), "/var/www/html");
  assert.equal(joinRemotePath("~", "logs"), "~/logs");
});

test("modeToRwx, parseModeBits, and bitsToMode convert between octal and rwx permissions", () => {
  assert.equal(modeToRwx("0755", true), "drwxr-xr-x");
  assert.equal(modeToRwx("644", false), "-rw-r--r--");
  assert.equal(modeToRwx("0600", false), "-rw-------");

  const parsed = parseModeBits("0750");
  assert.deepEqual(parsed, {
    owner: [true, true, true],
    group: [true, false, true],
    other: [false, false, false],
  });
  assert.equal(bitsToMode(parsed), "0750");
});

test("filterAndSortEntries hides dotfiles by default, keeps folders on top, and sorts by field", () => {
  const sample: DirEntry[] = [
    { name: ".env", path: "/app/.env", is_dir: false, size: 120, mtime: 1000, mode: "0600" },
    { name: "README.md", path: "/app/README.md", is_dir: false, size: 2048, mtime: 3000, mode: "0644" },
    { name: "alpha.txt", path: "/app/alpha.txt", is_dir: false, size: 512, mtime: 2000, mode: "0644" },
    { name: "src", path: "/app/src", is_dir: true, size: 4096, mtime: 1500, mode: "0755" },
    { name: ".git", path: "/app/.git", is_dir: true, size: 4096, mtime: 1200, mode: "0755" },
  ];

  const defaultList = filterAndSortEntries(sample, {
    query: "",
    showHidden: false,
    sortBy: "name",
    sortAsc: true,
  });
  assert.deepEqual(
    defaultList.map((e) => e.name),
    ["src", "alpha.txt", "README.md"],
  );

  const withHiddenBySizeDesc = filterAndSortEntries(sample, {
    query: "",
    showHidden: true,
    sortBy: "size",
    sortAsc: false,
  });
  assert.deepEqual(
    withHiddenBySizeDesc.map((e) => e.name),
    [".git", "src", "README.md", "alpha.txt", ".env"],
  );

  const searched = filterAndSortEntries(sample, {
    query: "read",
    showHidden: false,
    sortBy: "name",
    sortAsc: true,
  });
  assert.deepEqual(
    searched.map((e) => e.name),
    ["README.md"],
  );
});

test("QUICK_TERMINAL_COMMANDS exposes non-empty mobile command shortcuts", () => {
  assert.ok(QUICK_TERMINAL_COMMANDS.length >= 6);
  for (const item of QUICK_TERMINAL_COMMANDS) {
    assert.ok(item.label.length > 0);
    assert.ok(item.command.length > 0);
  }
});

test("linkDestination resolves relative and absolute symlink targets without escaping root", () => {
  const relLink: FileEntry = {
    name: "current",
    path: "/var/www/app/current",
    type: "link",
    size: 0,
    mtime: 1000,
    mode: 0o777,
    perms: "lrwxrwxrwx",
    owner: "root",
    group: "root",
    target: "../releases/2026",
    target_type: "directory",
  };
  assert.equal(linkDestination(relLink), "/var/www/releases/2026");

  const absLink: FileEntry = {
    ...relLink,
    target: "/etc/nginx/sites-enabled",
  };
  assert.equal(linkDestination(absLink), "/etc/nginx/sites-enabled");

  const escapeLink: FileEntry = {
    ...relLink,
    path: "/var/link",
    target: "../../../../etc",
  };
  assert.equal(linkDestination(escapeLink), "/etc");
});

test("accessOf and buildMode follow Cockpit permission semantics and preserve special bits", () => {
  assert.equal(accessOf(0o750, 6), "rw");
  assert.equal(accessOf(0o750, 3), "r");
  assert.equal(accessOf(0o750, 0), "none");

  assert.equal(buildMode(0o644, "rw", "r", "r", false, false), 0o644);
  assert.equal(buildMode(0o644, "rw", "r", "r", false, true), 0o755);
  assert.equal(buildMode(0o755, "rw", "r", "none", true, false), 0o750);
  assert.equal(buildMode(0o2755, "rw", "rw", "r", true, false), 0o2775);
});

test("previewKind classifies image, video, audio, and pdf entries accurately", () => {
  const base: FileEntry = {
    name: "photo.webp",
    path: "/root/photo.webp",
    type: "file",
    size: 1024,
    mtime: 1000,
    mode: 0o644,
    perms: "-rw-r--r--",
    owner: "root",
    group: "root",
  };
  assert.equal(previewKind(base), "image");
  assert.equal(previewKind({ ...base, name: "clip.mp4" }), "video");
  assert.equal(previewKind({ ...base, name: "song.flac" }), "audio");
  assert.equal(previewKind({ ...base, name: "manual.pdf" }), "pdf");
  assert.equal(previewKind({ ...base, name: "nginx.conf" }), null);
  assert.equal(previewKind({ ...base, name: "photo.png", type: "directory" }), null);
});

test("isWithin and parseJsonc support RemoteSync path containment and .vscode/sftp.json", () => {
  assert.equal(isWithin("/var/www/app/src", "/var/www/app"), true);
  assert.equal(isWithin("/var/www/app", "/var/www/app"), true);
  assert.equal(isWithin("/var/www/app2", "/var/www/app"), false);

  const rawJsonc = `{
    // VS Code SFTP config
    "host": "10.0.0.10",
    "remotePath": "/var/www/app", /* target path */
    "ignore": [
      ".git",
      "node_modules",
    ],
  }`;
  const parsed = parseJsonc(rawJsonc) as { host: string; remotePath: string; ignore: string[] };
  assert.equal(parsed.host, "10.0.0.10");
  assert.equal(parsed.remotePath, "/var/www/app");
  assert.deepEqual(parsed.ignore, [".git", "node_modules"]);
});
