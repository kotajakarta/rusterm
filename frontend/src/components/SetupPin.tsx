import { useEffect, useRef, useState, type FormEvent } from "react";

import { api, errorMessage } from "../api";
import { Icon } from "./Icon";

export function SetupPin({ onCreated }: { onCreated: () => void }) {
  const [pin, setPin] = useState("");
  const [confirmPin, setConfirmPin] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (pin.length < 4) {
      setError("PIN minimal 4 digit angka.");
      return;
    }
    if (pin === "7821724") {
      setError("PIN tidak boleh menggunakan PIN default lama (7821724).");
      return;
    }
    if (pin !== confirmPin) {
      setError("Konfirmasi PIN tidak cocok.");
      return;
    }

    setBusy(true);
    setError(null);
    try {
      await api.setupPin(pin);
      onCreated();
    } catch (err) {
      setError(errorMessage(err));
      setBusy(false);
      requestAnimationFrame(() => inputRef.current?.focus());
    }
  };

  return (
    <div className="lock-screen">
      <form className="lock-card" onSubmit={submit}>
        <span className="lock-mark" aria-hidden="true">
          <Icon name="key" size={22} />
        </span>
        <h1>Buat PIN Baru</h1>
        <p>Amankan data server dan kredensial Anda dengan PIN 4-12 digit angka.</p>

        <label style={{ fontSize: "12px", color: "var(--muted)", display: "block", textAlign: "left", marginBottom: "4px" }} htmlFor="setup-pin">
          PIN Baru
        </label>
        <input
          id="setup-pin"
          ref={inputRef}
          className={`pin-input${error ? " has-error" : ""}`}
          type="password"
          inputMode="numeric"
          autoComplete="new-password"
          maxLength={12}
          placeholder="4 - 12 digit"
          value={pin}
          onChange={(e) => setPin(e.target.value.replace(/\D/g, ""))}
          disabled={busy}
          aria-invalid={error !== null}
        />

        <label style={{ fontSize: "12px", color: "var(--muted)", display: "block", textAlign: "left", marginTop: "12px", marginBottom: "4px" }} htmlFor="setup-confirm-pin">
          Konfirmasi PIN
        </label>
        <input
          id="setup-confirm-pin"
          className={`pin-input${error ? " has-error" : ""}`}
          type="password"
          inputMode="numeric"
          autoComplete="new-password"
          maxLength={12}
          placeholder="Ulangi PIN di atas"
          value={confirmPin}
          onChange={(e) => setConfirmPin(e.target.value.replace(/\D/g, ""))}
          disabled={busy}
          aria-invalid={error !== null}
          aria-describedby={error ? "setup-pin-error" : undefined}
        />

        {error && (
          <p id="setup-pin-error" className="lock-error" role="alert">
            {error}
          </p>
        )}

        <button
          type="submit"
          className="btn btn-primary btn-block"
          style={{ marginTop: "16px" }}
          disabled={busy || pin.length < 4 || confirmPin.length < 4}
        >
          {busy ? "Menyimpan…" : "Buat PIN & Buka TermDeck"}
        </button>
      </form>
    </div>
  );
}
