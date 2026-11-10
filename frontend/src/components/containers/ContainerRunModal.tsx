import { useState } from "react";
import { api, errorMessage } from "../../api";
import { Modal } from "../Modal";

interface ContainerRunModalProps {
  serverId: number;
  initialImage?: string;
  onClose: () => void;
  onSuccess: (containerId: string) => void;
}

export function ContainerRunModal({
  serverId,
  initialImage = "",
  onClose,
  onSuccess,
}: ContainerRunModalProps) {
  const [image, setImage] = useState<string>(initialImage);
  const [name, setName] = useState<string>("");
  const [ports, setPorts] = useState<string>("");
  const [env, setEnv] = useState<string>("");
  const [volumes, setVolumes] = useState<string>("");
  const [command, setCommand] = useState<string>("");
  const [restart, setRestart] = useState<string>("no");
  const [loading, setLoading] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!image.trim()) {
      setError("Nama image wajib diisi");
      return;
    }

    setLoading(true);
    setError(null);
    try {
      const parsedPorts = ports
        .split("\n")
        .map((p) => p.trim())
        .filter(Boolean);
      const parsedEnv = env
        .split("\n")
        .map((e) => e.trim())
        .filter(Boolean);
      const parsedVolumes = volumes
        .split("\n")
        .map((v) => v.trim())
        .filter(Boolean);

      const res = await api.containersRun({
        server_id: serverId,
        image: image.trim(),
        name: name.trim() || undefined,
        ports: parsedPorts,
        env: parsedEnv,
        volumes: parsedVolumes,
        command: command.trim() || undefined,
        restart: restart !== "no" ? restart : undefined,
      });

      onSuccess(res.id || "");
      onClose();
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setLoading(false);
    }
  };

  return (
    <Modal
      title="Jalankan Kontainer Baru"
      onClose={onClose}
      wide
      footer={
        <div style={{ display: "flex", justifyContent: "flex-end", gap: "8px", width: "100%" }}>
          <button type="button" className="btn btn-subtle" onClick={onClose} disabled={loading}>
            Batal
          </button>
          <button
            type="button"
            className="btn btn-primary"
            onClick={handleSubmit}
            disabled={loading || !image.trim()}
          >
            {loading ? "Menjalankan..." : "Jalankan Kontainer"}
          </button>
        </div>
      }
    >
      <form onSubmit={handleSubmit} style={{ display: "flex", flexDirection: "column", gap: "12px" }}>
        {error && (
          <div
            style={{
              padding: "8px 12px",
              background: "rgba(240, 122, 111, 0.15)",
              color: "var(--red)",
              borderRadius: "var(--r-sm)",
              fontSize: "13px",
            }}
          >
            {error}
          </div>
        )}

        <div>
          <label style={{ display: "block", fontSize: "12px", color: "var(--muted)", marginBottom: "4px" }}>
            Image Container <span style={{ color: "var(--red)" }}>*</span>
          </label>
          <input
            type="text"
            className="input"
            value={image}
            onChange={(e) => setImage(e.target.value)}
            placeholder="misal: docker.io/library/nginx:alpine atau quay.io/podman/hello"
            required
            autoFocus
          />
        </div>

        <div>
          <label style={{ display: "block", fontSize: "12px", color: "var(--muted)", marginBottom: "4px" }}>
            Nama Kontainer (Opsional)
          </label>
          <input
            type="text"
            className="input"
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="misal: web-production"
          />
        </div>

        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "12px" }}>
          <div>
            <label style={{ display: "block", fontSize: "12px", color: "var(--muted)", marginBottom: "4px" }}>
              Port Mappings (Satu per baris)
            </label>
            <textarea
              className="input"
              rows={3}
              value={ports}
              onChange={(e) => setPorts(e.target.value)}
              placeholder="8080:80&#10;8443:443"
              style={{ resize: "vertical", fontFamily: "var(--mono)", fontSize: "12px" }}
            />
          </div>

          <div>
            <label style={{ display: "block", fontSize: "12px", color: "var(--muted)", marginBottom: "4px" }}>
              Volumes / Mounts (Satu per baris)
            </label>
            <textarea
              className="input"
              rows={3}
              value={volumes}
              onChange={(e) => setVolumes(e.target.value)}
              placeholder="/host/path:/container/path&#10;/data:/app/data:Z"
              style={{ resize: "vertical", fontFamily: "var(--mono)", fontSize: "12px" }}
            />
          </div>
        </div>

        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "12px" }}>
          <div>
            <label style={{ display: "block", fontSize: "12px", color: "var(--muted)", marginBottom: "4px" }}>
              Environment Variables (KEY=VAL per baris)
            </label>
            <textarea
              className="input"
              rows={2}
              value={env}
              onChange={(e) => setEnv(e.target.value)}
              placeholder="NODE_ENV=production&#10;PORT=80"
              style={{ resize: "vertical", fontFamily: "var(--mono)", fontSize: "12px" }}
            />
          </div>

          <div>
            <label style={{ display: "block", fontSize: "12px", color: "var(--muted)", marginBottom: "4px" }}>
              Restart Policy
            </label>
            <select
              value={restart}
              onChange={(e) => setRestart(e.target.value)}
              className="input"
              style={{ height: "36px" }}
            >
              <option value="no">no (default)</option>
              <option value="always">always</option>
              <option value="unless-stopped">unless-stopped</option>
              <option value="on-failure">on-failure</option>
            </select>
          </div>
        </div>

        <div>
          <label style={{ display: "block", fontSize: "12px", color: "var(--muted)", marginBottom: "4px" }}>
            Command (Opsional)
          </label>
          <input
            type="text"
            className="input"
            value={command}
            onChange={(e) => setCommand(e.target.value)}
            placeholder="misal: sh -c 'npm start'"
          />
        </div>
      </form>
    </Modal>
  );
}
