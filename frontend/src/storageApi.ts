import { ApiError, getSession, token } from "./api";
import type {
  StorageAccount,
  StorageAccountInput,
  StorageAccountUpdateInput,
  StorageAccountVerifyInput,
  StorageBucket,
  StorageDeleteOut,
  StorageListOut,
  StoragePresignOut,
  StorageTransferCreateInput,
  StorageTransferJob,
  StorageVerifyOut,
} from "./types";

function buildHeaders(extra?: Record<string, string>): HeadersInit {
  const headers: Record<string, string> = { ...(extra ?? {}) };
  if (token) headers["X-RusTerm-Token"] = token;
  const session = getSession();
  if (session) headers["X-Session"] = session;
  return headers;
}

async function parseError(res: Response): Promise<string> {
  const text = await res.text().catch(() => "");
  if (!text) return `HTTP ${res.status}`;
  try {
    const json = JSON.parse(text) as { detail?: string; error?: string; message?: string };
    return json.detail ?? json.error ?? json.message ?? text;
  } catch {
    return text;
  }
}

async function reqJson<T>(url: string, init?: RequestInit): Promise<T> {
  const headers = buildHeaders(
    init?.body && !(init.body instanceof FormData)
      ? { "Content-Type": "application/json" }
      : undefined,
  );
  const res = await fetch(url, { ...init, headers });
  if (!res.ok) {
    throw new ApiError(await parseError(res), res.status);
  }
  return (await res.json()) as T;
}

export const storageApi = {
  listAccounts(): Promise<StorageAccount[]> {
    return reqJson<StorageAccount[]>("/api/storage/accounts");
  },

  createAccount(input: StorageAccountInput): Promise<StorageAccount> {
    return reqJson<StorageAccount>("/api/storage/accounts", {
      method: "POST",
      body: JSON.stringify(input),
    });
  },

  updateAccount(id: number, input: StorageAccountUpdateInput): Promise<StorageAccount> {
    return reqJson<StorageAccount>(`/api/storage/accounts/${id}`, {
      method: "PUT",
      body: JSON.stringify(input),
    });
  },

  deleteAccount(id: number): Promise<{ ok: boolean }> {
    return reqJson<{ ok: boolean }>(`/api/storage/accounts/${id}`, {
      method: "DELETE",
    });
  },

  verifyAccount(input: StorageAccountVerifyInput): Promise<StorageVerifyOut> {
    return reqJson<StorageVerifyOut>("/api/storage/accounts/verify", {
      method: "POST",
      body: JSON.stringify(input),
    });
  },

  listBuckets(accountId: number): Promise<StorageBucket[]> {
    return reqJson<StorageBucket[]>(`/api/storage/accounts/${accountId}/buckets`);
  },

  createBucket(accountId: number, name: string): Promise<{ ok: boolean; name: string }> {
    return reqJson<{ ok: boolean; name: string }>(`/api/storage/accounts/${accountId}/buckets`, {
      method: "POST",
      body: JSON.stringify({ name }),
    });
  },

  deleteBucket(accountId: number, bucket: string): Promise<{ ok: boolean }> {
    return reqJson<{ ok: boolean }>(
      `/api/storage/accounts/${accountId}/buckets/${encodeURIComponent(bucket)}`,
      { method: "DELETE" },
    );
  },

  listObjects(
    accountId: number,
    bucket: string,
    prefix: string,
    continuationToken?: string,
  ): Promise<StorageListOut> {
    const qs = new URLSearchParams();
    if (prefix) qs.set("prefix", prefix);
    if (continuationToken) qs.set("continuation_token", continuationToken);
    qs.set("max_keys", "200");
    return reqJson<StorageListOut>(
      `/api/storage/accounts/${accountId}/buckets/${encodeURIComponent(bucket)}/objects?${qs.toString()}`,
    );
  },

  createFolder(accountId: number, bucket: string, prefix: string): Promise<{ ok: boolean; key: string }> {
    return reqJson<{ ok: boolean; key: string }>(
      `/api/storage/accounts/${accountId}/buckets/${encodeURIComponent(bucket)}/folder`,
      {
        method: "POST",
        body: JSON.stringify({ prefix }),
      },
    );
  },

  uploadFiles(
    accountId: number,
    bucket: string,
    prefix: string,
    files: File[],
  ): Promise<{ ok: boolean; uploaded: string[] }> {
    const form = new FormData();
    form.append("prefix", prefix);
    for (const f of files) {
      form.append("file", f, f.name);
    }
    return reqJson<{ ok: boolean; uploaded: string[] }>(
      `/api/storage/accounts/${accountId}/buckets/${encodeURIComponent(bucket)}/upload`,
      {
        method: "POST",
        body: form,
      },
    );
  },

  async downloadObject(accountId: number, bucket: string, key: string): Promise<void> {
    const url = `/api/storage/accounts/${accountId}/buckets/${encodeURIComponent(bucket)}/download?key=${encodeURIComponent(key)}`;
    const res = await fetch(url, { headers: buildHeaders() });
    if (!res.ok) {
      throw new ApiError(await parseError(res), res.status);
    }
    const blob = await res.blob();
    const objUrl = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = objUrl;
    a.download = key.split("/").pop() || "download.bin";
    document.body.appendChild(a);
    a.click();
    a.remove();
    URL.revokeObjectURL(objUrl);
  },

  copyObject(
    accountId: number,
    bucket: string,
    sourceKey: string,
    destKey: string,
    deleteSource: boolean,
  ): Promise<{ ok: boolean; dest_key: string }> {
    return reqJson<{ ok: boolean; dest_key: string }>(
      `/api/storage/accounts/${accountId}/buckets/${encodeURIComponent(bucket)}/copy`,
      {
        method: "POST",
        body: JSON.stringify({
          source_key: sourceKey,
          dest_key: destKey,
          delete_source: deleteSource,
        }),
      },
    );
  },

  deleteObjects(accountId: number, bucket: string, keys: string[]): Promise<StorageDeleteOut> {
    return reqJson<StorageDeleteOut>(
      `/api/storage/accounts/${accountId}/buckets/${encodeURIComponent(bucket)}/delete`,
      {
        method: "POST",
        body: JSON.stringify({ keys }),
      },
    );
  },

  presignObject(
    accountId: number,
    bucket: string,
    key: string,
    expiresSecs = 3600,
  ): Promise<StoragePresignOut> {
    return reqJson<StoragePresignOut>(
      `/api/storage/accounts/${accountId}/buckets/${encodeURIComponent(bucket)}/presign`,
      {
        method: "POST",
        body: JSON.stringify({ key, expires_secs: expiresSecs }),
      },
    );
  },

  listTransfers(): Promise<StorageTransferJob[]> {
    return reqJson<StorageTransferJob[]>("/api/storage/transfers");
  },

  createTransfer(input: StorageTransferCreateInput): Promise<StorageTransferJob> {
    return reqJson<StorageTransferJob>("/api/storage/transfers", {
      method: "POST",
      body: JSON.stringify(input),
    });
  },

  cancelTransfer(jobId: string): Promise<{ ok: boolean }> {
    return reqJson<{ ok: boolean }>(`/api/storage/transfers/${encodeURIComponent(jobId)}/cancel`, {
      method: "POST",
    });
  },
};
