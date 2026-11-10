import { useMemo, useState } from "react";
import { api, errorMessage } from "../../api";
import type { ImageItem } from "../../types";
import { Icon } from "../Icon";

interface ImageListProps {
  serverId: number;
  images: ImageItem[];
  loading: boolean;
  onRefresh: () => void;
  onRunImage: (imageName: string) => void;
}

export function ImageList({
  serverId,
  images,
  loading,
  onRefresh,
  onRunImage,
}: ImageListProps) {
  const [query, setQuery] = useState<string>("");
  const [pullInput, setPullInput] = useState<string>("");
  const [showPull, setShowPull] = useState<boolean>(false);
  const [pulling, setPulling] = useState<boolean>(false);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);

  const filtered = useMemo(() => {
    if (!query.trim()) return images;
    const q = query.toLowerCase();
    return images.filter(
      (img) =>
        img.repository.toLowerCase().includes(q) ||
        img.tag.toLowerCase().includes(q) ||
        img.id.toLowerCase().includes(q)
    );
  }, [images, query]);

  const handlePull = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!pullInput.trim()) return;

    setPulling(true);
    setError(null);
    setSuccess(null);
    try {
      await api.containersPullImage(serverId, pullInput.trim());
      setSuccess(`Image ${pullInput.trim()} berhasil diunduh!`);
      setPullInput("");
      setShowPull(false);
      onRefresh();
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setPulling(false);
    }
  };

  const handleDelete = async (imageId: string, imageName: string) => {
    if (!confirm(`Hapus image ${imageName} (${imageId})?`)) return;

    setBusyId(imageId);
    setError(null);
    try {
      await api.containersDeleteImage(serverId, imageId, false);
      onRefresh();
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusyId(null);
    }
  };

  return (
    <div className="containers-subview">
      {error && (
        <div className="containers-error-banner">
          <span>{error}</span>
          <button type="button" onClick={() => setError(null)} className="icon-btn">
            ×
          </button>
        </div>
      )}

      {success && (
        <div
          style={{
            padding: "8px 12px",
            background: "rgba(124, 191, 107, 0.15)",
            color: "var(--green)",
            borderRadius: "var(--r-sm)",
            marginBottom: "12px",
            fontSize: "13px",
            display: "flex",
            justifyContent: "space-between",
          }}
        >
          <span>{success}</span>
          <button type="button" onClick={() => setSuccess(null)} className="icon-btn">
            ×
          </button>
        </div>
      )}

      {/* Action / Pull Bar */}
      <div className="containers-filter-bar">
        <div className="containers-search-wrap">
          <Icon name="search" size={14} className="containers-search-icon" />
          <input
            type="text"
            className="input containers-search-input"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Cari image lokal berdasarkan repository, tag, atau ID..."
          />
          {query && (
            <button
              type="button"
              className="icon-btn containers-search-clear"
              onClick={() => setQuery("")}
            >
              ×
            </button>
          )}
        </div>

        <button
          type="button"
          className="btn btn-primary"
          onClick={() => setShowPull((v) => !v)}
        >
          <Icon name="download" size={14} /> <span>Pull Image</span>
        </button>
      </div>

      {showPull && (
        <form
          onSubmit={handlePull}
          style={{
            display: "flex",
            gap: "8px",
            padding: "12px",
            background: "var(--bg-card)",
            border: "1px solid var(--line)",
            borderRadius: "var(--r-md)",
            marginBottom: "14px",
            alignItems: "center",
          }}
        >
          <input
            type="text"
            className="input"
            value={pullInput}
            onChange={(e) => setPullInput(e.target.value)}
            placeholder="Nama image, misal: docker.io/library/alpine:latest atau registry.redhat.io/ubi9/ubi:latest"
            autoFocus
            required
            style={{ flex: 1 }}
          />
          <button
            type="submit"
            className="btn btn-primary"
            disabled={pulling || !pullInput.trim()}
          >
            {pulling ? "Mengunduh..." : "Download"}
          </button>
          <button
            type="button"
            className="btn btn-subtle"
            onClick={() => setShowPull(false)}
            disabled={pulling}
          >
            Batal
          </button>
        </form>
      )}

      {/* Image Table */}
      {filtered.length === 0 ? (
        <div className="containers-empty-state">
          <Icon name="layers" size={36} />
          <p>{loading ? "Memuat images..." : "Tidak ada image yang ditemukan."}</p>
          {!loading && (
            <button
              type="button"
              className="btn btn-primary"
              onClick={() => setShowPull(true)}
            >
              <Icon name="download" size={14} /> Pull Image Sekarang
            </button>
          )}
        </div>
      ) : (
        <div className="containers-table-wrap">
          <table className="containers-table">
            <thead>
              <tr>
                <th>Repository</th>
                <th>Tag</th>
                <th>Image ID</th>
                <th>Ukuran</th>
                <th>Created</th>
                <th style={{ textAlign: "right" }}>Aksi</th>
              </tr>
            </thead>
            <tbody>
              {filtered.map((img) => {
                const fullName =
                  img.repository !== "<none>"
                    ? `${img.repository}:${img.tag !== "<none>" ? img.tag : "latest"}`
                    : img.id;
                const isBusy = busyId === img.id;

                return (
                  <tr key={img.id + img.repository + img.tag}>
                    <td>
                      <span className="mono bold" style={{ color: "var(--text)" }}>
                        {img.repository}
                      </span>
                    </td>
                    <td>
                      <span className="tag-pill">{img.tag}</span>
                    </td>
                    <td>
                      <span className="mono" style={{ color: "var(--muted)" }}>
                        {img.id}
                      </span>
                    </td>
                    <td>{img.size || "-"}</td>
                    <td style={{ color: "var(--muted)", fontSize: "12px" }}>
                      {img.created || "-"}
                    </td>
                    <td style={{ textAlign: "right" }}>
                      <div style={{ display: "inline-flex", gap: "6px" }}>
                        <button
                          type="button"
                          className="btn-action btn-start"
                          onClick={() => onRunImage(fullName)}
                          title="Jalankan kontainer dari image ini"
                        >
                          <Icon name="play" size={12} /> <span>Run</span>
                        </button>
                        <button
                          type="button"
                          className="btn-action btn-delete"
                          onClick={() => void handleDelete(img.id, fullName)}
                          disabled={isBusy}
                          title="Hapus image"
                        >
                          <Icon name="trash" size={12} />
                        </button>
                      </div>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
