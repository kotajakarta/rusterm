/**
 * Converts a Blob to a pure Base64 string.
 */
export async function blobToBase64(blob: Blob): Promise<string> {
  const buffer = await blob.arrayBuffer();
  const bytes = new Uint8Array(buffer);
  let binary = "";
  const len = bytes.byteLength;
  const chunkSize = 8192;
  for (let i = 0; i < len; i += chunkSize) {
    const chunk = bytes.subarray(i, i + chunkSize);
    binary += String.fromCharCode(...chunk);
  }
  return btoa(binary);
}

/**
 * Utility helper to download or save a Blob safely across both desktop browsers and mobile WebView.
 */
export async function downloadBlob(blob: Blob, fileName: string): Promise<void> {
  // 1. Android WebView native interface
  const nativeApp = typeof window !== "undefined" ? (window.RusTermApp || window.TermDeckApp) : undefined;
  if (nativeApp?.saveBlob) {
    const base64Data = await blobToBase64(blob);
    nativeApp.saveBlob(base64Data, fileName, blob.type || "application/octet-stream");
    return;
  }

  // 2. Standard Web Browser download fallback
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = fileName;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  setTimeout(() => URL.revokeObjectURL(url), 3000);
}
