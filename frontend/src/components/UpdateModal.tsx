import { useEffect, useRef, useState } from "react";
import { api, errorMessage, openExternal } from "../api";
import type { UpdateCheckResult, UpdateProgressResult } from "../types";
import { getUpdateBadge, getUpdateStatusText } from "../updateLogic";
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

  const [applying, setApplying] = useState(false);
  const [progress, setProgress] = useState<UpdateProgressResult | null>(null);
  const [applyError, setApplyError] = useState<string | null>(null);
  const pollTimerRef = useRef<ReturnType<typeof setInterval> | null>(null);

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
      if (pollTimerRef.current) clearInterval(pollTimerRef.current);
    };
  }, []);

  const handleApplyUpdate = async () => {
    setApplying(true);
    setApplyError(null);
    setProgress({
      state: "downloading",
      progress_percent: 0,
      downloaded_bytes: 0,
      total_bytes: 0,
      message: "Menghubungi server rilis GitHub...",
    });

    try {
      await api.applyUpdate();

      pollTimerRef.current = setInterval(async () => {
        try {
          const prog = await api.getUpdateProgress();
          setProgress(prog);

          if (prog.state === "completed") {
            if (pollTimerRef.current) clearInterval(pollTimerRef.current);
            setTimeout(() => {
              window.location.reload();
            }, 3000);
          } else if (prog.state === "error") {
            if (pollTimerRef.current) clearInterval(pollTimerRef.current);
            setApplying(false);
            setApplyError(prog.message || "Pembaruan gagal dipasang.");
          }
        } catch {
          // Koneksi mungkin terputus sejenak saat backend me-restart proses baru
        }
      }, 700);
    } catch (err) {
      setApplying(false);
      setApplyError(errorMessage(err));
    }
  };

  const badge = checkResult ? getUpdateBadge(checkResult) : null;
  const releaseUrl = checkResult?.download_url || "https://github.com/kotajakarta/rusterm/releases";

  return (
    <Modal title="Pembaruan RusTerm" onClose={applying && progress?.state !== "completed" ? () => {} : onClose} wide>
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

            {applying && progress && (
              <div
                style={{
                  padding: "14px 16px",
                  background: "var(--surface)",
                  border: "1px solid var(--border)",
                  borderRadius: 8,
                  display: "flex",
                  flexDirection: "column",
                  gap: 10,
                }}
              >
                <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                  <div style={{ fontWeight: 600, fontSize: 13, display: "flex", alignItems: "center", gap: 8 }}>
                    <span className={progress.state === "completed" ? "" : "spin"}>
                      <Icon name={progress.state === "completed" ? "check" : "refresh"} size={14} />
                    </span>
                    <span>{getUpdateStatusText(progress)}</span>
                  </div>
                  <span style={{ fontSize: 12, fontWeight: 600, color: "var(--primary)" }}>
                    {progress.progress_percent}%
                  </span>
                </div>

                <div
                  style={{
                    width: "100%",
                    height: 8,
                    background: "rgba(255, 255, 255, 0.08)",
                    borderRadius: 4,
                    overflow: "hidden",
                  }}
                >
                  <div
                    style={{
                      width: `${progress.progress_percent}%`,
                      height: "100%",
                      backgroundColor:
                        progress.state === "completed" ? "var(--success, #22c55e)" : "var(--primary, #3b82f6)",
                      transition: "width 0.3s ease",
                    }}
                  />
                </div>

                {progress.state === "completed" && (
                  <div style={{ fontSize: 12, color: "var(--success, #22c55e)", fontWeight: 500 }}>
                    Pembaruan berhasil dipasang! Aplikasi baru sedang diluncurkan dan halaman akan dimuat ulang...
                  </div>
                )}
              </div>
            )}

            {applyError && (
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
                {applyError}
              </div>
            )}

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
              Fitur <strong>Perbarui Sekarang (In-App)</strong> akan mengunduh versi terbaru di latar belakang, mematikan proses background lama, menimpa berkas <code>rusterm.exe</code>, dan me-restart aplikasi secara otomatis.
            </div>

            <div style={{ display: "flex", justifyContent: "flex-end", gap: 10, marginTop: 8 }}>
              <button
                className="btn"
                type="button"
                onClick={onClose}
                disabled={applying && progress?.state !== "completed"}
              >
                {progress?.state === "completed" ? "Tutup" : "Batal"}
              </button>
              {!applying && (
                <button className="btn" type="button" onClick={performCheck} disabled={checking}>
                  Periksa Ulang
                </button>
              )}
              {checkResult.has_update && !applying && (
                <button
                  className="btn btn-primary"
                  type="button"
                  style={{ backgroundColor: "#22c55e", borderColor: "#16a34a" }}
                  onClick={handleApplyUpdate}
                >
                  <Icon name="download" size={13} /> Perbarui Sekarang (In-App)
                </button>
              )}
              <button
                className="btn"
                type="button"
                onClick={() => openExternal(releaseUrl)}
                disabled={applying}
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
