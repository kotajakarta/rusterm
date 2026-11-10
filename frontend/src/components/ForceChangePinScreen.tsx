import { useEffect, useRef, useState, type FormEvent } from "react";

import { api, errorMessage } from "../api";
import { Icon } from "./Icon";

export function ForceChangePinScreen({ onUpdated }: { onUpdated: () => void }) {
  const [newPin, setNewPin] = useState("");
  const [confirmPin, setConfirmPin] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (newPin.length < 4) {
      setError("PIN baru minimal 4 digit angka.");
      return;
    }
    if (newPin === "7821724") {
      setError("PIN tidak boleh menggunakan PIN default lama (7821724).");
      return;
    }
    if (newPin !== confirmPin) {
      setError("Konfirmasi PIN baru tidak cocok.");
      return;
    }

    setBusy(true);
    setError(null);
    try {
      await api.changePin("7821724", newPin);
      onUpdated();
    } catch (err) {
      setError(errorMessage(err));
      setBusy(false);
      requestAnimationFrame(() => inputRef.current?.focus());
    }
  };

  return (
    <div className="lock-screen">
      <form className="lock-card" onSubmit={submit}>
        <span className="lock-mark" aria-hidden="true" style={{ background: "rgba(240, 122, 111, 0.15)", color: "var(--red)" }}>
          <Icon name="key" size={22} />
        </span>
        <h1>Wajib Ganti PIN</h1>
        <p>
          Anda masih menggunakan PIN default bawaan versi lama (<code>7821724</code>). Demi keamanan, Anda wajib membuat PIN baru sebelum dapat mengakses server dan data Anda.
        </p>

        <label style={{ fontSize: "12px", color: "var(--muted)", display: "block", textAlign: "left", marginBottom: "4px" }} htmlFor="force-new-pin">
          PIN Baru
        </label>
        <input
          id="force-new-pin"
          ref={inputRef}
          className={`pin-input${error ? " has-error" : ""}`}
          type="password"
          inputMode="numeric"
          autoComplete="new-password"
          maxLength={12}
          placeholder="4 - 12 digit"
          value={newPin}
          onChange={(e) => setNewPin(e.target.value.replace(/\D/g, ""))}
          disabled={busy}
          aria-invalid={error !== null}
        />

        <label style={{ fontSize: "12px", color: "var(--muted)", display: "block", textAlign: "left", marginTop: "12px", marginBottom: "4px" }} htmlFor="force-confirm-pin">
          Konfirmasi PIN Baru
        </label>
        <input
          id="force-confirm-pin"
          className={`pin-input${error ? " has-error" : ""}`}
          type="password"
          inputMode="numeric"
          autoComplete="new-password"
          maxLength={12}
          placeholder="Ulangi PIN baru di atas"
          value={confirmPin}
          onChange={(e) => setConfirmPin(e.target.value.replace(/\D/g, ""))}
          disabled={busy}
          aria-invalid={error !== null}
          aria-describedby={error ? "force-pin-error" : undefined}
        />

        {error && (
          <p id="force-pin-error" className="lock-error" role="alert">
            {error}
          </p>
        )}

        <button
          type="submit"
          className="btn btn-primary btn-block"
          style={{ marginTop: "16px" }}
          disabled={busy || newPin.length < 4 || confirmPin.length < 4}
        >
          {busy ? "Menyimpan PIN Baru…" : "Simpan & Lanjutkan"}
        </button>
      </form>
    </div>
  );
}
