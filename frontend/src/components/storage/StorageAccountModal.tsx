import { useState, type FormEvent } from "react";
import { Modal } from "../Modal";
import { storageApi } from "../../storageApi";
import { STORAGE_PROVIDER_PRESETS, getProviderPreset } from "../../storageHelpers";
import type { StorageAccount, StorageProvider } from "../../types";

interface StorageAccountModalProps {
  account: StorageAccount | null;
  onClose: () => void;
  onSaved: (saved: StorageAccount) => void;
}

export function StorageAccountModal({ account, onClose, onSaved }: StorageAccountModalProps): JSX.Element {
  const isEdit = Boolean(account);
  const initialPreset = getProviderPreset(account?.provider ?? "r2");

  const [name, setName] = useState(account?.name ?? "");
  const [provider, setProvider] = useState<StorageProvider>((account?.provider as StorageProvider) ?? "r2");
  const [endpoint, setEndpoint] = useState(account?.endpoint ?? initialPreset.defaultEndpoint);
  const [region, setRegion] = useState(account?.region ?? initialPreset.defaultRegion);
  const [accessKeyId, setAccessKeyId] = useState(account?.access_key_id ?? "");
  const [secretKey, setSecretKey] = useState("");
  const [defaultBucket, setDefaultBucket] = useState(account?.default_bucket ?? "");
  const [pathStyle, setPathStyle] = useState(account?.path_style ?? initialPreset.defaultPathStyle);

  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<{ ok: boolean; message: string } | null>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const preset = getProviderPreset(provider);

  const handleProviderChange = (next: StorageProvider) => {
    setProvider(next);
    const p = getProviderPreset(next);
    if (!isEdit) {
      setEndpoint(p.defaultEndpoint);
      setRegion(p.defaultRegion);
      setPathStyle(p.defaultPathStyle);
    }
    setTestResult(null);
  };

  const handleTest = async () => {
    setError(null);
    setTestResult(null);
    setTesting(true);
    try {
      const res = await storageApi.verifyAccount({
        account_id: account?.id ?? null,
        provider,
        endpoint: endpoint.trim(),
        region: region.trim() || preset.defaultRegion,
        access_key_id: accessKeyId.trim(),
        secret_access_key: secretKey ? secretKey : null,
        default_bucket: defaultBucket.trim(),
        path_style: pathStyle,
      });
      setTestResult({ ok: res.ok, message: res.message });
    } catch (e) {
      setTestResult({
        ok: false,
        message: e instanceof Error ? e.message : "Gagal terhubung ke endpoint",
      });
    } finally {
      setTesting(false);
    }
  };

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    setError(null);
    if (!name.trim()) {
      setError("Nama profil wajib diisi.");
      return;
    }
    if (!endpoint.trim()) {
      setError("Endpoint URL wajib diisi.");
      return;
    }
    if (!accessKeyId.trim()) {
      setError("Access Key ID wajib diisi.");
      return;
    }
    if (!isEdit && !secretKey) {
      setError("Secret Access Key wajib diisi.");
      return;
    }
    setSaving(true);
    try {
      const saved = account
        ? await storageApi.updateAccount(account.id, {
            name: name.trim(),
            provider,
            endpoint: endpoint.trim(),
            region: region.trim() || preset.defaultRegion,
            access_key_id: accessKeyId.trim(),
            secret_access_key: secretKey ? secretKey : null,
            default_bucket: defaultBucket.trim(),
            path_style: pathStyle,
          })
        : await storageApi.createAccount({
            name: name.trim(),
            provider,
            endpoint: endpoint.trim(),
            region: region.trim() || preset.defaultRegion,
            access_key_id: accessKeyId.trim(),
            secret_access_key: secretKey,
            default_bucket: defaultBucket.trim(),
            path_style: pathStyle,
          });
      onSaved(saved);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Gagal menyimpan akun storage");
    } finally {
      setSaving(false);
    }
  };

  return (
    <Modal title={isEdit ? "Edit Akun Storage" : "Tambah Akun Storage (S3 / R2 / MinIO)"} onClose={onClose}>
      <form className="form" onSubmit={handleSubmit}>
        <div className="field">
          <label>Provider</label>
          <div className="storage-provider-pills">
            {STORAGE_PROVIDER_PRESETS.map((p) => (
              <button
                key={p.id}
                type="button"
                className={`btn btn-sm${provider === p.id ? " btn-primary" : ""}`}
                onClick={() => handleProviderChange(p.id)}
              >
                {p.label}
              </button>
            ))}
          </div>
          <small className="muted">{preset.hint}</small>
        </div>

        <div className="field">
          <label htmlFor="st-name">Nama Profil</label>
          <input
            id="st-name"
            type="text"
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="Misal: R2 Production / MinIO Lokal"
            autoFocus
            required
          />
        </div>

        <div className="field">
          <label htmlFor="st-endpoint">Endpoint URL</label>
          <input
            id="st-endpoint"
            type="url"
            value={endpoint}
            onChange={(e) => setEndpoint(e.target.value)}
            placeholder={preset.endpointPlaceholder}
            required
          />
        </div>

        <div className="storage-form-row">
          <div className="field">
            <label htmlFor="st-region">Region</label>
            <input
              id="st-region"
              type="text"
              value={region}
              onChange={(e) => setRegion(e.target.value)}
              placeholder={preset.defaultRegion}
            />
          </div>
          <div className="field">
            <label htmlFor="st-bucket">Default Bucket (opsional)</label>
            <input
              id="st-bucket"
              type="text"
              value={defaultBucket}
              onChange={(e) => setDefaultBucket(e.target.value)}
              placeholder="Untuk token scoped 1 bucket"
            />
          </div>
        </div>

        <div className="field">
          <label htmlFor="st-ak">Access Key ID</label>
          <input
            id="st-ak"
            type="text"
            value={accessKeyId}
            onChange={(e) => setAccessKeyId(e.target.value)}
            autoComplete="off"
            required
          />
        </div>

        <div className="field">
          <label htmlFor="st-sk">
            Secret Access Key {isEdit && <span className="muted">(kosongkan untuk tetap memakai key lama)</span>}
          </label>
          <input
            id="st-sk"
            type="password"
            value={secretKey}
            onChange={(e) => setSecretKey(e.target.value)}
            autoComplete="new-password"
            placeholder={isEdit ? "•••••••• (tersimpan terenkripsi)" : ""}
            required={!isEdit}
          />
        </div>

        <label className="storage-checkbox">
          <input type="checkbox" checked={pathStyle} onChange={(e) => setPathStyle(e.target.checked)} />
          <span>Gunakan Path-Style URL (disarankan untuk MinIO &amp; Cloudflare R2)</span>
        </label>

        {testResult && (
          <div className={`storage-banner ${testResult.ok ? "is-ok" : "is-err"}`}>
            {testResult.message}
          </div>
        )}
        {error && <div className="storage-banner is-err">{error}</div>}

        <div className="modal-actions">
          <button type="button" className="btn" onClick={handleTest} disabled={testing || saving}>
            {testing ? "Menguji..." : "Test Koneksi"}
          </button>
          <div style={{ flex: 1 }} />
          <button type="button" className="btn" onClick={onClose} disabled={saving}>
            Batal
          </button>
          <button type="submit" className="btn btn-primary" disabled={saving}>
            {saving ? "Menyimpan..." : isEdit ? "Simpan Perubahan" : "Simpan Akun"}
          </button>
        </div>
      </form>
    </Modal>
  );
}
