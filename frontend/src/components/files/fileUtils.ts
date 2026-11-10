import type { FileEntry, FileRemote } from "../../types";

export const joinPath = (dir: string, name: string): string =>
  dir === "/" ? `/${name}` : `${dir}/${name}`;

export const parentOf = (path: string): string => {
  if (path === "/") return "/";
  const parent = path.slice(0, path.lastIndexOf("/"));
  return parent || "/";
};

export const isDirLike = (e: FileEntry): boolean =>
  e.type === "directory" || (e.type === "link" && e.target_type === "directory");

/** Resolves where a symlink to a directory should navigate to. */
export const linkDestination = (e: FileEntry): string => {
  if (e.type !== "link" || !e.target) return e.path;
  if (e.target.startsWith("/")) return e.target;
  const parts = [...parentOf(e.path).split("/"), ...e.target.split("/")];
  const out: string[] = [];
  for (const p of parts) {
    if (!p || p === ".") continue;
    if (p === "..") out.pop();
    else out.push(p);
  }
  return "/" + out.join("/");
};

export const formatSize = (bytes: number): string => {
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let v = bytes / 1024;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v < 10 ? v.toFixed(1) : Math.round(v)} ${units[i]}`;
};

export const formatDate = (epoch: number): string =>
  new Date(epoch * 1000).toLocaleString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });

const ARCHIVE = /\.(zip|tar|gz|tgz|bz2|xz|zst|7z|rar|rpm|deb|iso)$/i;
const IMAGE = /\.(png|jpe?g|gif|svg|webp|ico|bmp|avif)$/i;
const VIDEO = /\.(mp4|m4v|webm|ogv|mov|mkv)$/i;
const AUDIO = /\.(mp3|m4a|ogg|oga|opus|wav|flac)$/i;
const PDF = /\.pdf$/i;

export type PreviewKind = "image" | "video" | "audio" | "pdf";

/** What the media viewer can show for this entry, or null (then it opens in the text editor). */
export const previewKind = (e: FileEntry): PreviewKind | null => {
  if (!(e.type === "file" || (e.type === "link" && e.target_type === "file"))) return null;
  if (IMAGE.test(e.name)) return "image";
  if (VIDEO.test(e.name)) return "video";
  if (AUDIO.test(e.name)) return "audio";
  if (PDF.test(e.name)) return "pdf";
  return null;
};

/** Files that get a thumbnail instead of their type icon. */
export const thumbKind = (e: FileEntry): "image" | "video" | null => {
  const kind = previewKind(e);
  return kind === "image" || kind === "video" ? kind : null;
};

const CODE =
  /\.(py|js|ts|tsx|jsx|sh|go|rs|c|h|cpp|java|php|rb|json|ya?ml|toml|ini|conf|cfg|xml|html|css|sql|container|network|volume|service|timer|socket)$/i;
const TEXT = /\.(txt|md|log|csv|env)$/i;

export type FileVisualKind =
  | "folder"
  | "folder-symlink"
  | "link"
  | "link-broken"
  | "device"
  | "archive"
  | "image"
  | "video"
  | "audio"
  | "pdf"
  | "code"
  | "text"
  | "file";

export const fileVisual = (e: FileEntry): { kind: FileVisualKind; color: string } => {
  if (e.type === "directory") return { kind: "folder", color: "#58a6ff" };
  if (e.type === "link") {
    if (e.target_type === "broken") return { kind: "link-broken", color: "#f85149" };
    if (e.target_type === "directory") return { kind: "folder-symlink", color: "#58a6ff" };
    return { kind: "link", color: "#a371f7" };
  }
  if (e.type !== "file") return { kind: "device", color: "#8b949e" };
  if (ARCHIVE.test(e.name)) return { kind: "archive", color: "#d29922" };
  if (IMAGE.test(e.name)) return { kind: "image", color: "#3fb950" };
  if (VIDEO.test(e.name)) return { kind: "video", color: "#f0883e" };
  if (AUDIO.test(e.name)) return { kind: "audio", color: "#39c5cf" };
  if (PDF.test(e.name)) return { kind: "pdf", color: "#f85149" };
  if (CODE.test(e.name)) return { kind: "code", color: "#db61a2" };
  if (TEXT.test(e.name)) return { kind: "text", color: "#c9d1d9" };
  return { kind: "file", color: "#8b949e" };
};

export const TYPE_LABEL: Record<string, string> = {
  directory: "Directory",
  file: "File",
  link: "Symbolic link",
  device: "Device",
  fifo: "Named pipe",
  socket: "Socket",
  other: "Special file",
};

export type Access = "rw" | "r" | "none";

export const accessOf = (mode: number, shift: number): Access => {
  const bits = (mode >> shift) & 7;
  if (bits & 2) return "rw";
  if (bits & 4) return "r";
  return "none";
};

/** Cockpit-style mode builder: directories get +x with read, files get +x only if "executable". */
export const buildMode = (
  current: number,
  owner: Access,
  group: Access,
  other: Access,
  isDir: boolean,
  executable: boolean,
): number => {
  const bits = (a: Access) => {
    let v = a === "rw" ? 6 : a === "r" ? 4 : 0;
    if (v && (isDir || executable)) v |= 1;
    return v;
  };
  const special = current & 0o7000;
  return special | (bits(owner) << 6) | (bits(group) << 3) | bits(other);
};

export const DEFAULT_IGNORE = [".git", ".vscode", ".idea"];

/** True when `path` is `root` or inside it */
export const isWithin = (path: string, root: string): boolean =>
  path === root || path.startsWith(root === "/" ? "/" : `${root}/`);

/** Remotes whose local folder (on this server) contains every one of `paths` */
export const remotesFor = (remotes: FileRemote[], serverId: number, paths: string[]): FileRemote[] =>
  paths.length
    ? remotes.filter((r) => r.local_server_id === serverId && paths.every((p) => isWithin(p, r.local_path)))
    : [];

/** JSON with // and /* comments and trailing commas, as VS Code writes sftp.json */
export const parseJsonc = (text: string): unknown => {
  let out = "";
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if (c === '"') {
      const start = i;
      for (i++; i < text.length && text[i] !== '"'; i++) if (text[i] === "\\") i++;
      out += text.slice(start, i + 1);
    } else if (c === "/" && text[i + 1] === "/") {
      while (i < text.length && text[i] !== "\n") i++;
      out += "\n";
    } else if (c === "/" && text[i + 1] === "*") {
      i = text.indexOf("*/", i + 2);
      if (i < 0) break;
      i++;
    } else out += c;
  }
  return JSON.parse(out.replace(/,(\s*[}\]])/g, "$1"));
};

export interface SftpConfig {
  ignore: string[];
  remotePath?: string;
  host?: string;
}

/** The parts of a VS Code SFTP extension config we can use (first profile when it holds several) */
export const readSftpConfig = (raw: unknown): SftpConfig | null => {
  const candidate = Array.isArray(raw) ? (raw[0] as unknown) : raw;
  if (!candidate || typeof candidate !== "object") return null;
  const cfg = candidate as Record<string, unknown>;
  return {
    ignore: Array.isArray(cfg.ignore)
      ? cfg.ignore.filter((p: unknown): p is string => typeof p === "string")
      : [],
    remotePath: typeof cfg.remotePath === "string" ? cfg.remotePath : undefined,
    host: typeof cfg.host === "string" ? cfg.host : undefined,
  };
};
