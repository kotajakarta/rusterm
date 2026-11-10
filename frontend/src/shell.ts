// Mirrors backend/app/ssh.py (quote_remote_path / build_cd_command) so jumping
// between paths in an open terminal types exactly what a new session would.

const UNSAFE = /[^\w@%+=:,./-]/;

/** POSIX shell quoting, equivalent to Python's shlex.quote. */
function shellQuote(value: string): string {
  if (value === "") return "''";
  if (!UNSAFE.test(value)) return value;
  return `'${value.replace(/'/g, `'"'"'`)}'`;
}

/** Quote a remote path while keeping a leading ~ expandable by the shell. */
export function quoteRemotePath(path: string): string {
  if (path === "" || path === "~") return "~";
  if (path.startsWith("~/")) return `~/${shellQuote(path.slice(2))}`;
  return shellQuote(path);
}

export function buildCdCommand(path: string, startup: string): string {
  let command = `cd -- ${quoteRemotePath(path)} && clear`;
  if (startup.trim()) command += ` && ${startup.trim()}`;
  return command;
}
