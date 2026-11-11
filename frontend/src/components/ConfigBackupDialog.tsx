import { useRef, useState, type FormEvent } from "react";
import { api, errorMessage } from "../api";
import { downloadBlob } from "../download";
import type { BackupStats } from "../types";
import { Icon } from "./Icon";
import { Modal } from "./Modal";

interface ConfigBackupDialogProps {
  onClose: () => void;
  onImportSuccess: () => void;
  onOpenCloudSync?: () => void;
}

export function ConfigBackupDialog({ onClose, onImportSuccess, onOpenCloudSync }: ConfigBackupDialogProps) {
  const [activeTab, setActiveTab] = useState<"export" | "import">("export");

  // Export state
  const [exportPin, setExportPin] = useState("");
  const [exportConfirmPin, setExportConfirmPin] = useState("");
  const [exporting, setExporting] = useState(false);
  const [exportSuccess, setExportSuccess] = useState(false);
  const [exportError, setExportError] = useState<string | null>(null);

  // Import state
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [importPin, setImportPin] = useState("");
  const [selectedFile, setSelectedFile] = useState<File | null>(null);
  const [importing, setImporting] = useState(false);
  const [importStats, setImportStats] = useState<BackupStats | null>(null);
  const [importError, setImportError] = useState<string | null>(null);

  const handleExport = async (e: FormEvent) => {
    e.preventDefault();
    if (exportPin.length < 4) {
      setExportError("PIN enkripsi minimal 4 karakter.");
      return;
    }
    if (exportPin !== exportConfirmPin) {
      setExportError("Konfirmasi PIN tidak cocok.");
      return;
    }

    setExporting(true);
    setExportError(null);
    try {
      const envelope = await api.exportConfig(exportPin);
      const jsonStr = JSON.stringify(envelope, null, 2);
      const blob = new Blob([jsonStr], { type: "application/json" });
      const dateStr = new Date().toISOString().split("T")[0];
      await downloadBlob(blob, `rusterm-backup-${dateStr}.tdk`);
      setExportSuccess(true);
    } catch (err) {
      setExportError(errorMessage(err));
    } finally {
      setExporting(false);
    }
  };

  const handleImport = async (e: FormEvent) => {
    e.preventDefault();
    if (!selectedFile) {
      setImportError("Pilih file backup (.tdk atau .json) terlebih dahulu.");
      return;
    }
    if (!importPin) {
      setImportError("Masukkan PIN pembuka backup.");
      return;
    }

    setImporting(true);
    setImportError(null);
    try {
      const fileText = await selectedFile.text();
      let backupObj: object;
      try {
        backupObj = JSON.parse(fileText);
      } catch {
        throw new Error("Format file bukan JSON/TDK yang valid.");
      }

      const stats = await api.importConfig(importPin, backupObj);
      setImportStats(stats);
      onImportSuccess();
    } catch (err) {
      setImportError(errorMessage(err));
    } finally {
      setImporting(false);
    }
  };

  return (
    <Modal
      title="Cadangkan & Pulihkan Konfigurasi"
      onClose={onClose}
      footer={
        <>
          <span className="spacer" />
          <button type="button" className="btn" onClick={onClose}>
            {importStats || exportSuccess ? "Tutup" : "Batal"}
          </button>
        </>
      }
    >
      <div className="backup-modal-tabs">
        <button
          type="button"
          className={`tab-btn${activeTab === "export" ? " is-active" : ""}`}
          onClick={() => {
            setActiveTab("export");
            setExportError(null);
          }}
        >
          <Icon name="download" size={14} /> <span>Export (Cadangkan)</span>
        </button>
        <button
          type="button"
          className={`tab-btn${activeTab === "import" ? " is-active" : ""}`}
          onClick={() => {
            setActiveTab("import");
            setImportError(null);
          }}
        >
          <Icon name="upload" size={14} /> <span>Import (Gabungkan)</span>
        </button>
        {onOpenCloudSync && (
          <button
            type="button"
            className="tab-btn"
            onClick={() => {
              onClose();
              onOpenCloudSync();
            }}
            title="Sinkronisasi otomatis antar device via Cloudflare KV"
          >
            <Icon name="cloud" size={14} /> <span>Sinkron Cloud (KV)</span>
          </button>
        )}
      </div>

      {activeTab === "export" && (
        <form className="backup-form" onSubmit={handleExport}>
          <p className="backup-desc">
            File backup akan dienkripsi dengan standar <strong>AES-256-GCM</strong>. Berkas ini mencakup seluruh data server,
            kredensial SSH, daftar path bookmark sidebar, dan snippet agar mudah dipindahkan antar perangkat Android.
          </p>

          <label className="form-field">
            <span>PIN / Password Enkripsi Backup</span>
            <input
              type="password"
              placeholder="Minimal 4 karakter"
              value={exportPin}
              onChange={(e) => setExportPin(e.target.value)}
              disabled={exporting}
              required
              autoFocus
            />
          </label>

          <label className="form-field">
            <span>Konfirmasi PIN</span>
            <input
              type="password"
              placeholder="Ulangi PIN di atas"
              value={exportConfirmPin}
              onChange={(e) => setExportConfirmPin(e.target.value)}
              disabled={exporting}
              required
            />
          </label>

          {exportError && <p className="form-error">{exportError}</p>}
          {exportSuccess && (
            <p className="form-success">
              ✓ Berkas backup berhasil dibuat dan diunduh. Simpan berkas ini dengan aman!
            </p>
          )}

          <div className="backup-actions">
            <button type="submit" className="btn btn-primary" disabled={exporting || !exportPin || !exportConfirmPin}>
              {exporting ? "Mengenkripsi…" : "Unduh File Backup (.tdk)"}
            </button>
          </div>
        </form>
      )}

      {activeTab === "import" && (
        <form className="backup-form" onSubmit={handleImport}>
          <p className="backup-desc">
            Pilih file backup terenkripsi (<code>.tdk</code>) lalu masukkan PIN pembukanya.
            Data akan di-<strong>Merge</strong> secara aman: server dan path baru akan ditambahkan tanpa menghapus server yang sudah ada.
          </p>

          <div className="form-field">
            <span>Pilih Berkas Backup (.tdk / .json)</span>
            <input
              ref={fileInputRef}
              type="file"
              accept=".tdk,.json,application/json"
              style={{ display: "none" }}
              onChange={(e) => {
                const file = e.target.files?.[0];
                if (file) {
                  setSelectedFile(file);
                  setImportError(null);
                }
              }}
            />
            <div className="file-pick-row">
              <button
                type="button"
                className="btn"
                onClick={() => fileInputRef.current?.click()}
                disabled={importing}
              >
                <Icon name="folder" size={14} /> {selectedFile ? "Ganti File…" : "Pilih File…"}
              </button>
              <span className="file-pick-name">{selectedFile ? selectedFile.name : "Belum ada file dipilih"}</span>
            </div>
          </div>

          <label className="form-field">
            <span>PIN Pembuka Backup</span>
            <input
              type="password"
              placeholder="Masukkan PIN saat file ini diexport"
              value={importPin}
              onChange={(e) => setImportPin(e.target.value)}
              disabled={importing}
              required
            />
          </label>

          {importError && <p className="form-error">{importError}</p>}
          {importStats && (
            <div className="form-success">
              <p><strong>✓ Berhasil Mengimpor & Menggabungkan Konfigurasi!</strong></p>
              <ul>
                <li>Server Baru: {importStats.servers_added}</li>
                <li>Server Diperbarui: {importStats.servers_updated}</li>
                <li>Path Bookmark Sidebar: {importStats.bookmarks_added}</li>
                <li>Snippet Baru: {importStats.snippets_added}</li>
              </ul>
            </div>
          )}

          <div className="backup-actions">
            <button
              type="submit"
              className="btn btn-primary"
              disabled={importing || !selectedFile || !importPin}
            >
              {importing ? "Mendekripsi & Menggabungkan…" : "Import & Gabungkan"}
            </button>
          </div>
        </form>
      )}
    </Modal>
  );
}
