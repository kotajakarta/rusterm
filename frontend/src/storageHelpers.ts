import type { StorageProvider, StorageTransferDirection } from "./types";

export interface StorageProviderPreset {
  id: StorageProvider;
  label: string;
  endpointPlaceholder: string;
  defaultEndpoint: string;
  defaultRegion: string;
  defaultPathStyle: boolean;
  hint: string;
}

export const STORAGE_PROVIDER_PRESETS: StorageProviderPreset[] = [
  {
    id: "r2",
    label: "Cloudflare R2",
    endpointPlaceholder: "https://<account_id>.r2.cloudflarestorage.com",
    defaultEndpoint: "",
    defaultRegion: "auto",
    defaultPathStyle: true,
    hint: "Gunakan Account ID Cloudflare pada endpoint dan region 'auto'.",
  },
  {
    id: "aws",
    label: "AWS S3",
    endpointPlaceholder: "https://s3.ap-southeast-1.amazonaws.com",
    defaultEndpoint: "https://s3.ap-southeast-1.amazonaws.com",
    defaultRegion: "ap-southeast-1",
    defaultPathStyle: false,
    hint: "Sesuaikan endpoint dan region bucket AWS S3 Anda.",
  },
  {
    id: "minio",
    label: "MinIO",
    endpointPlaceholder: "http://127.0.0.1:9000",
    defaultEndpoint: "http://127.0.0.1:9000",
    defaultRegion: "us-east-1",
    defaultPathStyle: true,
    hint: "Path-style wajib aktif untuk sebagian besar deployment MinIO.",
  },
  {
    id: "custom",
    label: "Custom S3",
    endpointPlaceholder: "https://s3.example.com",
    defaultEndpoint: "",
    defaultRegion: "us-east-1",
    defaultPathStyle: true,
    hint: "Untuk Backblaze B2, Wasabi, DigitalOcean Spaces, atau S3-compatible lain.",
  },
];

export function getProviderPreset(provider: string): StorageProviderPreset {
  return (
    STORAGE_PROVIDER_PRESETS.find((p) => p.id === provider) ??
    STORAGE_PROVIDER_PRESETS[STORAGE_PROVIDER_PRESETS.length - 1]
  );
}

export function validateBucketName(raw: string): string | null {
  const b = raw.trim();
  if (b.length < 3 || b.length > 63) {
    return "Nama bucket harus 3–63 karakter.";
  }
  if (b.includes("..")) {
    return "Nama bucket tidak boleh mengandung '..'.";
  }
  if (!/^[a-z0-9][a-z0-9.-]*[a-z0-9]$/.test(b)) {
    return "Hanya huruf kecil, angka, tanda hubung (-), atau titik (.), diawali dan diakhiri huruf/angka.";
  }
  return null;
}

export interface StorageBreadcrumb {
  label: string;
  prefix: string;
}

export function buildBreadcrumbs(prefix: string): StorageBreadcrumb[] {
  const trimmed = prefix.replace(/^\/+|\/+$/g, "");
  if (!trimmed) return [];
  const parts = trimmed.split("/").filter(Boolean);
  const out: StorageBreadcrumb[] = [];
  let acc = "";
  for (const part of parts) {
    acc += `${part}/`;
    out.push({ label: part, prefix: acc });
  }
  return out;
}

export function formatTransferDirection(direction: StorageTransferDirection): string {
  switch (direction) {
    case "ssh_to_s3":
      return "SSH → S3";
    case "s3_to_ssh":
      return "S3 → SSH";
    case "s3_to_s3":
      return "S3 → S3";
  }
}

export function formatStorageBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let val = bytes;
  let idx = 0;
  while (val >= 1024 && idx < units.length - 1) {
    val /= 1024;
    idx += 1;
  }
  return `${val >= 10 || idx === 0 ? val.toFixed(0) : val.toFixed(1)} ${units[idx]}`;
}
