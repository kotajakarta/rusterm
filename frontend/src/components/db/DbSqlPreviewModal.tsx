import { useState } from "react";
import { Modal } from "../Modal";

interface DbSqlPreviewModalProps {
  title: string;
  description: string;
  sqlPreview: string;
  confirmLabel: string;
  danger?: boolean;
  /** Jika diisi, user wajib mengetik string ini sebelum tombol konfirmasi aktif. */
  requireTypeText?: string;
  busy?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

export function DbSqlPreviewModal({
  title,
  description,
  sqlPreview,
  confirmLabel,
  danger = false,
  requireTypeText,
  busy = false,
  onConfirm,
  onCancel,
}: DbSqlPreviewModalProps): JSX.Element {
  const [typed, setTyped] = useState("");
  const canConfirm = !busy && (!requireTypeText || typed.trim() === requireTypeText);

  return (
    <Modal
      title={title}
      onClose={onCancel}
      footer={
        <>
          <button type="button" className="btn" onClick={onCancel} disabled={busy}>
            Batal
          </button>
          <button
            type="button"
            className={`btn ${danger ? "btn-danger" : "btn-primary"}`}
            onClick={onConfirm}
            disabled={!canConfirm}
          >
            {busy ? "Memproses..." : confirmLabel}
          </button>
        </>
      }
    >
      <div className="db-form">
        <p className="confirm-text">{description}</p>
        <pre className="db-sql-preview">{sqlPreview}</pre>
        {requireTypeText && (
          <label className="field">
            <span>
              Ketik <strong>{requireTypeText}</strong> untuk konfirmasi:
            </span>
            <input
              type="text"
              value={typed}
              onChange={(e) => setTyped(e.target.value)}
              placeholder={requireTypeText}
            />
          </label>
        )}
      </div>
    </Modal>
  );
}
