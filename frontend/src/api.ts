import type {
  AuthStatus,
  BackupStats,
  Bookmark,
  BookmarkInput,
  CFAccount,
  CFTunnel,
  CFZone,
  CFRoute,
  CFRouteIn,
  ConnectionCheck,
  ContainerActionBody,
  ContainerEngineInfo,
  ContainerItem,
  ContainerLogsOut,
  ContainerRunBody,
  DirectTransferScanIn,
  DirectTransferStartIn,
  DirListing,
  FileActionBody,
  FileActionOut,
  FileContentOut,
  FileEntry,
  FileListOut,
  FilePrincipalsOut,
  FileReadOut,
  FileRemote,
  FileRemoteInput,
  ImageItem,
  PastedImage,
  Server,
  ServerInput,
  ServerStats,
  Snippet,
  SnippetInput,
  TransferDirection,
  TransferJob,
  TransferScan,
  UpdateApplyResult,
  UpdateCheckResult,
  UpdateProgressResult,
  CFSyncNowIn,
  CFSyncNowOut,
  CFSyncPullIn,
  CFSyncPullOut,
  CFSyncPushIn,
  CFSyncPushOut,
  CFSyncStatusOut,
} from "./types";

/** Session token: embedded in the page by the backend; `?token=` is the Vite dev-server fallback. */
export const token =
  (typeof document !== "undefined"
    ? document.querySelector<HTMLMetaElement>('meta[name="rusterm-token"], meta[name="termdeck-token"]')?.content
    : null) ??
  (typeof window !== "undefined"
    ? new URLSearchParams(window.location.search).get("token")
    : null) ??
  "";

const SESSION_KEY = "rusterm.session";
const LEGACY_SESSION_KEY = "termdeck.session";

function readSession(): string {
  try {
    if (typeof sessionStorage !== "undefined") {
      return sessionStorage.getItem(SESSION_KEY) ?? sessionStorage.getItem(LEGACY_SESSION_KEY) ?? "";
    }
  } catch {
    return "";
  }
  return "";
}

/** PIN session: kept per browser tab so a reload stays unlocked but a new tab asks for the PIN. */
let session = readSession();

export function getSession(): string {
  return session;
}

export function setSession(value: string): void {
  session = value;
  try {
    if (value) sessionStorage.setItem(SESSION_KEY, value);
    else sessionStorage.removeItem(SESSION_KEY);
  } catch {
    // Storage unavailable: the session still lives in memory for this page.
  }
}

let unauthorizedHandler: (() => void) | null = null;

/** Called when the backend rejects the session (locked elsewhere or restarted). */
export function setUnauthorizedHandler(handler: (() => void) | null): void {
  unauthorizedHandler = handler;
}

export class ApiError extends Error {
  readonly status: number;

  constructor(message: string, status: number) {
    super(message);
    this.status = status;
  }
}

interface ValidationIssue {
  loc?: (string | number)[];
  msg?: string;
}

function describeDetail(detail: unknown, fallback: string): string {
  if (typeof detail === "string") return detail;
  if (Array.isArray(detail)) {
    return detail
      .map((item: ValidationIssue) => {
        const field = item.loc?.[item.loc.length - 1];
        return field ? `${field}: ${item.msg ?? ""}` : (item.msg ?? "");
      })
      .join("; ");
  }
  return fallback;
}

/** JSON is the default body; a Blob (e.g. a pasted image) is sent as-is. */
async function request<T>(method: string, path: string, body?: unknown): Promise<T> {
  const raw = body instanceof Blob ? body : null;
  const res = await fetch(path, {
    method,
    headers: {
      "X-Token": token,
      "X-Session": session,
      ...(raw ? { "Content-Type": raw.type || "application/octet-stream" } : {}),
      ...(body !== undefined && !raw ? { "Content-Type": "application/json" } : {}),
    },
    body: raw ?? (body !== undefined ? JSON.stringify(body) : undefined),
  });
  if (!res.ok) {
    let detail: unknown = null;
    try {
      detail = ((await res.json()) as { detail?: unknown }).detail;
    } catch {
      detail = null;
    }
    if (res.status === 401 && path !== "/api/auth/login" && path !== "/api/auth/setup-pin") {
      setSession("");
      unauthorizedHandler?.();
    }
    throw new ApiError(describeDetail(detail, `Permintaan gagal (${res.status})`), res.status);
  }
  if (res.status === 204) return undefined as T;
  return (await res.json()) as T;
}

export const api = {
  authStatus: () => request<AuthStatus>("GET", "/api/auth/status"),
  setupPin: async (pin: string) => {
    const result = await request<{ session: string; must_change_pin?: boolean }>("POST", "/api/auth/setup-pin", { pin });
    setSession(result.session);
    return result;
  },
  login: async (pin: string) => {
    const result = await request<{ session: string; must_change_pin?: boolean }>("POST", "/api/auth/login", { pin });
    setSession(result.session);
    return result;
  },
  lock: async () => {
    try {
      await request<void>("POST", "/api/auth/lock");
    } finally {
      setSession("");
    }
  },
  changePin: (oldPin: string, newPin: string) =>
    request<void>("POST", "/api/auth/change-pin", { old_pin: oldPin, new_pin: newPin }),

  listServers: () => request<Server[]>("GET", "/api/servers"),
  createServer: (data: ServerInput) => request<Server>("POST", "/api/servers", data),
  updateServer: (id: number, data: ServerInput) => request<Server>("PUT", `/api/servers/${id}`, data),
  deleteServer: (id: number) => request<void>("DELETE", `/api/servers/${id}`),
  reorderServers: (ids: number[]) => request<Server[]>("PUT", "/api/servers/order", { ids }),
  resetHostKey: (id: number) => request<Server>("POST", `/api/servers/${id}/reset-hostkey`),
  testServer: (data: ServerInput & { id: number | null }) =>
    request<ConnectionCheck>("POST", "/api/servers/test", data),
  serverStats: (id: number) => request<ServerStats>("GET", `/api/servers/${id}/stats`),
  listDir: (id: number, path: string) =>
    request<DirListing>("GET", `/api/servers/${id}/ls?path=${encodeURIComponent(path)}`),
  readFile: (id: number, path: string) =>
    request<FileContentOut>("GET", `/api/servers/${id}/files/read?path=${encodeURIComponent(path)}`),
  writeFile: (id: number, path: string, content: string) =>
    request<FileActionOut>("PUT", `/api/servers/${id}/files/write`, { path, content }),
  mkdir: (id: number, path: string) =>
    request<FileActionOut>("POST", `/api/servers/${id}/files/mkdir`, { path }),
  renameFile: (id: number, oldPath: string, newPath: string) =>
    request<FileActionOut>("POST", `/api/servers/${id}/files/rename`, { old_path: oldPath, new_path: newPath }),
  chmodFile: (id: number, path: string, mode: string) =>
    request<FileActionOut>("POST", `/api/servers/${id}/files/chmod`, { path, mode }),
  deleteFile: (id: number, path: string) =>
    request<void>("DELETE", `/api/servers/${id}/files?path=${encodeURIComponent(path)}`),
  uploadFile: (id: number, path: string, file: Blob) =>
    request<FileActionOut>("POST", `/api/servers/${id}/files/upload?path=${encodeURIComponent(path)}`, file),
  pasteImage: (id: number, image: Blob) => request<PastedImage>("POST", `/api/servers/${id}/paste-image`, image),

  // Cockpit-py Files API
  filesList: (serverId: number, path: string, showHidden: boolean) =>
    request<FileListOut>(
      "GET",
      `/api/files/${serverId}/list?path=${encodeURIComponent(path)}&show_hidden=${showHidden ? "true" : "false"}`,
    ),
  filesPrincipals: (serverId: number) =>
    request<FilePrincipalsOut>("GET", `/api/files/${serverId}/principals`),
  filesRead: (serverId: number, path: string) =>
    request<FileReadOut>("GET", `/api/files/${serverId}/read?path=${encodeURIComponent(path)}`),
  filesAction: <T = FileEntry>(serverId: number, body: FileActionBody) =>
    request<T>("POST", `/api/files/${serverId}/action`, body),
  filesUpload: (
    serverId: number,
    dir: string,
    file: File,
    overwrite: boolean,
    onProgress?: (fraction: number) => void,
  ): Promise<FileEntry> =>
    new Promise<FileEntry>((resolve, reject) => {
      const xhr = new XMLHttpRequest();
      const params = new URLSearchParams({
        dir,
        name: file.name,
        overwrite: overwrite ? "true" : "false",
      });
      xhr.open("POST", `/api/files/${serverId}/upload?${params.toString()}`);
      xhr.setRequestHeader("X-Token", token);
      xhr.setRequestHeader("X-Session", session);
      xhr.setRequestHeader("Content-Type", "application/octet-stream");
      if (onProgress) {
        xhr.upload.onprogress = (ev) => {
          if (ev.lengthComputable && ev.total > 0) {
            onProgress(ev.loaded / ev.total);
          }
        };
      }
      xhr.onload = () => {
        if (xhr.status >= 200 && xhr.status < 300) {
          try {
            resolve(JSON.parse(xhr.responseText) as FileEntry);
          } catch {
            reject(new ApiError("Respons upload tidak valid", xhr.status));
          }
        } else {
          if (xhr.status === 401) {
            setSession("");
            unauthorizedHandler?.();
          }
          let detail: unknown = null;
          try {
            detail = (JSON.parse(xhr.responseText) as { detail?: unknown }).detail;
          } catch {
            detail = null;
          }
          reject(new ApiError(describeDetail(detail, `Upload gagal (${xhr.status})`), xhr.status));
        }
      };
      xhr.onerror = () => reject(new ApiError("Koneksi jaringan gagal saat upload", 0));
      xhr.send(file);
    }),
  filesDownloadUrl: (serverId: number, path: string): string => {
    const params = new URLSearchParams({ path, token, session });
    return `/api/files/${serverId}/download?${params.toString()}`;
  },
  filesViewUrl: (serverId: number, path: string): string => {
    const params = new URLSearchParams({ path, token, session });
    return `/api/files/${serverId}/view?${params.toString()}`;
  },
  filesThumbUrl: (serverId: number, path: string, version: string): string => {
    const params = new URLSearchParams({ path, v: version, token, session });
    return `/api/files/${serverId}/thumb?${params.toString()}`;
  },
  filesRemotes: () => request<FileRemote[]>("GET", "/api/files/remotes"),
  createFileRemote: (input: FileRemoteInput) => request<FileRemote>("POST", "/api/files/remotes", input),
  updateFileRemote: (id: number, input: FileRemoteInput) =>
    request<FileRemote>("PUT", `/api/files/remotes/${id}`, input),
  deleteFileRemote: (id: number) => request<void>("DELETE", `/api/files/remotes/${id}`),
  scanTransfer: (id: number, direction: TransferDirection, paths: string[]) =>
    request<TransferScan>("POST", `/api/files/remotes/${id}/scan`, { direction, paths }),
  startTransfer: (id: number, direction: TransferDirection, paths: string[]) =>
    request<TransferJob>("POST", `/api/files/remotes/${id}/transfer`, { direction, paths }),
  getTransfer: (jobId: string) => request<TransferJob>("GET", `/api/files/transfers/${jobId}`),
  cancelTransfer: (jobId: string) => request<TransferJob>("POST", `/api/files/transfers/${jobId}/cancel`),
  directTransferScan: (body: DirectTransferScanIn) =>
    request<TransferScan>("POST", "/api/files/direct-transfer/scan", body),
  directTransferStart: (body: DirectTransferStartIn) =>
    request<TransferJob>("POST", "/api/files/direct-transfer/start", body),
  getFileBookmarks: (serverId: number) =>
    request<string[]>("GET", `/api/files/bookmarks?server_id=${serverId}`),
  toggleFileBookmark: (serverId: number, path: string) =>
    request<{ bookmarked: boolean; bookmarks: string[] }>("POST", "/api/files/bookmarks/toggle", {
      server_id: serverId,
      path,
    }),
  syncFileBookmarks: (serverId: number, paths: string[]) =>
    request<string[]>("POST", "/api/files/bookmarks/sync", { server_id: serverId, paths }),

  listBookmarks: () => request<Bookmark[]>("GET", "/api/bookmarks"),
  createBookmark: (data: BookmarkInput) => request<Bookmark>("POST", "/api/bookmarks", data),
  updateBookmark: (id: number, data: BookmarkInput) => request<Bookmark>("PUT", `/api/bookmarks/${id}`, data),
  deleteBookmark: (id: number) => request<void>("DELETE", `/api/bookmarks/${id}`),

  listSnippets: () => request<Snippet[]>("GET", "/api/snippets"),
  createSnippet: (data: SnippetInput) => request<Snippet>("POST", "/api/snippets", data),
  updateSnippet: (id: number, data: SnippetInput) => request<Snippet>("PUT", `/api/snippets/${id}`, data),
  deleteSnippet: (id: number) => request<void>("DELETE", `/api/snippets/${id}`),

  exportConfig: (pin: string) => request<object>("POST", "/api/config/export", { pin }),
  importConfig: (pin: string, backup: object) =>
    request<BackupStats>("POST", "/api/config/import", { pin, backup }),

  // Containers API (Cockpit RHEL style)
  containersEngine: (serverId: number) =>
    request<ContainerEngineInfo>("GET", `/api/containers/engine?server_id=${serverId}`),
  containersList: (serverId: number, all = true) =>
    request<ContainerItem[]>("GET", `/api/containers/list?server_id=${serverId}&all=${all ? "true" : "false"}`),
  containersAction: (body: ContainerActionBody) =>
    request<{ ok: boolean; message: string }>("POST", "/api/containers/action", body),
  containersLogs: (serverId: number, containerId: string, tail = 150) =>
    request<ContainerLogsOut>("GET", `/api/containers/logs?server_id=${serverId}&container_id=${encodeURIComponent(containerId)}&tail=${tail}`),
  containersImages: (serverId: number) =>
    request<ImageItem[]>("GET", `/api/containers/images?server_id=${serverId}`),
  containersPullImage: (serverId: number, image: string) =>
    request<{ ok: boolean; message: string }>("POST", "/api/containers/images/pull", { server_id: serverId, image }),
  containersDeleteImage: (serverId: number, imageId: string, force = false) =>
    request<{ ok: boolean; message: string }>(
      "DELETE",
      `/api/containers/images?server_id=${serverId}&image_id=${encodeURIComponent(imageId)}&force=${force ? "true" : "false"}`
    ),
  containersRun: (body: ContainerRunBody) =>
    request<{ ok: boolean; id?: string; message: string }>("POST", "/api/containers/run", body),

  // Cloudflare Tunnels
  cfListAccounts: () => request<CFAccount[]>("GET", "/api/cf/accounts"),
  cfVerifyToken: (cfToken: string) =>
    request<{ account_id: string; account_name: string; has_dns_access?: boolean }>("POST", "/api/cf/accounts/verify", { token: cfToken }),
  cfCreateAccount: (name: string, cfToken: string) =>
    request<CFAccount>("POST", "/api/cf/accounts", { name, token: cfToken }),
  cfUpdateAccount: (accountId: number, body: { name: string; token?: string }) =>
    request<CFAccount>("PUT", `/api/cf/accounts/${accountId}`, body),
  cfDeleteAccount: (accountId: number) => request<{ ok: boolean }>("DELETE", `/api/cf/accounts/${accountId}`),
  cfListTunnels: (accountId: number) => request<CFTunnel[]>("GET", `/api/cf/accounts/${accountId}/tunnels`),
  cfListZones: (accountId: number) => request<CFZone[]>("GET", `/api/cf/accounts/${accountId}/zones`),
  cfListRoutes: (accountId: number, tunnelId: string) =>
    request<CFRoute[]>("GET", `/api/cf/accounts/${accountId}/tunnels/${tunnelId}/routes`),
  cfCreateRoute: (accountId: number, tunnelId: string, body: CFRouteIn) =>
    request<{
      ok: boolean;
      hostname: string;
      service: string;
      dns_synced?: boolean;
      dns_warning?: string | null;
      cname_target?: string;
    }>("POST", `/api/cf/accounts/${accountId}/tunnels/${tunnelId}/routes`, body),
  cfDeleteRoute: (accountId: number, tunnelId: string, hostname: string, zoneId?: string) => {
    const params = new URLSearchParams({ hostname });
    if (zoneId) params.set("zone_id", zoneId);
    return request<{ ok: boolean; deleted: string }>(
      "DELETE",
      `/api/cf/accounts/${accountId}/tunnels/${tunnelId}/routes?${params.toString()}`
    );
  },
  cfGetTunnelToken: (accountId: number, tunnelId: string) =>
    request<{ token: string }>("GET", `/api/cf/accounts/${accountId}/tunnels/${tunnelId}/token`),
  cfGetSyncStatus: (accountId?: number) =>
    request<CFSyncStatusOut>("GET", `/api/cf/sync/status${accountId ? `?account_id=${accountId}` : ""}`),
  cfSyncNow: (body: CFSyncNowIn) => request<CFSyncNowOut>("POST", "/api/cf/sync/sync-now", body),
  cfSyncPush: (body: CFSyncPushIn) => request<CFSyncPushOut>("POST", "/api/cf/sync/push", body),
  cfSyncPull: (body: CFSyncPullIn) => request<CFSyncPullOut>("POST", "/api/cf/sync/pull", body),
  checkUpdate: () => request<UpdateCheckResult>("GET", "/api/system/update-check"),
  applyUpdate: () => request<UpdateApplyResult>("POST", "/api/system/update-apply"),
  getUpdateProgress: () => request<UpdateProgressResult>("GET", "/api/system/update-progress"),
};

export function terminalSocketUrl(serverId: number, bookmarkId: number | null, cols: number, rows: number): string {
  const proto = window.location.protocol === "https:" ? "wss" : "ws";
  const params = new URLSearchParams({
    session,
    server_id: String(serverId),
    cols: String(cols),
    rows: String(rows),
  });
  if (bookmarkId !== null) params.set("bookmark_id", String(bookmarkId));
  return `${proto}://${window.location.host}/ws/terminal?${params.toString()}`;
}

export function downloadFileUrl(serverId: number, path: string): string {
  const params = new URLSearchParams({
    path,
    token,
    session,
  });
  return `/api/files/${serverId}/download?${params.toString()}`;
}

export function openExternal(url: string): void {
  window.open(url, "_blank", "noopener");
}

export function errorMessage(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}
