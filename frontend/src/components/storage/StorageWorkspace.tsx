import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Icon } from "../Icon";
import { storageApi } from "../../storageApi";
import {
  buildBreadcrumbs,
  formatStorageBytes,
  formatTransferDirection,
} from "../../storageHelpers";
import type {
  Server,
  StorageAccount,
  StorageBucket,
  StorageObject,
  StorageTransferJob,
} from "../../types";
import { StorageAccountModal } from "./StorageAccountModal";
import { StorageBucketModal } from "./StorageBucketModal";
import { StorageTransferModal } from "./StorageTransferModal";

interface StorageWorkspaceProps {
  servers: Server[];
}

export function StorageWorkspace({ servers }: StorageWorkspaceProps): JSX.Element {
  const [accounts, setAccounts] = useState<StorageAccount[]>([]);
  const [loadingAccounts, setLoadingAccounts] = useState(true);
  const [activeAccountId, setActiveAccountId] = useState<number | null>(null);

  const [buckets, setBuckets] = useState<StorageBucket[]>([]);
  const [loadingBuckets, setLoadingBuckets] = useState(false);
  const [activeBucket, setActiveBucket] = useState<string | null>(null);

  const [prefix, setPrefix] = useState("");
  const [entries, setEntries] = useState<StorageObject[]>([]);
  const [nextToken, setNextToken] = useState<string | null>(null);
  const [loadingObjects, setLoadingObjects] = useState(false);
  const [filterText, setFilterText] = useState("");
  const [selectedKeys, setSelectedKeys] = useState<Set<string>>(new Set());

  const [uploading, setUploading] = useState(false);
  const [notice, setNotice] = useState<{ kind: "ok" | "err"; text: string } | null>(null);

  const [accountModal, setAccountModal] = useState<{ open: boolean; account: StorageAccount | null }>({
    open: false,
    account: null,
  });
  const [bucketModalOpen, setBucketModalOpen] = useState(false);
  const [transferModalOpen, setTransferModalOpen] = useState(false);

  const [jobs, setJobs] = useState<StorageTransferJob[]>([]);
  const fileInputRef = useRef<HTMLInputElement>(null);

  const activeAccount = useMemo(
    () => accounts.find((a) => a.id === activeAccountId) ?? null,
    [accounts, activeAccountId],
  );

  const refreshAccounts = useCallback(async () => {
    setLoadingAccounts(true);
    try {
      const list = await storageApi.listAccounts();
      setAccounts(list);
      setActiveAccountId((prev) => {
        if (prev && list.some((a) => a.id === prev)) return prev;
        return list[0]?.id ?? null;
      });
    } catch (e) {
      setNotice({ kind: "err", text: e instanceof Error ? e.message : "Gagal memuat akun storage" });
    } finally {
      setLoadingAccounts(false);
    }
  }, []);

  const refreshBuckets = useCallback(async (accId: number) => {
    setLoadingBuckets(true);
    setNotice(null);
    try {
      const list = await storageApi.listBuckets(accId);
      setBuckets(list);
      setActiveBucket((prev) => {
        if (prev && list.some((b) => b.name === prev)) return prev;
        return list[0]?.name ?? null;
      });
    } catch (e) {
      setBuckets([]);
      setActiveBucket(null);
      setNotice({ kind: "err", text: e instanceof Error ? e.message : "Gagal memuat daftar bucket" });
    } finally {
      setLoadingBuckets(false);
    }
  }, []);

  const refreshObjects = useCallback(
    async (accId: number, bucket: string, nextPrefix: string, token?: string) => {
      setLoadingObjects(true);
      setNotice(null);
      try {
        const res = await storageApi.listObjects(accId, bucket, nextPrefix, token);
        setEntries((prev) => (token ? [...prev, ...res.entries] : res.entries));
        setNextToken(res.is_truncated ? res.next_continuation_token : null);
        if (!token) setSelectedKeys(new Set());
      } catch (e) {
        if (!token) setEntries([]);
        setNotice({ kind: "err", text: e instanceof Error ? e.message : "Gagal memuat isi bucket" });
      } finally {
        setLoadingObjects(false);
      }
    },
    [],
  );

  const refreshJobs = useCallback(async () => {
    try {
      const list = await storageApi.listTransfers();
      setJobs(list);
    } catch {
      // ignore background poll error
    }
  }, []);

  useEffect(() => {
    void refreshAccounts();
    void refreshJobs();
  }, [refreshAccounts, refreshJobs]);

  useEffect(() => {
    if (activeAccountId) {
      setPrefix("");
      setEntries([]);
      void refreshBuckets(activeAccountId);
    } else {
      setBuckets([]);
      setActiveBucket(null);
      setEntries([]);
    }
  }, [activeAccountId, refreshBuckets]);

  useEffect(() => {
    if (activeAccountId && activeBucket) {
      void refreshObjects(activeAccountId, activeBucket, prefix);
    } else {
      setEntries([]);
    }
  }, [activeAccountId, activeBucket, prefix, refreshObjects]);

  const hasActiveJobs = jobs.some((j) => j.state === "queued" || j.state === "running");
  useEffect(() => {
    if (!hasActiveJobs) return;
    const id = window.setInterval(() => {
      void refreshJobs();
    }, 1000);
    return () => window.clearInterval(id);
  }, [hasActiveJobs, refreshJobs]);

  const breadcrumbs = useMemo(() => buildBreadcrumbs(prefix), [prefix]);
  const visibleEntries = useMemo(() => {
    const q = filterText.trim().toLowerCase();
    if (!q) return entries;
    return entries.filter((e) => e.name.toLowerCase().includes(q));
  }, [entries, filterText]);

  const handleDeleteAccount = async (acc: StorageAccount) => {
    if (!window.confirm(`Hapus profil storage "${acc.name}" dari RusTerm? (Bucket/file di cloud tidak ikut terhapus)`)) {
      return;
    }
    try {
      await storageApi.deleteAccount(acc.id);
      setNotice({ kind: "ok", text: `Akun "${acc.name}" dihapus.` });
      await refreshAccounts();
    } catch (e) {
      setNotice({ kind: "err", text: e instanceof Error ? e.message : "Gagal menghapus akun" });
    }
  };

  const handleDeleteBucket = async (bucketName: string) => {
    if (!activeAccountId) return;
    if (!window.confirm(`Hapus bucket "${bucketName}"? Bucket harus kosong terlebih dahulu.`)) {
      return;
    }
    try {
      await storageApi.deleteBucket(activeAccountId, bucketName);
      setNotice({ kind: "ok", text: `Bucket "${bucketName}" berhasil dihapus.` });
      await refreshBuckets(activeAccountId);
    } catch (e) {
      setNotice({ kind: "err", text: e instanceof Error ? e.message : "Gagal menghapus bucket" });
    }
  };

  const handleCreateFolder = async () => {
    if (!activeAccountId || !activeBucket) return;
    const name = window.prompt("Nama folder baru:");
    if (!name || !name.trim()) return;
    const clean = name.trim().replace(/^\/+|\/+$/g, "");
    try {
      await storageApi.createFolder(activeAccountId, activeBucket, `${prefix}${clean}/`);
      setNotice({ kind: "ok", text: `Folder "${clean}" dibuat.` });
      await refreshObjects(activeAccountId, activeBucket, prefix);
    } catch (e) {
      setNotice({ kind: "err", text: e instanceof Error ? e.message : "Gagal membuat folder" });
    }
  };

  const handleUploadChange = async (files: FileList | null) => {
    if (!files || files.length === 0 || !activeAccountId || !activeBucket) return;
    setUploading(true);
    setNotice(null);
    try {
      const res = await storageApi.uploadFiles(activeAccountId, activeBucket, prefix, Array.from(files));
      setNotice({ kind: "ok", text: `${res.uploaded.length} file berhasil diunggah.` });
      await refreshObjects(activeAccountId, activeBucket, prefix);
    } catch (e) {
      setNotice({ kind: "err", text: e instanceof Error ? e.message : "Gagal mengunggah file" });
    } finally {
      setUploading(false);
      if (fileInputRef.current) fileInputRef.current.value = "";
    }
  };

  const handleDownload = async (obj: StorageObject) => {
    if (!activeAccountId || !activeBucket) return;
    try {
      await storageApi.downloadObject(activeAccountId, activeBucket, obj.key);
    } catch (e) {
      setNotice({ kind: "err", text: e instanceof Error ? e.message : "Gagal mengunduh file" });
    }
  };

  const handleRenameOrCopy = async (obj: StorageObject, move: boolean) => {
    if (!activeAccountId || !activeBucket) return;
    const dest = window.prompt(
      move ? "Pindah / rename ke key baru:" : "Salin ke key baru (dalam bucket ini):",
      obj.key,
    );
    if (!dest || !dest.trim() || dest.trim() === obj.key) return;
    try {
      await storageApi.copyObject(activeAccountId, activeBucket, obj.key, dest.trim(), move);
      setNotice({ kind: "ok", text: move ? `Dipindahkan ke ${dest.trim()}` : `Disalin ke ${dest.trim()}` });
      await refreshObjects(activeAccountId, activeBucket, prefix);
    } catch (e) {
      setNotice({ kind: "err", text: e instanceof Error ? e.message : "Operasi copy/rename gagal" });
    }
  };

  const handlePresign = async (obj: StorageObject) => {
    if (!activeAccountId || !activeBucket) return;
    try {
      const res = await storageApi.presignObject(activeAccountId, activeBucket, obj.key, 3600);
      if (navigator.clipboard?.writeText) {
        await navigator.clipboard.writeText(res.url);
        setNotice({ kind: "ok", text: "Presigned URL (berlaku 1 jam) disalin ke clipboard." });
      } else {
        window.prompt("Presigned URL (berlaku 1 jam):", res.url);
      }
    } catch (e) {
      setNotice({ kind: "err", text: e instanceof Error ? e.message : "Gagal membuat presigned URL" });
    }
  };

  const handleDeleteKeys = async (keys: string[]) => {
    if (!activeAccountId || !activeBucket || keys.length === 0) return;
    if (!window.confirm(`Hapus ${keys.length} item dari bucket "${activeBucket}"?`)) return;
    try {
      const res = await storageApi.deleteObjects(activeAccountId, activeBucket, keys);
      setNotice({
        kind: res.failed.length === 0 ? "ok" : "err",
        text: `${res.deleted.length} dihapus${res.failed.length > 0 ? `, ${res.failed.length} gagal` : ""}.`,
      });
      await refreshObjects(activeAccountId, activeBucket, prefix);
    } catch (e) {
      setNotice({ kind: "err", text: e instanceof Error ? e.message : "Gagal menghapus object" });
    }
  };

  const toggleSelect = (key: string) => {
    setSelectedKeys((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  };

  return (
    <div className="storage-workspace">
      <aside className="storage-sidebar">
        <div className="storage-sidebar-header">
          <span className="storage-sidebar-title">
            <Icon name="cloud" size={15} />
            <span>Akun S3 / R2 / MinIO</span>
          </span>
          <button
            type="button"
            className="btn btn-sm btn-primary"
            onClick={() => setAccountModal({ open: true, account: null })}
          >
            <Icon name="plus" size={13} />
            <span>Akun</span>
          </button>
        </div>

        <div className="storage-account-list">
          {loadingAccounts && <div className="storage-empty">Memuat akun...</div>}
          {!loadingAccounts && accounts.length === 0 && (
            <div className="storage-empty">
              Belum ada akun storage. Klik <strong>+ Akun</strong> untuk menambah S3, Cloudflare R2, atau MinIO.
            </div>
          )}
          {accounts.map((acc) => {
            const selected = acc.id === activeAccountId;
            return (
              <div
                key={acc.id}
                className={`storage-account-item${selected ? " is-active" : ""}`}
                onClick={() => setActiveAccountId(acc.id)}
              >
                <div className="storage-account-meta">
                  <div className="storage-account-name">{acc.name}</div>
                  <div className="storage-account-sub">
                    <span className="storage-badge">{acc.provider.toUpperCase()}</span>
                    <span>{acc.region}</span>
                  </div>
                </div>
                <div className="storage-row-actions" onClick={(e) => e.stopPropagation()}>
                  <button
                    type="button"
                    className="icon-btn"
                    title="Edit akun"
                    onClick={() => setAccountModal({ open: true, account: acc })}
                  >
                    <Icon name="edit" size={13} />
                  </button>
                  <button
                    type="button"
                    className="icon-btn"
                    title="Hapus akun"
                    onClick={() => void handleDeleteAccount(acc)}
                  >
                    <Icon name="trash" size={13} />
                  </button>
                </div>
              </div>
            );
          })}
        </div>

        {activeAccount && (
          <div className="storage-buckets-section">
            <div className="storage-sidebar-header">
              <span className="storage-sidebar-title">Bucket</span>
              <div className="storage-row-actions">
                <button
                  type="button"
                  className="icon-btn"
                  title="Muat ulang bucket"
                  onClick={() => void refreshBuckets(activeAccount.id)}
                >
                  <Icon name="refresh" size={13} />
                </button>
                <button
                  type="button"
                  className="btn btn-sm"
                  onClick={() => setBucketModalOpen(true)}
                >
                  <Icon name="plus" size={12} />
                  <span>Bucket</span>
                </button>
              </div>
            </div>
            <div className="storage-bucket-list">
              {loadingBuckets && <div className="storage-empty">Memuat bucket...</div>}
              {!loadingBuckets && buckets.length === 0 && (
                <div className="storage-empty">Belum ada bucket.</div>
              )}
              {buckets.map((b) => (
                <div
                  key={b.name}
                  className={`storage-bucket-item${activeBucket === b.name ? " is-active" : ""}`}
                  onClick={() => {
                    setActiveBucket(b.name);
                    setPrefix("");
                  }}
                >
                  <span className="storage-bucket-name">
                    <Icon name="box" size={13} />
                    <span>{b.name}</span>
                  </span>
                  <button
                    type="button"
                    className="icon-btn"
                    title="Hapus bucket kosong"
                    onClick={(e) => {
                      e.stopPropagation();
                      void handleDeleteBucket(b.name);
                    }}
                  >
                    <Icon name="trash" size={12} />
                  </button>
                </div>
              ))}
            </div>
          </div>
        )}
      </aside>

      <main className="storage-main">
        <div className="storage-toolbar">
          <div className="storage-breadcrumbs">
            {activeBucket ? (
              <>
                <button
                  type="button"
                  className="storage-crumb"
                  onClick={() => setPrefix("")}
                >
                  {activeBucket}
                </button>
                {breadcrumbs.map((bc) => (
                  <span key={bc.prefix} className="storage-crumb-wrap">
                    <span className="muted">/</span>
                    <button
                      type="button"
                      className="storage-crumb"
                      onClick={() => setPrefix(bc.prefix)}
                    >
                      {bc.label}
                    </button>
                  </span>
                ))}
              </>
            ) : (
              <span className="muted">Pilih akun dan bucket di panel kiri</span>
            )}
          </div>

          <div className="storage-actions">
            <input
              ref={fileInputRef}
              type="file"
              multiple
              hidden
              onChange={(e) => void handleUploadChange(e.target.files)}
            />
            <input
              type="search"
              className="storage-search"
              placeholder="Filter nama..."
              value={filterText}
              onChange={(e) => setFilterText(e.target.value)}
            />
            <button
              type="button"
              className="btn btn-sm btn-primary"
              disabled={!activeBucket || uploading}
              onClick={() => fileInputRef.current?.click()}
            >
              <Icon name="upload" size={13} />
              <span>{uploading ? "Mengunggah..." : "Upload"}</span>
            </button>
            <button
              type="button"
              className="btn btn-sm"
              disabled={!activeBucket}
              onClick={() => void handleCreateFolder()}
            >
              <Icon name="folder" size={13} />
              <span>Folder</span>
            </button>
            <button
              type="button"
              className="btn btn-sm"
              disabled={accounts.length === 0}
              onClick={() => setTransferModalOpen(true)}
            >
              <Icon name="send" size={13} />
              <span>Sync SSH / S3</span>
            </button>
            {selectedKeys.size > 0 && (
              <button
                type="button"
                className="btn btn-sm btn-danger"
                onClick={() => void handleDeleteKeys(Array.from(selectedKeys))}
              >
                <Icon name="trash" size={13} />
                <span>Hapus ({selectedKeys.size})</span>
              </button>
            )}
            <button
              type="button"
              className="icon-btn"
              title="Muat ulang"
              disabled={!activeAccountId || !activeBucket}
              onClick={() =>
                activeAccountId &&
                activeBucket &&
                void refreshObjects(activeAccountId, activeBucket, prefix)
              }
            >
              <Icon name="refresh" size={14} />
            </button>
          </div>
        </div>

        {notice && (
          <div className={`storage-banner ${notice.kind === "ok" ? "is-ok" : "is-err"}`}>
            <span>{notice.text}</span>
            <button type="button" className="icon-btn" onClick={() => setNotice(null)}>
              <Icon name="close" size={12} />
            </button>
          </div>
        )}

        <div className="storage-table-wrap">
          <table className="storage-table">
            <thead>
              <tr>
                <th style={{ width: 32 }} />
                <th>Nama</th>
                <th style={{ width: 110 }}>Ukuran</th>
                <th style={{ width: 180 }}>Terakhir Diubah</th>
                <th style={{ width: 170 }}>Aksi</th>
              </tr>
            </thead>
            <tbody>
              {prefix && (
                <tr
                  className="storage-row is-dir"
                  onClick={() => {
                    const parts = prefix.replace(/\/+$/, "").split("/");
                    parts.pop();
                    setPrefix(parts.length > 0 ? `${parts.join("/")}/` : "");
                  }}
                >
                  <td />
                  <td colSpan={4}>
                    <span className="storage-name-cell">
                      <Icon name="folder" size={14} />
                      <span>..</span>
                    </span>
                  </td>
                </tr>
              )}
              {loadingObjects && entries.length === 0 && (
                <tr>
                  <td colSpan={5} className="storage-empty">
                    Memuat object...
                  </td>
                </tr>
              )}
              {!loadingObjects && activeBucket && visibleEntries.length === 0 && (
                <tr>
                  <td colSpan={5} className="storage-empty">
                    Folder kosong.
                  </td>
                </tr>
              )}
              {visibleEntries.map((obj) => (
                <tr key={obj.key} className={`storage-row${obj.is_dir ? " is-dir" : ""}`}>
                  <td>
                    {!obj.is_dir && (
                      <input
                        type="checkbox"
                        checked={selectedKeys.has(obj.key)}
                        onChange={() => toggleSelect(obj.key)}
                      />
                    )}
                  </td>
                  <td
                    className="storage-name-td"
                    onClick={() => {
                      if (obj.is_dir) setPrefix(obj.key);
                    }}
                  >
                    <span className="storage-name-cell">
                      <Icon name={obj.is_dir ? "folder" : "file"} size={14} />
                      <span>{obj.name}</span>
                    </span>
                  </td>
                  <td>{obj.is_dir ? "—" : formatStorageBytes(obj.size)}</td>
                  <td className="muted">
                    {obj.last_modified ? obj.last_modified.replace("T", " ").slice(0, 19) : "—"}
                  </td>
                  <td>
                    <div className="storage-row-actions">
                      {!obj.is_dir && (
                        <>
                          <button
                            type="button"
                            className="icon-btn"
                            title="Download"
                            onClick={() => void handleDownload(obj)}
                          >
                            <Icon name="download" size={13} />
                          </button>
                          <button
                            type="button"
                            className="icon-btn"
                            title="Salin Presigned URL (1 jam)"
                            onClick={() => void handlePresign(obj)}
                          >
                            <Icon name="copy" size={13} />
                          </button>
                          <button
                            type="button"
                            className="icon-btn"
                            title="Rename / Pindah"
                            onClick={() => void handleRenameOrCopy(obj, true)}
                          >
                            <Icon name="edit" size={13} />
                          </button>
                        </>
                      )}
                      <button
                        type="button"
                        className="icon-btn"
                        title="Hapus"
                        onClick={() => void handleDeleteKeys([obj.key])}
                      >
                        <Icon name="trash" size={13} />
                      </button>
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>

          {nextToken && activeAccountId && activeBucket && (
            <div className="storage-load-more">
              <button
                type="button"
                className="btn btn-sm"
                disabled={loadingObjects}
                onClick={() => void refreshObjects(activeAccountId, activeBucket, prefix, nextToken)}
              >
                Muat Lebih Banyak...
              </button>
            </div>
          )}
        </div>

        {jobs.length > 0 && (
          <div className="storage-jobs-panel">
            <div className="storage-jobs-header">
              <span>Transfer Jobs ({jobs.length})</span>
              <button type="button" className="icon-btn" onClick={() => void refreshJobs()}>
                <Icon name="refresh" size={12} />
              </button>
            </div>
            <div className="storage-jobs-list">
              {jobs.slice(0, 6).map((job) => (
                <div key={job.id} className={`storage-job-row state-${job.state}`}>
                  <div className="storage-job-info">
                    <span className="storage-badge">
                      {formatTransferDirection(job.direction as "ssh_to_s3" | "s3_to_ssh" | "s3_to_s3")}
                    </span>
                    <span className="storage-badge">{job.effective_mode}</span>
                    <span className="storage-job-label">
                      {job.source_label} → {job.dest_label}
                    </span>
                  </div>
                  <div className="storage-job-meta">
                    <span>
                      {job.transferred_files}/{job.total_files} file (
                      {formatStorageBytes(job.transferred_bytes)}/{formatStorageBytes(job.total_bytes)})
                    </span>
                    <span className={`storage-job-state state-${job.state}`}>{job.state}</span>
                    {(job.state === "queued" || job.state === "running") && (
                      <button
                        type="button"
                        className="btn btn-sm"
                        onClick={() => void storageApi.cancelTransfer(job.id).then(refreshJobs)}
                      >
                        Batal
                      </button>
                    )}
                  </div>
                  {job.error && <div className="storage-job-err">{job.error}</div>}
                </div>
              ))}
            </div>
          </div>
        )}
      </main>

      {accountModal.open && (
        <StorageAccountModal
          account={accountModal.account}
          onClose={() => setAccountModal({ open: false, account: null })}
          onSaved={async (saved) => {
            setAccountModal({ open: false, account: null });
            await refreshAccounts();
            setActiveAccountId(saved.id);
          }}
        />
      )}

      {bucketModalOpen && activeAccount && (
        <StorageBucketModal
          accountId={activeAccount.id}
          accountName={activeAccount.name}
          onClose={() => setBucketModalOpen(false)}
          onCreated={async (bName) => {
            setBucketModalOpen(false);
            await refreshBuckets(activeAccount.id);
            setActiveBucket(bName);
            setPrefix("");
          }}
        />
      )}

      {transferModalOpen && (
        <StorageTransferModal
          accounts={accounts}
          servers={servers}
          activeAccountId={activeAccountId}
          activeBucket={activeBucket}
          activePrefix={prefix}
          onClose={() => setTransferModalOpen(false)}
          onStarted={(job) => {
            setTransferModalOpen(false);
            setJobs((prev) => [job, ...prev]);
          }}
        />
      )}
    </div>
  );
}
