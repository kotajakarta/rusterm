import { useState, type FormEvent } from "react";

import { api, errorMessage } from "../api";
import {
  formatIdleLockLabel,
  IDLE_LOCK_OPTIONS,
  normalizeIdleLockMinutes,
  type IdleLockMinutes,
} from "../idleLock";
import { Modal } from "./Modal";

const PIN_PATTERN = /^\d{4,12}$/;

interface ChangePinProps {
  onClose: () => void;
  idleLockMinutes?: IdleLockMinutes;
  onChangeIdleLockMinutes?: (minutes: IdleLockMinutes) => void;
}

export function ChangePin({ onClose, idleLockMinutes = 30, onChangeIdleLockMinutes }: ChangePinProps) {
  const [oldPin, setOldPin] = useState("");
  const [newPin, setNewPin] = useState("");
  const [confirmPin, setConfirmPin] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [done, setDone] = useState(false);

  const digits = (value: string) => value.replace(/\D/g, "");

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (!PIN_PATTERN.test(newPin)) {
      setError("PIN baru harus 4-12 digit angka.");
      return;
    }
    if (newPin !== confirmPin) {
      setError("Konfirmasi PIN baru tidak sama.");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await api.changePin(oldPin, newPin);
      setDone(true);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal
      title="Ganti PIN"
      onClose={onClose}
      footer={
        done ? (
          <>
            <span className="spacer" />
            <button type="button" className="btn btn-primary" onClick={onClose}>
              Selesai
            </button>
          </>
        ) : (
          <>
            <span className="spacer" />
            <button type="button" className="btn" onClick={onClose}>
              Batal
            </button>
            <button type="submit" form="pin-form" className="btn btn-primary" disabled={busy || !oldPin || !newPin}>
              {busy ? "Mengenkripsi ulang…" : "Ganti PIN"}
            </button>
          </>
        )
      }
    >
      {done ? (
        <p className="notice ok">PIN diganti. Database sudah dienkripsi ulang dengan PIN baru.</p>
      ) : (
        <form id="pin-form" className="form" onSubmit={submit}>
          {onChangeIdleLockMinutes && (
            <label className="field">
              <span>Kunci otomatis saat tidak aktif</span>
              <select
                value={idleLockMinutes}
                onChange={(e) => onChangeIdleLockMinutes(normalizeIdleLockMinutes(e.target.value))}
              >
                {IDLE_LOCK_OPTIONS.map((opt) => (
                  <option key={opt} value={opt}>
                    {formatIdleLockLabel(opt)}
                  </option>
                ))}
              </select>
            </label>
          )}
          <label className="field">
            <span>PIN sekarang</span>
            <input
              type="password"
              inputMode="numeric"
              autoComplete="current-password"
              maxLength={12}
              value={oldPin}
              onChange={(e) => setOldPin(digits(e.target.value))}
            />
          </label>
          <label className="field">
            <span>PIN baru (4-12 digit)</span>
            <input
              type="password"
              inputMode="numeric"
              autoComplete="new-password"
              maxLength={12}
              value={newPin}
              onChange={(e) => setNewPin(digits(e.target.value))}
            />
          </label>
          <label className="field">
            <span>Ulangi PIN baru</span>
            <input
              type="password"
              inputMode="numeric"
              autoComplete="new-password"
              maxLength={12}
              value={confirmPin}
              onChange={(e) => setConfirmPin(digits(e.target.value))}
            />
          </label>
          {error && <p className="notice bad">{error}</p>}
        </form>
      )}
    </Modal>
  );
}
