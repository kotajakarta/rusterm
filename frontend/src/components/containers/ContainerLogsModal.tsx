import { useEffect, useRef, useState } from "react";
import { api, errorMessage } from "../../api";
import { Icon } from "../Icon";
import { Modal } from "../Modal";

interface ContainerLogsModalProps {
  serverId: number;
  containerId: string;
  containerName: string;
  onClose: () => void;
}

export function ContainerLogsModal({
  serverId,
  containerId,
  containerName,
  onClose,
}: ContainerLogsModalProps) {
  const [logs, setLogs] = useState<string>("");
  const [loading, setLoading] = useState<boolean>(true);
  const [tail, setTail] = useState<number>(150);
  const [autoRefresh, setAutoRefresh] = useState<boolean>(true);
  const [copied, setCopied] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);
  const logPreRef = useRef<HTMLPreElement>(null);

  const fetchLogs = async () => {
    try {
      setLoading(true);
      setError(null);
      const res = await api.containersLogs(serverId, containerId, tail);
      setLogs(res.logs || "(Tidak ada log output)");
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    void fetchLogs();
  }, [serverId, containerId, tail]);

  useEffect(() => {
    if (!autoRefresh) return;
    const interval = setInterval(() => {
      void api
        .containersLogs(serverId, containerId, tail)
        .then((res) => {
          setLogs(res.logs || "(Tidak ada log output)");
        })
        .catch(() => {});
    }, 4000);
    return () => clearInterval(interval);
  }, [autoRefresh, serverId, containerId, tail]);

  useEffect(() => {
    if (logPreRef.current) {
      logPreRef.current.scrollTop = logPreRef.current.scrollHeight;
    }
  }, [logs]);

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(logs);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // Fallback
    }
  };

  return (
    <Modal
      title={`Logs: ${containerName} (${containerId})`}
      onClose={onClose}
      wide
      footer={
        <div style={{ display: "flex", justifyContent: "space-between", width: "100%", alignItems: "center" }}>
          <div style={{ display: "flex", gap: "10px", alignItems: "center" }}>
            <label style={{ fontSize: "12px", color: "var(--muted)", display: "flex", alignItems: "center", gap: "6px" }}>
              <input
                type="checkbox"
                checked={autoRefresh}
                onChange={(e) => setAutoRefresh(e.target.checked)}
              />
              Auto-refresh (4s)
            </label>
            <select
              value={tail}
              onChange={(e) => setTail(Number(e.target.value))}
              style={{
                background: "var(--bg-card)",
                color: "var(--text)",
                border: "1px solid var(--line)",
                borderRadius: "var(--r-sm)",
                padding: "2px 6px",
                fontSize: "12px",
              }}
            >
              <option value={50}>50 baris</option>
              <option value={150}>150 baris</option>
              <option value={500}>500 baris</option>
              <option value={1000}>1000 baris</option>
            </select>
          </div>
          <div style={{ display: "flex", gap: "8px" }}>
            <button
              type="button"
              className="btn btn-subtle"
              onClick={handleCopy}
              title="Salin log"
              style={{ padding: "4px 10px", fontSize: "12px" }}
            >
              <Icon name="clipboard" size={13} /> {copied ? "Tersalin!" : "Salin"}
            </button>
            <button
              type="button"
              className="btn btn-subtle"
              onClick={() => void fetchLogs()}
              disabled={loading}
              title="Refresh sekarang"
              style={{ padding: "4px 10px", fontSize: "12px" }}
            >
              <Icon name="refresh" size={13} className={loading ? "spin" : ""} /> Refresh
            </button>
            <button
              type="button"
              className="btn btn-primary"
              onClick={onClose}
              style={{ padding: "4px 14px", fontSize: "12px" }}
            >
              Tutup
            </button>
          </div>
        </div>
      }
    >
      {error && (
        <div style={{ padding: "8px 12px", background: "rgba(240, 122, 111, 0.15)", color: "var(--red)", borderRadius: "var(--r-sm)", marginBottom: "10px", fontSize: "13px" }}>
          {error}
        </div>
      )}
      <pre
        ref={logPreRef}
        style={{
          margin: 0,
          background: "#0c1211",
          color: "#c8d4cf",
          fontFamily: "var(--mono)",
          fontSize: "12px",
          lineHeight: "1.45",
          padding: "12px",
          borderRadius: "var(--r-md)",
          maxHeight: "60vh",
          overflowY: "auto",
          whiteSpace: "pre-wrap",
          wordBreak: "break-all",
        }}
      >
        {logs}
      </pre>
    </Modal>
  );
}
