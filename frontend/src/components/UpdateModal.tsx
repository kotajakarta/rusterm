import { useEffect, useState } from "react";
import { api, errorMessage, openExternal } from "../api";
import type { UpdateCheckResult } from "../types";
import { getUpdateBadge } from "../updateLogic";
import { Icon } from "./Icon";
import { Modal } from "./Modal";

export { getUpdateBadge };

interface UpdateModalProps {
  onClose: () => void;
}

export function UpdateModal({ onClose }: UpdateModalProps) {
  const [checking, setChecking] = useState(true);
  const [checkResult, setCheckResult] = useState<UpdateCheckResult | null>(null);
  const [checkError, setCheckError] = useState<string | null>(null);

  const performCheck = async () => {
    setChecking(true);
    setCheckError(null);
    try {
      const res = await api.checkUpdate();
      setCheckResult(res);
    } catch (err) {
      setCheckError(errorMessage(err));
    } finally {
      setChecking(false);
    }
  };

  useEffect(() => {
    performCheck();
  }, []);

  const badge = checkResult ? getUpdateBadge(checkResult) : null;
  const releaseUrl = checkResult?.download_url || "https://github.com/kotajakarta/rusterm/releases";

  return (
    <Modal title="Pembaruan RusTerm" onClose={onClose} wide>
      <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
        {checking && (
          <div style={{ display: "flex", alignItems: "center", gap: 8, padding: "16px 0", color: "var(--muted)" }}>
            <span className="spin">
              <Icon name="refresh" size={16} />
            </span>
            <span>Memeriksa pembaruan dari GitHub (kotajakarta/rusterm)...</span>
          </div>
        )}

        {checkError && (
          <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
            <div
              style={{
                padding: "10px 14px",
                background: "rgba(239, 68, 68, 0.1)",
                border: "1px solid var(--danger)",
                borderRadius: 6,
                color: "var(--danger)",
                fontSize: 13,
              }}
            >
              {checkError}
            </div>
            <div style={{ display: "flex", justifyContent: "flex-end", gap: 10 }}>
              <button className="btn" type="button" onClick={onClose}>
                Tutup
              </button>
              <button className="btn" type="button" onClick={performCheck}>
                Coba Lagi
              </button>
              <button
                className="btn btn-primary"
                type="button"
                onClick={() => openExternal(releaseUrl)}
              >
                Buka Rilis di GitHub
              </button>
            </div>
          </div>
        )}

        {!checking && checkResult && (
          <>
            <div
              style={{
                display: "flex",
                alignItems: "center",
                justifyContent: "space-between",
                padding: "12px 16px",
                background: "var(--surface)",
                border: "1px solid var(--border)",
                borderRadius: 8,
              }}
            >
              <div>
                <div style={{ fontSize: 12, color: "var(--muted)", textTransform: "uppercase", letterSpacing: "0.5px" }}>
                  Platform: Desktop (Windows / Linux) &bull; GitHub: kotajakarta/rusterm
                </div>
                <div style={{ display: "flex", alignItems: "center", gap: 10, marginTop: 4 }}>
                  <span style={{ fontSize: 14, fontWeight: 500 }}>
                    Versi Terpasang: <code style={{ color: "var(--primary)" }}>v{checkResult.current_version}</code>
                  </span>
                  <span>&rarr;</span>
                  <span style={{ fontSize: 14, fontWeight: 500 }}>
                    Versi Rilis: <code style={{ color: "var(--accent)" }}>{checkResult.latest_version.startsWith("v") ? checkResult.latest_version : `v${checkResult.latest_version}`}</code>
                  </span>
                </div>
              </div>
              {badge && (
                <span
                  style={{
                    padding: "4px 8px",
                    borderRadius: 4,
                    fontSize: 12,
                    fontWeight: 600,
                    background:
                      badge.type === "warning" ? "rgba(245, 158, 11, 0.15)" : "rgba(34, 197, 94, 0.15)",
                    color: badge.type === "warning" ? "var(--warning, #f59e0b)" : "var(--success, #22c55e)",
                    border: `1px solid ${
                      badge.type === "warning" ? "var(--warning, #f59e0b)" : "var(--success, #22c55e)"
                    }`,
                  }}
                >
                  {badge.label}
                </span>
              )}
            </div>

            {checkResult.release_title && (
              <div>
                <div style={{ fontSize: 13, fontWeight: 600, marginBottom: 6 }}>
                  {checkResult.release_title}
                </div>
                {checkResult.release_notes && (
                  <pre
                    style={{
                      maxHeight: 180,
                      overflowY: "auto",
                      padding: 12,
                      background: "var(--surface)",
                      border: "1px solid var(--border)",
                      borderRadius: 6,
                      fontSize: 12,
                      fontFamily: "var(--font-mono, monospace)",
                      whiteSpace: "pre-wrap",
                      wordBreak: "break-word",
                      color: "var(--fg)",
                    }}
                  >
                    {checkResult.release_notes}
                  </pre>
                )}
              </div>
            )}

            <div
              style={{
                fontSize: 12,
                color: "var(--muted)",
                background: "var(--surface)",
                padding: "10px 14px",
                borderRadius: 6,
                border: "1px solid var(--border)",
              }}
            >
              Berkas executable mandiri Windows (<code>termdeck.exe</code> / <code>rusterm.exe</code>) dapat diunduh langsung melalui halaman GitHub Releases.
            </div>

            <div style={{ display: "flex", justifyContent: "flex-end", gap: 10, marginTop: 8 }}>
              <button className="btn" type="button" onClick={onClose}>
                Tutup
              </button>
              <button className="btn" type="button" onClick={performCheck} disabled={checking}>
                Periksa Ulang
              </button>
              <button
                className="btn btn-primary"
                type="button"
                onClick={() => openExternal(releaseUrl)}
              >
                Buka Halaman Rilis GitHub
              </button>
            </div>
          </>
        )}
      </div>
    </Modal>
  );
}
