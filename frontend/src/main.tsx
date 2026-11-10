import { createRoot } from "react-dom/client";
import "@fontsource/ibm-plex-sans/400.css";
import "@fontsource/ibm-plex-sans/500.css";
import "@fontsource/ibm-plex-sans/600.css";
import "@fontsource/ibm-plex-mono/400.css";
import "@fontsource/ibm-plex-mono/600.css";

import { Root } from "./Root";
import "./styles.css";

async function boot(): Promise<void> {
  // xterm measures glyph size when a terminal opens, so the mono font must be ready first.
  try {
    await Promise.all([
      document.fonts.load('400 14px "IBM Plex Mono"'),
      document.fonts.load('600 14px "IBM Plex Mono"'),
    ]);
  } catch {
    // Fall back to whatever monospace font is available.
  }
  createRoot(document.getElementById("root") as HTMLElement).render(<Root />);
}

void boot();
