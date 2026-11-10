import assert from "node:assert/strict";
import { test } from "node:test";

import { buildCdCommand, quoteRemotePath } from "../src/shell.ts";

// Same cases as backend/tests/test_ssh.py::test_build_cd_command_quotes, so both sides type identical commands.
test("quotes paths like the backend", () => {
  assert.equal(buildCdCommand("/var/www/my app", ""), "cd -- '/var/www/my app' && clear");
  assert.equal(buildCdCommand("/srv/it's", ""), "cd -- '/srv/it'\"'\"'s' && clear");
  assert.equal(buildCdCommand("~", ""), "cd -- ~ && clear");
  assert.equal(buildCdCommand("~/proj a", "git status"), "cd -- ~/'proj a' && clear && git status");
});

test("leaves safe paths unquoted like shlex.quote", () => {
  assert.equal(quoteRemotePath("/var/www/app-1.2_x"), "/var/www/app-1.2_x");
  assert.equal(quoteRemotePath(""), "~");
  assert.equal(quoteRemotePath("$HOME/x"), "'$HOME/x'");
});
