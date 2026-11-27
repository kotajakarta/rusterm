import { useEffect, useState, type FormEvent } from "react";
import { Modal } from "../Modal";
import { storageApi } from "../../storageApi";
import type {
  Server,
  StorageAccount,
  StorageBucket,
  StorageTransferDirection,
  StorageTransferJob,
  StorageTransferMode,
} from "../../types";

interface StorageTransferModalProps {
  accounts: StorageAccount[];
  servers: Server[];
  activeAccountId: number | null;
  activeBucket: string | null;
  activePrefix: string;
  onClose: () => void;
  onStarted: (job: StorageTransferJob) => void;
}

export function StorageTransferModal({
  accounts,
  servers,
  activeAccountId,
  activeBucket,
  activePrefix,
  onClose,
  onStarted,
}: StorageTransferModalProps): JSX.Element {
  const [direction, setDirection] = useState<StorageTransferDirection>(
    servers.length > 0 ? "ssh_to_s3" : "s3_to_s3",
  );
  const [mode, setMode] = useState<StorageTransferMode>("auto");
  const [overwrite, setOverwrite] = useState(true);

  const [serverId, setServerId] = useState<number>(servers[0]?.id ?? 0);
  const [sshPath, setSshPath] = useState("/tmp");

  const [srcAccountId, setSrcAccountId] = useState<number>(activeAccountId ?? accounts[0]?.id ?? 0);
  const [srcBucket, setSrcBucket] = useState<string>(activeBucket ?? "");
  const [srcPrefix, setSrcPrefix] = useState<string>(activePrefix);
  const [srcBuckets, setSrcBuckets] = useState<StorageBucket[]>([]);

  const [dstAccountId, setDstAccountId] = useState<number>(activeAccountId ?? accounts[0]?.id ?? 0);
  const [dstBucket, setDstBucket] = useState<string>(activeBucket ?? "");
  const [dstPrefix, setDstPrefix] = useState<string>("");
  const [dstBuckets, setDstBuckets] = useState<StorageBucket[]>([]);

  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!srcAccountId) return;
    storageApi
      .listBuckets(srcAccountId)
      .then((list) => {
        setSrcBuckets(list);
        if (!srcBucket && list.length > 0) setSrcBucket(list[0].name);
      })
      .catch(() => setSrcBuckets([]));
  }, [srcAccountId]);

  useEffect(() => {
    if (!dstAccountId) return;
    storageApi
      .listBuckets(dstAccountId)
      .then((list) => {
        setDstBuckets(list);
        if (!dstBucket && list.length > 0) setDstBucket(list[0].name);
      })
      .catch(() => setDstBuckets([]));
  }, [dstAccountId]);

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    setError(null);
    setSubmitting(true);
    try {
      let job: StorageTransferJob;
      if (direction === "ssh_to_s3") {
        if (!serverId) throw new Error("Pilih server SSH terlebih dahulu.");
        if (!dstAccountId || !dstBucket) throw new Error("Pilih akun dan bucket tujuan.");
        job = await storageApi.createTransfer({
          direction,
          mode,
          overwrite,
          server_id: serverId,
          ssh_path: sshPath.trim(),
          account_id: dstAccountId,
          bucket: dstBucket,
          s3_prefix: dstPrefix.trim(),
        });
      } else if (direction === "s3_to_ssh") {
        if (!serverId) throw new Error("Pilih server SSH tujuan.");
        if (!srcAccountId || !srcBucket) throw new Error("Pilih akun dan bucket sumber.");
        job = await storageApi.createTransfer({
          direction,
          mode,
          overwrite,
          server_id: serverId,
          ssh_path: sshPath.trim(),
          account_id: srcAccountId,
          bucket: srcBucket,
          s3_prefix: srcPrefix.trim(),
        });
      } else {
        if (!srcAccountId || !srcBucket || !dstAccountId || !dstBucket) {
          throw new Error("Pilih akun dan bucket sumber serta tujuan.");
        }
        job = await storageApi.createTransfer({
          direction: "s3_to_s3",
          mode: "auto",
          overwrite,
          source_account_id: srcAccountId,
          source_bucket: srcBucket,
          source_prefix: srcPrefix.trim(),
          dest_account_id: dstAccountId,
          dest_bucket: dstBucket,
          dest_prefix: dstPrefix.trim(),
        });
      }
      onStarted(job);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Gagal memulai transfer");
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <Modal title="Sync / Transfer Storage (Tanpa rclone)" onClose={onClose}>
      <form className="form" onSubmit={handleSubmit}>
        <div className="field">
          <label>Arah Transfer</label>
          <div className="storage-provider-pills">
            <button
              type="button"
              className={`btn btn-sm${direction === "ssh_to_s3" ? " btn-primary" : ""}`}
              onClick={() => setDirection("ssh_to_s3")}
              disabled={servers.length === 0}
            >
              Server SSH → S3
            </button>
            <button
              type="button"
              className={`btn btn-sm${direction === "s3_to_ssh" ? " btn-primary" : ""}`}
              onClick={() => setDirection("s3_to_ssh")}
              disabled={servers.length === 0}
            >
              S3 → Server SSH
            </button>
            <button
              type="button"
              className={`btn btn-sm${direction === "s3_to_s3" ? " btn-primary" : ""}`}
              onClick={() => setDirection("s3_to_s3")}
            >
              S3 → S3 (Lintas Bucket/Akun)
            </button>
          </div>
        </div>

        {(direction === "ssh_to_s3" || direction === "s3_to_ssh") && (
          <div className="storage-form-row">
            <div className="field">
              <label htmlFor="tr-server">Server SSH</label>
              <select
                id="tr-server"
                value={serverId}
                onChange={(e) => setServerId(Number(e.target.value))}
              >
                {servers.map((s) => (
                  <option key={s.id} value={s.id}>
                    {s.name} ({s.host})
                  </option>
                ))}
              </select>
            </div>
            <div className="field">
              <label htmlFor="tr-ssh-path">
                {direction === "ssh_to_s3" ? "Path Sumber (file/folder)" : "Folder Tujuan di Server"}
              </label>
              <input
                id="tr-ssh-path"
                type="text"
                value={sshPath}
                onChange={(e) => setSshPath(e.target.value)}
                placeholder="/var/backups/app"
                required
              />
            </div>
          </div>
        )}

        {(direction === "s3_to_ssh" || direction === "s3_to_s3") && (
          <div className="storage-transfer-box">
            <div className="storage-box-title">Sumber S3</div>
            <div className="storage-form-row">
              <div className="field">
                <label>Akun</label>
                <select
                  value={srcAccountId}
                  onChange={(e) => {
                    setSrcAccountId(Number(e.target.value));
                    setSrcBucket("");
                  }}
                >
                  {accounts.map((a) => (
                    <option key={a.id} value={a.id}>
                      {a.name} ({a.provider.toUpperCase()})
                    </option>
                  ))}
                </select>
              </div>
              <div className="field">
                <label>Bucket</label>
                <select value={srcBucket} onChange={(e) => setSrcBucket(e.target.value)}>
                  <option value="">-- pilih bucket --</option>
                  {srcBuckets.map((b) => (
                    <option key={b.name} value={b.name}>
                      {b.name}
                    </option>
                  ))}
                </select>
              </div>
            </div>
            <div className="field">
              <label>Prefix / Key Sumber (kosong = seluruh bucket)</label>
              <input
                type="text"
                value={srcPrefix}
                onChange={(e) => setSrcPrefix(e.target.value)}
                placeholder="backups/2026/"
              />
            </div>
          </div>
        )}

        {(direction === "ssh_to_s3" || direction === "s3_to_s3") && (
          <div className="storage-transfer-box">
            <div className="storage-box-title">Tujuan S3</div>
            <div className="storage-form-row">
              <div className="field">
                <label>Akun</label>
                <select
                  value={dstAccountId}
                  onChange={(e) => {
                    setDstAccountId(Number(e.target.value));
                    setDstBucket("");
                  }}
                >
                  {accounts.map((a) => (
                    <option key={a.id} value={a.id}>
                      {a.name} ({a.provider.toUpperCase()})
                    </option>
                  ))}
                </select>
              </div>
              <div className="field">
                <label>Bucket</label>
                <select value={dstBucket} onChange={(e) => setDstBucket(e.target.value)}>
                  <option value="">-- pilih bucket --</option>
                  {dstBuckets.map((b) => (
                    <option key={b.name} value={b.name}>
                      {b.name}
                    </option>
                  ))}
                </select>
              </div>
            </div>
            <div className="field">
              <label>Prefix Tujuan (opsional)</label>
              <input
                type="text"
                value={dstPrefix}
                onChange={(e) => setDstPrefix(e.target.value)}
                placeholder="archive/vps-1/"
              />
            </div>
          </div>
        )}

        {(direction === "ssh_to_s3" || direction === "s3_to_ssh") && (
          <div className="field">
            <label htmlFor="tr-mode">Mode Jalur Transfer</label>
            <select
              id="tr-mode"
              value={mode}
              onChange={(e) => setMode(e.target.value as StorageTransferMode)}
            >
              <option value="auto">Auto (Direct Presigned URL → fallback Relay jika privat)</option>
              <option value="direct">Paksa Direct Presigned URL (VPS ↔ S3 langsung)</option>
              <option value="relay">Paksa Relay lewat RusTerm</option>
            </select>
          </div>
        )}

        <label className="storage-checkbox">
          <input type="checkbox" checked={overwrite} onChange={(e) => setOverwrite(e.target.checked)} />
          <span>Timpa file jika sudah ada di tujuan</span>
        </label>

        {error && <div className="storage-banner is-err">{error}</div>}

        <div className="modal-actions">
          <button type="button" className="btn" onClick={onClose} disabled={submitting}>
            Batal
          </button>
          <button type="submit" className="btn btn-primary" disabled={submitting}>
            {submitting ? "Memulai..." : "Mulai Transfer"}
          </button>
        </div>
      </form>
    </Modal>
  );
}
