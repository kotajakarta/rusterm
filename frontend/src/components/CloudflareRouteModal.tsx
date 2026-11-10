import { useState, type FormEvent } from "react";
import { api, errorMessage } from "../api";
import type { CFRoute, CFZone } from "../types";
import { Icon } from "./Icon";
import { Modal } from "./Modal";

interface Props {
  accountId: number;
  tunnelId: string;
  tunnelName: string;
  zones: CFZone[];
  editRoute?: CFRoute | null;
  onRouteAdded: () => void;
  onClose: () => void;
}

function parseService(svc: string): { proto: string; target: string } {
  const match = svc.match(/^([a-z]+:\/\/)(.*)$/);
  if (match) {
    return { proto: match[1], target: match[2] };
  }
  return { proto: "http://", target: svc };
}

export function CloudflareRouteModal({
  accountId,
  tunnelId,
  tunnelName,
  zones,
  editRoute,
  onRouteAdded,
  onClose,
}: Props) {
  const isEdit = Boolean(editRoute);
  const parsed = editRoute ? parseService(editRoute.service) : { proto: "http://", target: "localhost:1945" };

  const [subdomain, setSubdomain] = useState("");
  const [selectedZoneId, setSelectedZoneId] = useState<string>(zones[0]?.id ?? "");
  const [protocol, setProtocol] = useState<string>(parsed.proto);
  const [originTarget, setOriginTarget] = useState<string>(parsed.target);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [resultWarning, setResultWarning] = useState<{
    warning: string;
    cnameTarget?: string;
    hostname: string;
  } | null>(null);
  const [copiedTarget, setCopiedTarget] = useState(false);

  const selectedZone = zones.find((z) => z.id === selectedZoneId) ?? zones[0];

  const fullHostname = isEdit && editRoute
    ? editRoute.hostname
    : subdomain.trim() && subdomain.trim() !== "@"
    ? `${subdomain.trim()}.${selectedZone?.name ?? ""}`
    : selectedZone?.name ?? "";

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    if (!isEdit && !selectedZone) {
      setError("Pilih domain (zone) terlebih dahulu.");
      return;
    }
    const cleanHostname = fullHostname.toLowerCase();
    const cleanService = `${protocol}${originTarget.trim().replace(/^[a-z]+:\/\//, "")}`;

    setSaving(true);
    setError(null);
    try {
      const res = await api.cfCreateRoute(accountId, tunnelId, {
        hostname: cleanHostname,
        service: cleanService,
        zone_id: selectedZone?.id,
      });
      onRouteAdded();
      if (res.dns_warning) {
        setResultWarning({
          warning: res.dns_warning,
          cnameTarget: res.cname_target,
          hostname: cleanHostname,
        });
      } else {
        onClose();
      }
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setSaving(false);
    }
  };

  const handleCopyTarget = async () => {
    if (!resultWarning?.cnameTarget) return;
    try {
      await navigator.clipboard.writeText(resultWarning.cnameTarget);
      setCopiedTarget(true);
      setTimeout(() => setCopiedTarget(false), 1800);
    } catch {
      // ignore
    }
  };

  if (resultWarning) {
    return (
      <Modal
        title="Rute Tunnel Berhasil Didaftarkan"
        onClose={onClose}
        footer={
          <>
            <button
              type="button"
              className="btn btn-secondary"
              onClick={() => void handleCopyTarget()}
            >
              <Icon name={copiedTarget ? "check" : "copy"} size={13} />{" "}
              {copiedTarget ? "Tersalin!" : "Salin CNAME Target"}
            </button>
            <button type="button" className="btn btn-primary" onClick={onClose}>
              Selesai
            </button>
          </>
        }
      >
        <div className="backup-desc">
          <div className="notice warn" style={{ padding: "10px 14px", marginBottom: "14px" }}>
            <strong style={{ display: "block", marginBottom: "4px" }}>
              Perhatian: Pembuatan DNS CNAME Otomatis Gagal
            </strong>
            <p style={{ margin: 0, fontSize: "11.5px", lineHeight: 1.5 }}>{resultWarning.warning}</p>
          </div>

          <p style={{ fontSize: "12px", color: "var(--text)", marginBottom: "8px" }}>
            Rute Ingress pada tunnel <strong>{tunnelName}</strong> sudah aktif mengarahkan ke{" "}
            <code>{protocol}{originTarget.trim().replace(/^[a-z]+:\/\//, "")}</code>.
            Jika domain belum memiliki record DNS CNAME, tambahkan record berikut di Cloudflare DNS Dashboard:
          </p>

          <div
            style={{
              background: "var(--panel)",
              border: "1px solid var(--line)",
              borderRadius: "var(--r-md)",
              padding: "10px 14px",
              display: "flex",
              flexDirection: "column",
              gap: "6px",
              fontSize: "12px",
            }}
          >
            <div>
              <span style={{ color: "var(--faint)", width: "100px", display: "inline-block" }}>Type:</span>
              <strong style={{ color: "var(--accent)" }}>CNAME</strong>
            </div>
            <div>
              <span style={{ color: "var(--faint)", width: "100px", display: "inline-block" }}>Name:</span>
              <code>{resultWarning.hostname}</code>
            </div>
            <div>
              <span style={{ color: "var(--faint)", width: "100px", display: "inline-block" }}>Target:</span>
              <code>{resultWarning.cnameTarget}</code>
            </div>
            <div>
              <span style={{ color: "var(--faint)", width: "100px", display: "inline-block" }}>Proxy status:</span>
              <span style={{ color: "var(--ok)", fontWeight: 600 }}>Proxied (Orange Cloud)</span>
            </div>
          </div>
        </div>
      </Modal>
    );
  }

  return (
    <Modal
      title={isEdit ? `Edit Service Origin — ${editRoute?.hostname}` : `Tambah Rute Domain — ${tunnelName}`}
      onClose={onClose}
      footer={
        <>
          <button type="button" className="btn" onClick={onClose} disabled={saving}>
            Batal
          </button>
          <button
            type="button"
            className="btn btn-primary"
            onClick={(e) => void handleSubmit(e)}
            disabled={saving || (!isEdit && !selectedZone)}
          >
            <Icon name="check" size={14} />{" "}
            {saving ? "Menyimpan..." : isEdit ? "Simpan Perubahan Origin" : "Buat Rute & DNS CNAME"}
          </button>
        </>
      }
    >
      <form onSubmit={handleSubmit} className="backup-form">
        <p className="backup-desc">
          {isEdit ? (
            <>
              Ubah tujuan forwarding <strong>Service Origin</strong> untuk domain ini. Perubahan akan
              langsung diterapkan ke Ingress Configuration Cloudflare Tunnel tanpa mengubah DNS CNAME.
            </>
          ) : (
            <>
              Sistem akan mendaftarkan hostname publik ke <strong>Cloudflare Tunnel (Ingress)</strong> dan
              secara otomatis membuat record <strong>DNS CNAME</strong> (ter-proxy Cloudflare CDN/SSL).
            </>
          )}
        </p>

        {isEdit ? (
          <div className="form-field">
            <span>Domain Publik (Hostname):</span>
            <div
              style={{
                display: "flex",
                alignItems: "center",
                gap: "8px",
                padding: "8px 12px",
                background: "var(--panel)",
                border: "1px solid var(--line)",
                borderRadius: "var(--r-sm)",
              }}
            >
              <Icon name="cloud" size={16} />
              <strong style={{ fontSize: "13px", color: "var(--text)" }}>{editRoute?.hostname}</strong>
              <span
                style={{
                  marginLeft: "auto",
                  fontSize: "10.5px",
                  color: "var(--faint)",
                  background: "var(--panel-2)",
                  padding: "1px 6px",
                  borderRadius: "3px",
                }}
              >
                Locked Hostname
              </span>
            </div>
          </div>
        ) : (
          <label className="form-field">
            <span>Domain Publik (Hostname):</span>
            <div className="cf-hostname-input-group">
              <input
                type="text"
                placeholder="subdomain (mis. termdeck, app)"
                value={subdomain}
                onChange={(e) => setSubdomain(e.target.value)}
                disabled={saving}
                className="cf-subdomain-input"
              />
              <span className="cf-dot-separator">.</span>
              <select
                value={selectedZoneId}
                onChange={(e) => setSelectedZoneId(e.target.value)}
                disabled={saving}
                className="cf-zone-select"
                required
              >
                {zones.map((z) => (
                  <option key={z.id} value={z.id}>
                    {z.name}
                  </option>
                ))}
              </select>
            </div>
            {fullHostname && (
              <div className="cf-preview-hint">
                URL Publik: <code>https://{fullHostname}</code>
              </div>
            )}
          </label>
        )}

        <label className="form-field">
          <span>Service Origin (Aplikasi Target):</span>
          <div className="cf-origin-input-group">
            <select
              value={protocol}
              onChange={(e) => setProtocol(e.target.value)}
              disabled={saving}
              className="cf-proto-select"
            >
              <option value="http://">http://</option>
              <option value="https://">https://</option>
              <option value="ssh://">ssh://</option>
              <option value="tcp://">tcp://</option>
            </select>
            <input
              type="text"
              placeholder="localhost:1945 atau 192.168.1.x:port"
              value={originTarget}
              onChange={(e) => setOriginTarget(e.target.value)}
              disabled={saving}
              required
              className="cf-target-input"
            />
          </div>
          <div className="cf-preview-hint">
            Target Forwarding: <code>{protocol}{originTarget.trim().replace(/^[a-z]+:\/\//, "")}</code>
          </div>
        </label>

        {error && (
          <div className="notice bad" style={{ padding: "8px 12px", marginTop: "12px" }}>
            {error}
          </div>
        )}
      </form>
    </Modal>
  );
}
