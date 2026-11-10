export type AuthType = "password" | "key";

export interface Server {
  id: number;
  name: string;
  host: string;
  port: number;
  username: string;
  auth_type: AuthType;
  group_name: string;
  color: string;
  has_password: boolean;
  has_private_key: boolean;
  host_key_fingerprint: string | null;
}

export interface ServerInput {
  name: string;
  host: string;
  port: number;
  username: string;
  auth_type: AuthType;
  /** null keeps the stored secret on update */
  password: string | null;
  private_key: string | null;
  passphrase: string | null;
  group_name: string;
  color: string;
}

export interface Bookmark {
  id: number;
  server_id: number;
  name: string;
  path: string;
  startup_cmd: string;
}

export type BookmarkInput = Omit<Bookmark, "id">;

export interface Snippet {
  id: number;
  name: string;
  command: string;
}

export type SnippetInput = Omit<Snippet, "id">;

export interface BackupStats {
  servers_added: number;
  servers_updated: number;
  bookmarks_added: number;
  snippets_added: number;
}

export interface DirEntry {
  name: string;
  path: string;
  is_dir: boolean;
  size?: number;
  mtime?: number | null;
  mode?: string;
  is_symlink?: boolean;
}

export interface DirListing {
  path: string;
  entries: DirEntry[];
}

export interface FileContentOut {
  path: string;
  content: string;
  size: number;
  mtime: number | null;
  mode: string;
}

export interface FileActionOut {
  path: string;
  message: string;
}

export type FileType = "directory" | "file" | "link" | "device" | "fifo" | "socket" | "other";

export interface FileEntry {
  name: string;
  path: string;
  type: FileType;
  size: number;
  mtime: number;
  mode: number;
  perms: string;
  owner: string;
  group: string;
  target?: string;
  target_type?: FileType | "broken";
}

export interface FileListOut {
  path: string;
  entries: FileEntry[];
  self: FileEntry;
}

export interface FilePrincipalsOut {
  users: string[];
  groups: string[];
  home: string;
}

export interface FileReadOut {
  path: string;
  content: string;
  mtime: number;
  size: number;
}

export interface FileActionBody {
  op: "write" | "mkdir" | "symlink" | "rename" | "paste" | "delete" | "chmod";
  path?: string;
  paths?: string[];
  new_path?: string;
  dest_dir?: string;
  mode?: number | "copy" | "move";
  content?: string;
  create?: boolean;
  expected_mtime?: number;
  target?: string;
  owner?: string;
  group?: string;
  recursive?: boolean;
}

export interface FileRemote {
  id: number;
  name: string;
  local_server_id: number;
  local_path: string;
  remote_server_id: number;
  remote_path: string;
  ignore: string[];
  updated_at?: string;
}

export type FileRemoteInput = Omit<FileRemote, "id" | "updated_at">;
export type TransferDirection = "upload" | "download";

export interface TransferScan {
  files: number;
  dirs: number;
  links: number;
  bytes: number;
  ignored: number;
  unsupported: number;
  skipped: string[];
  tar_bytes: number;
  source_server_id: number;
  source_root: string;
  dest_server_id: number;
  dest_root: string;
  rels: string[];
  has_gitignore?: boolean;
  use_gitignore?: boolean;
}

export interface TransferJob {
  id: string;
  remote_id?: number | null;
  remote_name: string;
  direction: TransferDirection | string;
  state: "running" | "done" | "error" | "cancelled";
  source_server_id: number;
  source_root: string;
  dest_server_id: number;
  dest_root: string;
  rels: string[];
  read_gitignore?: boolean;
  sent_bytes: number;
  total_bytes: number;
  total_files: number;
  result: { files: number; dirs: number; links: number; bytes: number } | null;
  error: string | null;
}

export interface DirectTransferScanIn {
  source_server_id: number;
  dest_server_id: number;
  source_dir: string;
  dest_dir: string;
  paths: string[];
  ignore?: string[];
  use_gitignore?: boolean;
}

export interface DirectTransferStartIn {
  source_server_id: number;
  dest_server_id: number;
  source_dir: string;
  dest_dir: string;
  paths: string[];
  mode?: "copy" | "move";
  ignore?: string[];
  use_gitignore?: boolean;
}

export interface PastedImage {
  path: string;
}

export interface ServerStats {
  hostname: string;
  os: string;
  kernel: string;
  uptime_seconds: number | null;
  load: number[];
  cpu_count: number | null;
  mem_total: number | null;
  mem_used: number | null;
  disk_total: number | null;
  disk_used: number | null;
}

export interface ConnectionCheck {
  ok: boolean;
  message: string;
}

export type TabStatus = "connecting" | "connected" | "closed" | "error";

export interface Tab {
  id: string;
  serverId: number;
  bookmarkId: number | null;
  title: string;
  status: TabStatus;
  /** bumped to force a fresh SSH session in the same tab */
  session: number;
}

/** Control frames sent by the backend as text over the terminal WebSocket. */
export type ControlMessage =
  | { type: "status"; status: "connecting" | "connected" }
  | { type: "error"; message: string }
  | { type: "exit"; code: number | null };

export interface AuthStatus {
  unlocked: boolean;
  session_valid: boolean;
  initialized: boolean;
  must_change_pin?: boolean;
}

export type ContainerEngineType = "podman" | "docker" | "none";

export interface ContainerEngineInfo {
  engine: ContainerEngineType;
  version: string;
  rootless: boolean;
  error?: string | null;
}

export type ContainerState =
  | "running"
  | "exited"
  | "paused"
  | "created"
  | "restarting"
  | "dead"
  | "unknown";

export interface ContainerPort {
  host_ip?: string;
  host_port?: number;
  container_port: number;
  protocol?: string;
}

export interface ContainerItem {
  id: string;
  names: string[];
  image: string;
  state: ContainerState;
  status: string;
  created: number | string;
  ports: ContainerPort[];
  command?: string;
}

export interface ContainerActionBody {
  server_id: number;
  container_id: string;
  action: "start" | "stop" | "restart" | "pause" | "unpause" | "delete";
  force?: boolean;
}

export interface ContainerLogsOut {
  logs: string;
}

export interface ImageItem {
  id: string;
  repository: string;
  tag: string;
  size: string;
  created: string;
}

export interface ContainerRunBody {
  server_id: number;
  image: string;
  name?: string;
  ports?: string[];
  env?: string[];
  volumes?: string[];
  command?: string;
  restart?: string;
}

export interface CFAccount {
  id: number;
  name: string;
  token_masked: string;
  account_id: string;
  account_name: string;
  created_at: string;
}

export interface CFTunnel {
  id: string;
  name: string;
  status: "healthy" | "inactive" | "down" | string;
  conns_count: number;
  created_at: string;
  remote_config: boolean;
}

export interface CFZone {
  id: string;
  name: string;
  status: string;
}

export interface CFRoute {
  hostname: string;
  service: string;
}

export interface CFRouteIn {
  hostname: string;
  service: string;
  zone_id?: string;
}

export interface UpdateCheckResult {
  platform: "desktop" | "android" | string;
  current_version: string;
  latest_version: string;
  has_update: boolean;
  release_title: string | null;
  release_notes: string | null;
  download_url: string | null;
}

export interface UpdateApplyResult {
  started: boolean;
  message: string;
}

export interface UpdateProgressResult {
  state: "idle" | "downloading" | "ready" | "completed" | "error" | string;
  progress_percent: number;
  downloaded_bytes: number;
  total_bytes: number;
  message: string;
}

export interface CFSyncMetadata {
  device_name: string;
  updated_at: number;
  servers_count: number;
  snippets_count: number;
  version: number;
}

export interface CFSyncStatusOut {
  configured: boolean;
  has_remote: boolean;
  remote_metadata?: CFSyncMetadata | null;
  local_counts: {
    servers_count: number;
    snippets_count: number;
  };
  message?: string | null;
}

export interface CFSyncNowIn {
  account_id?: number | null;
  pin: string;
  device_name?: string;
}

export interface CFSyncNowOut {
  success: boolean;
  synced_at: number;
  stats: BackupStats;
  remote_metadata: CFSyncMetadata;
}

export interface CFSyncPushIn {
  account_id?: number | null;
  pin: string;
  device_name?: string;
}

export interface CFSyncPushOut {
  success: boolean;
  synced_at: number;
  remote_metadata: CFSyncMetadata;
}

export interface CFSyncPullIn {
  account_id?: number | null;
  pin: string;
  replace_local?: boolean;
}

export interface CFSyncPullOut {
  success: boolean;
  stats: BackupStats;
  remote_metadata?: CFSyncMetadata | null;
}


declare global {
  interface Window {
    TermDeckApp?: {
      saveBlob?: (base64Data: string, fileName: string, mimeType: string) => void;
    };
  }
}

