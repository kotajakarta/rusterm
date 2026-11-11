import { useEffect, useRef, useState } from "react";
import { Terminal, type ITheme } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebLinksAddon } from "@xterm/addon-web-links";
import "@xterm/xterm/css/xterm.css";

import { api, errorMessage, openExternal, terminalSocketUrl } from "../api";
import {
  deleteCommandSequence,
  deleteSelectionSequence,
  moveCursorSequence,
  selectAllRange,
  type CellPos,
} from "../lineEdit";
import { applyModifiers, specialKeySequence, type ModifierState, type SpecialKey } from "../mobileKeys";
import { formatPastedPaths, pickImages } from "../paste";
import type { ControlMessage, Tab, TabStatus } from "../types";

const THEME: ITheme = {
  background: "#141c1a",
  foreground: "#d8e0d9",
  cursor: "#e8a33d",
  cursorAccent: "#141c1a",
  selectionBackground: "rgba(232, 163, 61, 0.32)",
  black: "#1d2725",
  red: "#e2574c",
  green: "#7cbf6b",
  yellow: "#e8a33d",
  blue: "#5f9fd8",
  magenta: "#c088d0",
  cyan: "#4fb3c8",
  white: "#d8e0d9",
  brightBlack: "#5b6b66",
  brightRed: "#f07a6f",
  brightGreen: "#9fd88f",
  brightYellow: "#f3c06c",
  brightBlue: "#86bdf0",
  brightMagenta: "#d9a6e6",
  brightCyan: "#7fd0e0",
  brightWhite: "#f4f7f3",
};

const DIM = "\x1b[38;2;122;140;134m";
const RED = "\x1b[38;2;240;122;111m";
const RESET = "\x1b[0m";

export type Sender = (data: string) => void;

export interface TerminalController {
  send: (data: string) => void;
  sendSpecial: (key: SpecialKey, mods: ModifierState) => void;
  pasteText: (text: string) => void;
  pasteFiles: (files: File[]) => void;
  focus: () => void;
}

/** Alt-based shortcuts handled by App; browsers reserve most Ctrl combinations. */
export function isAppShortcut(event: KeyboardEvent): boolean {
  if (event.ctrlKey && event.shiftKey && event.code === "KeyP") return true;
  if (!event.altKey || event.ctrlKey || event.metaKey) return false;
  if (event.shiftKey) return ["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"].includes(event.code);
  return ["KeyP", "KeyT", "KeyW", "KeyJ", "Equal", "Minus", "Digit0"].includes(event.code);
}

interface TerminalViewProps {
  tab: Tab;
  label: string;
  /** Shown in a pane (hidden tabs keep their session but are not laid out). */
  visible: boolean;
  /** The pane that receives keyboard input. */
  focused: boolean;
  fontSize?: number;
  onStepFontSize?: (delta: number) => void;
  onBroadcastData?: (data: string) => boolean;
  modifiers?: ModifierState;
  onConsumeModifiers?: () => void;
  onStatus: (tabId: string, status: TabStatus) => void;
  onReconnect: (tabId: string) => void;
  registerSender: (tabId: string, sender: Sender | null) => void;
  registerController?: (tabId: string, controller: TerminalController | null) => void;
}

async function copyText(text: string): Promise<void> {
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    const area = document.createElement("textarea");
    area.value = text;
    document.body.appendChild(area);
    area.select();
    document.execCommand("copy");
    area.remove();
  }
}

interface Notice {
  kind: "busy" | "bad";
  text: string;
}

const NOTICE_MS = 5000;
/** Pointer travel (px) still treated as a click rather than a drag-select. */
const CLICK_SLOP_PX = 4;
/** Finger travel (px) before a touch counts as a scroll rather than a tap. */
const TOUCH_SCROLL_SLOP_PX = 8;
/** Fling: velocity kept per ~16ms frame, start/stop thresholds (px/ms), max pause before lift. */
const FLING_FRICTION = 0.95;
const FLING_MIN_VELOCITY = 0.25;
const FLING_STOP_VELOCITY = 0.03;
const FLING_MAX_PAUSE_MS = 80;

export function TerminalView({
  tab,
  label,
  visible,
  focused,
  fontSize = 14,
  onStepFontSize,
  onBroadcastData,
  modifiers,
  onConsumeModifiers,
  onStatus,
  onReconnect,
  registerSender,
  registerController,
}: TerminalViewProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const termRef = useRef<Terminal | null>(null);
  const fitRef = useRef<FitAddon | null>(null);
  const socketRef = useRef<WebSocket | null>(null);
  const statusRef = useRef<TabStatus>("connecting");
  const visibleRef = useRef(visible);
  const modifiersRef = useRef<ModifierState>(modifiers ?? { ctrl: false, alt: false });
  const [notice, setNotice] = useState<Notice | null>(null);
  const callbacks = useRef({ onStatus, onReconnect, onConsumeModifiers, onStepFontSize, onBroadcastData });
  callbacks.current = { onStatus, onReconnect, onConsumeModifiers, onStepFontSize, onBroadcastData };
  visibleRef.current = visible;
  modifiersRef.current = modifiers ?? { ctrl: false, alt: false };

  const setStatus = (status: TabStatus) => {
    statusRef.current = status;
    callbacks.current.onStatus(tab.id, status);
  };

  const send = (payload: object) => {
    const ws = socketRef.current;
    if (ws && ws.readyState === WebSocket.OPEN) ws.send(JSON.stringify(payload));
  };

  const safeFit = () => {
    const el = containerRef.current;
    if (!el || el.offsetWidth === 0 || el.offsetHeight === 0) return;
    try {
      fitRef.current?.fit();
    } catch {
      // Fit can throw while the renderer is not attached yet; the next resize retries.
    }
  };

  // Terminal instance: lives as long as the tab, survives reconnects.
  useEffect(() => {
    const term = new Terminal({
      fontFamily: '"IBM Plex Mono", ui-monospace, Consolas, monospace',
      fontSize,
      lineHeight: 1.15,
      cursorBlink: true,
      scrollback: 10000,
      theme: THEME,
      allowProposedApi: false,
    });
    const fit = new FitAddon();
    term.loadAddon(fit);
    term.loadAddon(new WebLinksAddon((_event, uri) => openExternal(uri)));
    term.open(containerRef.current as HTMLDivElement);
    termRef.current = term;
    fitRef.current = fit;

    // Editor-style editing of the command line (see lineEdit.ts). Off while a program
    // runs full-screen or handles the mouse itself, so vim, htop, etc. are untouched.
    const canEditLine = () =>
      statusRef.current === "connected" &&
      term.buffer.active.type === "normal" &&
      term.modes.mouseTrackingMode === "none";
    // The selection on screen is Ctrl+A's; any other selection change resets it.
    let commandSelected = false;
    const selectionSub = term.onSelectionChange(() => {
      commandSelected = false;
    });

    term.attachCustomKeyEventHandler((event) => {
      if (event.type !== "keydown") return true;
      // Ctrl+C copies when text is selected; otherwise it is sent as ^C (interrupt).
      if (event.ctrlKey && !event.altKey && event.code === "KeyC" && (event.shiftKey || term.hasSelection())) {
        const selection = term.getSelection();
        if (selection) void copyText(selection);
        term.clearSelection();
        event.preventDefault();
        return false;
      }
      // Ctrl+V / Ctrl+Shift+V: let the browser fire its native paste event, which xterm handles.
      if (event.ctrlKey && !event.altKey && event.code === "KeyV") return false;
      if (canEditLine()) {
        // Ctrl+A selects the command being typed; with nothing typed it stays ^A.
        if (event.ctrlKey && !event.altKey && !event.shiftKey && !event.metaKey && event.code === "KeyA") {
          const range = selectAllRange(term.buffer.active);
          if (range) {
            term.select(range.column, range.row, range.length);
            commandSelected = true;
            event.preventDefault();
            return false;
          }
        }
        // Backspace/Delete erase a selection on the command line; typing replaces it.
        // Ctrl+A's selection is re-read, since keys typed just before it may echo late.
        const selection = term.getSelectionPosition();
        const erase = event.key === "Backspace" || event.key === "Delete";
        const typing = event.key.length === 1 && !event.ctrlKey && !event.altKey && !event.metaKey;
        if (selection && (erase || typing)) {
          const appCursor = term.modes.applicationCursorKeysMode;
          const keys = commandSelected
            ? deleteCommandSequence(term.buffer.active, appCursor)
            : deleteSelectionSequence(term.buffer.active, selection, appCursor);
          if (keys) {
            send({ type: "input", data: keys });
            term.clearSelection();
            if (erase) {
              event.preventDefault();
              return false;
            }
          }
        }
      }
      // App shortcuts (see isAppShortcut) bubble up to the window listener in App.
      if (isAppShortcut(event)) return false;
      return true;
    });

    const dataSub = term.onData((data) => {
      const status = statusRef.current;
      if (status === "connected") {
        const mods = modifiersRef.current;
        let outgoing = data;
        if (mods.ctrl || mods.alt) {
          const transformed = applyModifiers(data, mods);
          if (transformed.consumed) {
            callbacks.current.onConsumeModifiers?.();
          }
          outgoing = transformed.data;
        }
        if (callbacks.current.onBroadcastData?.(outgoing)) {
          return;
        }
        send({ type: "input", data: outgoing });
      } else if ((status === "closed" || status === "error") && data === "\r") {
        callbacks.current.onReconnect(tab.id);
      }
    });
    const resizeSub = term.onResize(({ cols, rows }) => send({ type: "resize", cols, rows }));

    const observer = new ResizeObserver(() => {
      if (visibleRef.current) safeFit();
    });
    observer.observe(containerRef.current as HTMLDivElement);

    // Images are uploaded to the server and their path is pasted (bracketed, like a
    // text paste), which Claude Code and similar CLIs turn into an image attachment.
    const pasteImages = async (files: File[]) => {
      if (statusRef.current !== "connected") return;
      setNotice({ kind: "busy", text: files.length > 1 ? `Mengunggah ${files.length} gambar…` : "Mengunggah gambar…" });
      try {
        const paths: string[] = [];
        for (const file of files) paths.push((await api.pasteImage(tab.serverId, file)).path);
        if (termRef.current === term && statusRef.current === "connected") {
          term.paste(formatPastedPaths(paths));
          term.focus();
        }
        setNotice(null);
      } catch (err) {
        setNotice({ kind: "bad", text: `Gagal menempel gambar: ${errorMessage(err)}` });
      }
    };

    const host = containerRef.current as HTMLDivElement;
    // Capture phase: runs before xterm's own paste listener on its textarea.
    const onPaste = (event: ClipboardEvent) => {
      const data = event.clipboardData;
      if (!data) return;
      const images = pickImages(data.files, data.getData("text/plain"));
      if (images.length === 0) return;
      event.preventDefault();
      event.stopPropagation();
      void pasteImages(images);
    };
    const onDragOver = (event: DragEvent) => {
      if (!event.dataTransfer?.types.includes("Files")) return;
      event.preventDefault();
      event.dataTransfer.dropEffect = "copy";
    };
    const onDrop = (event: DragEvent) => {
      const data = event.dataTransfer;
      if (!data?.types.includes("Files")) return;
      // Always handled: otherwise the browser navigates away to the dropped file.
      event.preventDefault();
      const images = pickImages(data.files, "");
      if (images.length > 0) void pasteImages(images);
      else setNotice({ kind: "bad", text: "Hanya file gambar yang bisa dijatuhkan ke terminal." });
    };
    // A plain click on the command line moves the cursor there; drags still select.
    const cellAt = (event: MouseEvent): CellPos | null => {
      const screen = term.element?.querySelector<HTMLElement>(".xterm-screen");
      if (!screen) return null;
      const rect = screen.getBoundingClientRect();
      if (rect.width === 0 || rect.height === 0) return null;
      // Rounded so a click on a character's right half lands after it, like a text caret.
      const col = Math.round(((event.clientX - rect.left) / rect.width) * term.cols);
      const row = Math.floor(((event.clientY - rect.top) / rect.height) * term.rows);
      if (row < 0 || row >= term.rows) return null;
      return { x: Math.min(Math.max(col, 0), term.cols), y: row + term.buffer.active.viewportY };
    };
    let pressAt: { x: number; y: number } | null = null;
    const onMouseDown = (event: MouseEvent) => {
      pressAt = event.button === 0 ? { x: event.clientX, y: event.clientY } : null;
    };
    // mouseup, not click: any pointer movement while pressed makes xterm redraw the
    // rows under it, and the browser then never fires click.
    const onMouseUp = (event: MouseEvent) => {
      const press = pressAt;
      pressAt = null;
      if (!press || event.button !== 0 || event.detail !== 1) return;
      if (event.altKey || event.shiftKey || event.ctrlKey || event.metaKey) return;
      if (Math.abs(event.clientX - press.x) > CLICK_SLOP_PX || Math.abs(event.clientY - press.y) > CLICK_SLOP_PX) return;
      // Jitter across a cell edge leaves a one-character selection; a click drops it.
      if (term.getSelection().length > 1 || !canEditLine()) return;
      term.clearSelection();
      const cell = cellAt(event);
      if (!cell) return;
      const keys = moveCursorSequence(term.buffer.active, cell, term.modes.applicationCursorKeysMode);
      if (keys) send({ type: "input", data: keys });
    };

    // Touch: one finger scrolls, two fingers pinch the font size. Scrolling is fed to xterm
    // as wheel events, so it behaves like a mouse wheel everywhere: scrollback in the shell,
    // arrow keys in full-screen apps (less, vim), wheel reports when the app tracks the
    // mouse (tmux mouse mode, htop). xterm's own touch scrolling does none of the latter.
    let wheelRemainder = 0;
    const scrollByPixels = (dy: number, clientX: number, clientY: number) => {
      const screen = term.element?.querySelector<HTMLElement>(".xterm-screen");
      if (!screen || term.rows === 0) return;
      const rowHeight = screen.getBoundingClientRect().height / term.rows;
      if (rowHeight <= 0) return;
      wheelRemainder += dy;
      const lines = Math.trunc(wheelRemainder / rowHeight);
      if (lines === 0) return;
      wheelRemainder -= lines * rowHeight;
      for (let i = 0; i < Math.abs(lines); i++) {
        screen.dispatchEvent(
          new WheelEvent("wheel", {
            deltaY: Math.sign(lines) * rowHeight,
            deltaMode: WheelEvent.DOM_DELTA_PIXEL,
            clientX,
            clientY,
            bubbles: true,
            cancelable: true,
          }),
        );
      }
    };

    let pinchStartDist: number | null = null;
    let drag: { x: number; y: number; scrolling: boolean } | null = null;
    /** Finger speed in px/ms; positive scrolls toward newer output. */
    let velocity = 0;
    let lastMoveAt = 0;
    let flingFrame = 0;
    const stopFling = () => {
      if (flingFrame) cancelAnimationFrame(flingFrame);
      flingFrame = 0;
    };
    const touchDistance = (touches: TouchList): number => {
      if (touches.length < 2) return 0;
      const dx = touches[0].clientX - touches[1].clientX;
      const dy = touches[0].clientY - touches[1].clientY;
      return Math.hypot(dx, dy);
    };
    const onTouchStart = (event: TouchEvent) => {
      stopFling();
      if (event.touches.length === 2) {
        pinchStartDist = touchDistance(event.touches);
        drag = null;
      } else if (event.touches.length === 1) {
        const touch = event.touches[0];
        drag = { x: touch.clientX, y: touch.clientY, scrolling: false };
        velocity = 0;
        lastMoveAt = event.timeStamp;
        wheelRemainder = 0;
      }
    };
    const onTouchMove = (event: TouchEvent) => {
      // Ours replaces xterm's touch handling, which would scroll a second time.
      event.stopPropagation();
      if (event.touches.length === 2 && pinchStartDist !== null) {
        if (event.cancelable) event.preventDefault();
        const currentDist = touchDistance(event.touches);
        const diff = currentDist - pinchStartDist;
        if (Math.abs(diff) >= 24) {
          callbacks.current.onStepFontSize?.(diff > 0 ? 1 : -1);
          pinchStartDist = currentDist;
        }
        return;
      }
      if (!drag || event.touches.length !== 1) return;
      const touch = event.touches[0];
      const dy = drag.y - touch.clientY;
      if (!drag.scrolling) {
        if (Math.abs(dy) < TOUCH_SCROLL_SLOP_PX || Math.abs(dy) < Math.abs(touch.clientX - drag.x)) return;
        drag.scrolling = true;
      }
      if (event.cancelable) event.preventDefault();
      const dt = Math.max(event.timeStamp - lastMoveAt, 1);
      velocity = 0.8 * (dy / dt) + 0.2 * velocity;
      lastMoveAt = event.timeStamp;
      drag.x = touch.clientX;
      drag.y = touch.clientY;
      scrollByPixels(dy, touch.clientX, touch.clientY);
    };
    const onTouchEnd = (event: TouchEvent) => {
      if (event.touches.length < 2) pinchStartDist = null;
      if (event.touches.length > 0 || !drag) return;
      const last = drag;
      drag = null;
      // Fling only through scrollback; in full-screen apps it would flood them with keys.
      const inScrollback = term.buffer.active.type === "normal" && term.modes.mouseTrackingMode === "none";
      if (!last.scrolling || !inScrollback) return;
      if (event.timeStamp - lastMoveAt > FLING_MAX_PAUSE_MS || Math.abs(velocity) < FLING_MIN_VELOCITY) return;
      let speed = velocity;
      let prev = performance.now();
      const step = (now: number) => {
        const dt = now - prev;
        prev = now;
        scrollByPixels(speed * dt, last.x, last.y);
        speed *= Math.pow(FLING_FRICTION, dt / 16);
        flingFrame = Math.abs(speed) > FLING_STOP_VELOCITY ? requestAnimationFrame(step) : 0;
      };
      flingFrame = requestAnimationFrame(step);
    };

    host.addEventListener("paste", onPaste, true);
    host.addEventListener("dragover", onDragOver);
    host.addEventListener("drop", onDrop);
    host.addEventListener("mousedown", onMouseDown);
    host.addEventListener("mouseup", onMouseUp);
    // Capture phase: runs before (and stops) xterm's listeners on its own element.
    host.addEventListener("touchstart", onTouchStart, { capture: true, passive: true });
    host.addEventListener("touchmove", onTouchMove, { capture: true, passive: false });
    host.addEventListener("touchend", onTouchEnd, true);
    host.addEventListener("touchcancel", onTouchEnd, true);

    registerSender(tab.id, (data) => {
      if (statusRef.current === "connected") {
        send({ type: "input", data });
        term.focus();
      }
    });
    registerController?.(tab.id, {
      send: (data) => {
        if (statusRef.current === "connected") {
          send({ type: "input", data });
          term.focus();
        }
      },
      sendSpecial: (key, mods) => {
        if (statusRef.current === "connected") {
          const seq = specialKeySequence(key, term.modes.applicationCursorKeysMode, mods);
          send({ type: "input", data: seq });
          const isArrow = key === "Up" || key === "Down" || key === "Left" || key === "Right";
          if (!isArrow) {
            term.focus();
          }
        }
      },
      pasteText: (text) => {
        if (statusRef.current === "connected" && text) {
          term.paste(text);
          term.focus();
        }
      },
      pasteFiles: (files) => {
        const images = pickImages(files, "");
        if (images.length > 0) void pasteImages(images);
        else setNotice({ kind: "bad", text: "Format gambar tidak didukung (PNG, JPEG, GIF, WebP)." });
      },
      focus: () => term.focus(),
    });

    return () => {
      registerSender(tab.id, null);
      registerController?.(tab.id, null);
      host.removeEventListener("paste", onPaste, true);
      host.removeEventListener("dragover", onDragOver);
      host.removeEventListener("drop", onDrop);
      host.removeEventListener("mousedown", onMouseDown);
      host.removeEventListener("mouseup", onMouseUp);
      host.removeEventListener("touchstart", onTouchStart, true);
      host.removeEventListener("touchmove", onTouchMove, true);
      host.removeEventListener("touchend", onTouchEnd, true);
      host.removeEventListener("touchcancel", onTouchEnd, true);
      stopFling();
      observer.disconnect();
      selectionSub.dispose();
      dataSub.dispose();
      resizeSub.dispose();
      term.dispose();
      termRef.current = null;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // One SSH session per `tab.session` value; bumping it reconnects.
  useEffect(() => {
    const term = termRef.current;
    if (!term) return;
    safeFit();
    term.write(`${tab.session > 0 ? "\r\n" : ""}${DIM}Menghubungkan ke ${label}…${RESET}\r\n`);
    setStatus("connecting");

    const ws = new WebSocket(terminalSocketUrl(tab.serverId, tab.bookmarkId, term.cols, term.rows));
    ws.binaryType = "arraybuffer";
    socketRef.current = ws;
    let finished = false;

    const finish = (status: TabStatus, line: string) => {
      if (finished) return;
      finished = true;
      term.write(`\r\n${line}\r\n${DIM}Tekan Enter untuk menyambung ulang.${RESET}\r\n`);
      setStatus(status);
    };

    ws.onmessage = (event: MessageEvent<ArrayBuffer | string>) => {
      if (typeof event.data !== "string") {
        term.write(new Uint8Array(event.data));
        return;
      }
      let msg: ControlMessage;
      try {
        msg = JSON.parse(event.data) as ControlMessage;
      } catch {
        return;
      }
      if (msg.type === "status" && msg.status === "connected") {
        setStatus("connected");
        send({ type: "resize", cols: term.cols, rows: term.rows });
      } else if (msg.type === "error") {
        finish("error", `${RED}${msg.message}${RESET}`);
      } else if (msg.type === "exit") {
        finish("closed", `${DIM}Sesi berakhir${msg.code !== null ? ` (kode ${msg.code})` : ""}.${RESET}`);
      }
    };
    ws.onerror = () => finish("error", `${RED}Tidak bisa terhubung ke backend RusTerm.${RESET}`);
    ws.onclose = () => finish("closed", `${DIM}Koneksi terputus.${RESET}`);

    return () => {
      finished = true;
      ws.onclose = null;
      ws.onerror = null;
      ws.onmessage = null;
      ws.close();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tab.session]);

  useEffect(() => {
    const term = termRef.current;
    if (!term) return;
    if (term.options.fontSize !== fontSize) {
      term.options.fontSize = fontSize;
      if (visibleRef.current) {
        requestAnimationFrame(() => safeFit());
      }
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [fontSize]);

  useEffect(() => {
    if (!visible) return;
    const frame = requestAnimationFrame(() => safeFit());
    return () => cancelAnimationFrame(frame);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [visible]);

  useEffect(() => {
    if (!visible || !focused) return;
    const frame = requestAnimationFrame(() => termRef.current?.focus());
    return () => cancelAnimationFrame(frame);
  }, [visible, focused]);

  useEffect(() => {
    if (notice?.kind !== "bad") return;
    const timer = window.setTimeout(() => setNotice(null), NOTICE_MS);
    return () => window.clearTimeout(timer);
  }, [notice]);

  return (
    <div className="terminal-wrap">
      <div className="terminal-host" ref={containerRef} />
      {notice && (
        <div className={`terminal-toast ${notice.kind === "bad" ? "is-bad" : ""}`} role="status">
          {notice.text}
        </div>
      )}
    </div>
  );
}
