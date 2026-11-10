import { useEffect, useRef, useState } from "react";
import { api, errorMessage } from "../api";
import type { UpdateCheckResult, UpdateProgressResult } from "../types";
import { formatUpdateBytes, getUpdateBadge } from "../updateLogic";
import { Icon } from "./Icon";
import { Modal } from "./Modal";

export { formatUpdateBytes, getUpdateBadge };

interface UpdateModalProps {
  onClose: () => void;
}

export function UpdateModal({ onClose }: UpdateModalProps) {
  const [checking, setChecking] = useState(true);
  const [checkResult, setCheckResult] = useState<UpdateCheckResult | null>(null);
  const [checkError, setCheckError] = useState<string | null>(null);

  const [applying, setApplying] = useState(false);
  const [applyMessage, setApplyMessage] = useState<string | null>(null);
  const [progress, setProgress] = useState<UpdateProgressResult | null>(null);

  const pollIntervalRef = useRef<number | null>(null);

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
    return () => {
      if (pollIntervalRef.current !== null) {
        window.clearInterval(pollIntervalRef.current);
      }
    };
  }, []);

  const startProgressPolling = () => {
    if (pollIntervalRef.current !== null) {
      window.clearInterval(pollIntervalRef.current);
    }
    pollIntervalRef.current = window.setInterval(async () => {
      try {
        const prog = await api.getUpdateProgress();
        setProgress(prog);
        if (prog.state === "completed" || prog.state === "ready" || prog.state === "error") {
          if (pollIntervalRef.current !== null) {
            window.clearInterval(pollIntervalRef.current);
            pollIntervalRef.current = null;
          }
          setApplying(false);
          if (prog.state === "error") {
            setCheckError(prog.message || "Gagal mengunduh atau menerapkan pembaruan.");
          }
        }
      } catch {
        // network jitter during pull/restart
      }
    }, 1000);
  };

  const handleApply = async () => {
    setApplying(true);
    setCheckError(null);
    setApplyMessage(null);
    try {
      const res = await api.applyUpdate();
      if (!res.started) {
        setCheckError(res.message);
        setApplying(false);
        return;
      }
      setApplyMessage(res.message);
      startProgressPolling();
    } catch (err) {
      setCheckError(errorMessage(err));
      setApplying(false);
    }
  };

  const badge = checkResult ? getUpdateBadge(checkResult) : null;

  return (
    <Modal title="Pembaruan TermDeck" onClose={onClose} wide>
      <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
        {checking && (
          <div style={{ display: "flex", alignItems: "center", gap: 8, padding: "16px 0", color: "var(--muted)" }}>
            <span className="spin">
              <Icon name="refresh" size={16} />
            </span>
            <span>Memeriksa pembaruan dari GitHub...</span>
          </div>
        )}

        {checkError && (
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
                  Platform: {checkResult.platform === "android" ? "Android" : "Desktop (Linux/WSL)"}
                </div>
                <div style={{ display: "flex", alignItems: "center", gap: 10, marginTop: 4 }}>
                  <span style={{ fontSize: 14, fontWeight: 500 }}>
                    Versi Saat Ini: <code style={{ color: "var(--primary)" }}>{checkResult.current_version}</code>
                  </span>
                  <span>&rarr;</span>
                  <span style={{ fontSize: 14, fontWeight: 500 }}>
                    Versi Terbaru: <code style={{ color: "var(--accent)" }}>{checkResult.latest_version}</code>
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
                      maxHeight: 160,
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

            {applying && (
              <div style={{ display: "flex", flexDirection: "column", gap: 8, marginTop: 4 }}>
                <div style={{ display: "flex", justifyContent: "space-between", fontSize: 12, color: "var(--muted)" }}>
                  <span>{progress?.message || applyMessage || "Memproses pembaruan..."}</span>
                  {progress && progress.total_bytes > 0 && (
                    <span>
                      {formatUpdateBytes(progress.downloaded_bytes)} / {formatUpdateBytes(progress.total_bytes)}
                    </span>
                  )}
                </div>
                <div
                  style={{
                    width: "100%",
                    height: 8,
                    background: "var(--surface)",
                    borderRadius: 4,
                    overflow: "hidden",
                    border: "1px solid var(--border)",
                  }}
                >
                  <div
                    style={{
                      width: `${progress?.progress_percent ?? (checkResult.platform === "desktop" ? 50 : 20)}%`,
                      height: "100%",
                      background: "var(--accent, #3b82f6)",
                      transition: "width 0.3s ease",
                    }}
                  />
                </div>
              </div>
            )}

            {!applying && progress && progress.state === "ready" && (
              <div
                style={{
                  padding: "10px 14px",
                  background: "rgba(59, 130, 246, 0.1)",
                  border: "1px solid var(--accent, #3b82f6)",
                  borderRadius: 6,
                  color: "var(--accent, #3b82f6)",
                  fontSize: 13,
                }}
              >
                {progress.message}
              </div>
            )}

            {!applying && progress && progress.state === "completed" && (
              <div
                style={{
                  padding: "10px 14px",
                  background: "rgba(34, 197, 94, 0.1)",
                  border: "1px solid var(--success, #22c55e)",
                  borderRadius: 6,
                  color: "var(--success, #22c55e)",
                  fontSize: 13,
                }}
              >
                {progress.message}
              </div>
            )}

            <div style={{ display: "flex", justifyContent: "flex-end", gap: 10, marginTop: 8 }}>
              <button className="btn" type="button" onClick={onClose} disabled={applying}>
                Tutup
              </button>
              <button className="btn" type="button" onClick={performCheck} disabled={checking || applying}>
                Periksa Ulang
              </button>
              {progress?.state === "ready" ? (
                <button
                  className="btn btn-primary"
                  type="button"
                  onClick={handleApply}
                  disabled={applying}
                >
                  Pasang APK Sekarang
                </button>
              ) : (
                checkResult.has_update && (
                  <button
                    className="btn btn-primary"
                    type="button"
                    onClick={handleApply}
                    disabled={applying || progress?.state === "completed"}
                  >
                    {applying ? "Memperbarui..." : "Perbarui Sekarang"}
                  </button>
                )
              )}
            </div>
          </>
        )}
      </div>
    </Modal>
  );
}
