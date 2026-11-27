import { useState, type FormEvent } from "react";
import { Modal } from "../Modal";
import { storageApi } from "../../storageApi";
import { validateBucketName } from "../../storageHelpers";

interface StorageBucketModalProps {
  accountId: number;
  accountName: string;
  onClose: () => void;
  onCreated: (bucketName: string) => void;
}

export function StorageBucketModal({
  accountId,
  accountName,
  onClose,
  onCreated,
}: StorageBucketModalProps): JSX.Element {
  const [name, setName] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    const validation = validateBucketName(name);
    if (validation) {
      setError(validation);
      return;
    }
    setError(null);
    setSaving(true);
    try {
      const clean = name.trim();
      await storageApi.createBucket(accountId, clean);
      onCreated(clean);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Gagal membuat bucket");
    } finally {
      setSaving(false);
    }
  };

  return (
    <Modal title={`Buat Bucket Baru (${accountName})`} onClose={onClose}>
      <form className="form" onSubmit={handleSubmit}>
        <div className="field">
          <label htmlFor="bucket-name">Nama Bucket</label>
          <input
            id="bucket-name"
            type="text"
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="misal: backup-prod-2026"
            autoFocus
            required
          />
          <small className="muted">Huruf kecil, angka, tanda hubung (-), atau titik (.), 3–63 karakter.</small>
        </div>

        {error && <div className="storage-banner is-err">{error}</div>}

        <div className="modal-actions">
          <button type="button" className="btn" onClick={onClose} disabled={saving}>
            Batal
          </button>
          <button type="submit" className="btn btn-primary" disabled={saving}>
            {saving ? "Membuat..." : "Buat Bucket"}
          </button>
        </div>
      </form>
    </Modal>
  );
}
