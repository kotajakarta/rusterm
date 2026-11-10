import { useCallback, useEffect, useState } from "react";
import { api, errorMessage } from "../../api";
import type {
  ContainerEngineInfo,
  ContainerItem,
  ImageItem,
  Server,
} from "../../types";
import { Icon } from "../Icon";
import { ContainerList } from "./ContainerList";
import { ContainerLogsModal } from "./ContainerLogsModal";
import { ContainerRunModal } from "./ContainerRunModal";
import { ImageList } from "./ImageList";

interface ContainersViewProps {
  servers: Server[];
  activeServerId: number | null;
  onSelectServer: (id: number) => void;
  onOpenTerminal: (serverId: number, startupCmd: string, title: string) => void;
}

export function ContainersView({
  servers,
  activeServerId,
  onSelectServer,
  onOpenTerminal,
}: ContainersViewProps) {
  const currentServerId = activeServerId ?? servers[0]?.id ?? 0;
  const [selectedServerId, setSelectedServerId] = useState<number>(currentServerId);

  const [activeTab, setActiveTab] = useState<"containers" | "images">("containers");
  const [engineInfo, setEngineInfo] = useState<ContainerEngineInfo | null>(null);
  const [containers, setContainers] = useState<ContainerItem[]>([]);
  const [images, setImages] = useState<ImageItem[]>([]);
  const [loading, setLoading] = useState<boolean>(true);
  const [error, setError] = useState<string | null>(null);

  // Modals state
  const [logsModal, setLogsModal] = useState<{ id: string; name: string } | null>(null);
  const [runModal, setRunModal] = useState<{ open: boolean; initialImage?: string }>({ open: false });

  const loadData = useCallback(async (srvId: number) => {
    if (!srvId) return;
    setLoading(true);
    setError(null);
    try {
      const eng = await api.containersEngine(srvId);
      setEngineInfo(eng);

      if (eng.engine !== "none") {
        const [cList, iList] = await Promise.all([
          api.containersList(srvId, true),
          api.containersImages(srvId),
        ]);
        setContainers(cList);
        setImages(iList);
      } else {
        setContainers([]);
        setImages([]);
      }
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    if (activeServerId && activeServerId !== selectedServerId) {
      setSelectedServerId(activeServerId);
    }
  }, [activeServerId]);

  useEffect(() => {
    if (selectedServerId) {
      void loadData(selectedServerId);
    }
  }, [selectedServerId, loadData]);

  const handleServerChange = (newId: number) => {
    setSelectedServerId(newId);
    onSelectServer(newId);
  };

  const handleExecTerminal = (containerId: string, containerName: string) => {
    const engine = engineInfo?.engine || "podman";
    const cmd = `${engine} exec -it ${containerId} /bin/sh || ${engine} exec -it ${containerId} bash`;
    onOpenTerminal(selectedServerId, cmd, `Exec: ${containerName}`);
  };

  const selectedServer = servers.find((s) => s.id === selectedServerId);

  return (
    <div className="containers-view">
      {/* Top Header Bar */}
      <header className="containers-header">
        <div className="containers-header-left">
          <div className="containers-server-selector">
            <Icon name="server" size={14} />
            <select
              value={selectedServerId}
              onChange={(e) => handleServerChange(Number(e.target.value))}
              className="containers-server-select"
            >
              {servers.map((s) => (
                <option key={s.id} value={s.id}>
                  {s.name} ({s.host})
                </option>
              ))}
            </select>
          </div>

          {/* Engine Info Badge */}
          {engineInfo && (
            <div className={`engine-badge is-${engineInfo.engine}`}>
              <Icon name="box" size={13} />
              <span>
                {engineInfo.engine === "podman"
                  ? `Podman ${engineInfo.version || ""}${engineInfo.rootless ? " (rootless)" : ""}`
                  : engineInfo.engine === "docker"
                  ? `Docker ${engineInfo.version || ""}${engineInfo.rootless ? " (rootless)" : ""}`
                  : "Engine Tidak Ditemukan"}
              </span>
            </div>
          )}
        </div>

        <div className="containers-header-right">
          {/* Sub-tabs [Containers | Images] */}
          {engineInfo && engineInfo.engine !== "none" && (
            <div className="containers-tabs-switch" role="tablist">
              <button
                type="button"
                role="tab"
                aria-selected={activeTab === "containers"}
                className={`containers-tab-btn${activeTab === "containers" ? " is-active" : ""}`}
                onClick={() => setActiveTab("containers")}
              >
                <Icon name="box" size={13} />
                <span>Kontainer</span>
                <span className="count-badge">{containers.length}</span>
              </button>
              <button
                type="button"
                role="tab"
                aria-selected={activeTab === "images"}
                className={`containers-tab-btn${activeTab === "images" ? " is-active" : ""}`}
                onClick={() => setActiveTab("images")}
              >
                <Icon name="layers" size={13} />
                <span>Images</span>
                <span className="count-badge">{images.length}</span>
              </button>
            </div>
          )}

          <button
            type="button"
            className="btn btn-subtle"
            onClick={() => void loadData(selectedServerId)}
            disabled={loading}
            title="Refresh status kontainer"
            style={{ padding: "5px 10px", fontSize: "12px" }}
          >
            <Icon name="refresh" size={14} className={loading ? "spin" : ""} />
            <span className="hide-mobile">Refresh</span>
          </button>
        </div>
      </header>

      {/* Main Content Area */}
      <div className="containers-content">
        {error && (
          <div className="containers-error-banner" style={{ margin: "16px" }}>
            <span>{error}</span>
          </div>
        )}

        {!error && engineInfo && engineInfo.engine === "none" && (
          <div className="containers-empty-state">
            <Icon name="box" size={48} />
            <h2>Podman atau Docker Belum Terpasang</h2>
            <p>
              Server <strong>{selectedServer?.name || selectedServerId}</strong> belum memiliki
              runtime container Podman atau Docker.
            </p>
            <div className="containers-install-hint">
              <strong>Rekomendasi Instalasi (RHEL / CentOS / Fedora):</strong>
              <code>sudo dnf install -y podman</code>
              <br />
              <strong style={{ marginTop: "8px", display: "inline-block" }}>
                Rekomendasi Instalasi (Ubuntu / Debian):
              </strong>
              <code>sudo apt update && sudo apt install -y podman # atau docker.io</code>
            </div>
          </div>
        )}

        {!error && engineInfo && engineInfo.engine !== "none" && (
          <>
            {activeTab === "containers" ? (
              <ContainerList
                serverId={selectedServerId}
                containers={containers}
                loading={loading}
                onRefresh={() => void loadData(selectedServerId)}
                onOpenLogs={(cid, cname) => setLogsModal({ id: cid, name: cname })}
                onExecTerminal={handleExecTerminal}
                onRunNew={() => setRunModal({ open: true })}
              />
            ) : (
              <ImageList
                serverId={selectedServerId}
                images={images}
                loading={loading}
                onRefresh={() => void loadData(selectedServerId)}
                onRunImage={(img) => setRunModal({ open: true, initialImage: img })}
              />
            )}
          </>
        )}
      </div>

      {/* Logs Modal */}
      {logsModal && (
        <ContainerLogsModal
          serverId={selectedServerId}
          containerId={logsModal.id}
          containerName={logsModal.name}
          onClose={() => setLogsModal(null)}
        />
      )}

      {/* Run Container Modal */}
      {runModal.open && (
        <ContainerRunModal
          serverId={selectedServerId}
          initialImage={runModal.initialImage}
          onClose={() => setRunModal({ open: false })}
          onSuccess={() => void loadData(selectedServerId)}
        />
      )}
    </div>
  );
}
