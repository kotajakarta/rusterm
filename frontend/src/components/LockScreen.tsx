import { useEffect, useRef, useState, type FormEvent } from "react";

import { api, errorMessage } from "../api";
import { Icon } from "./Icon";

export function LockScreen({ onUnlocked }: { onUnlocked: (mustChangePin?: boolean) => void }) {
  const [pin, setPin] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (!pin) return;
    setBusy(true);
    setError(null);
    try {
      const res = await api.login(pin);
      onUnlocked(res?.must_change_pin);
    } catch (err) {
      setError(errorMessage(err));
      setPin("");
      setBusy(false);
      requestAnimationFrame(() => inputRef.current?.focus());
    }
  };

  return (
    <div className="lock-screen">
      <form className="lock-card" onSubmit={submit}>
        <span className="lock-mark" aria-hidden="true">
          <Icon name="lock" size={22} />
        </span>
        <h1>RusTerm terkunci</h1>
        <p>Masukkan PIN untuk membuka data server Anda.</p>
        <label className="sr-only" htmlFor="pin">
          PIN
        </label>
        <input
          id="pin"
          ref={inputRef}
          className={`pin-input${error ? " has-error" : ""}`}
          type="password"
          inputMode="numeric"
          autoComplete="current-password"
          maxLength={12}
          value={pin}
          onChange={(e) => setPin(e.target.value.replace(/\D/g, ""))}
          disabled={busy}
          aria-invalid={error !== null}
          aria-describedby={error ? "pin-error" : undefined}
        />
        {error && (
          <p id="pin-error" className="lock-error" role="alert">
            {error}
          </p>
        )}
        <button type="submit" className="btn btn-primary btn-block" disabled={busy || pin.length < 4}>
          {busy ? "Membuka…" : "Buka"}
        </button>
      </form>
    </div>
  );
}
