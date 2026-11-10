import { useRef, useState } from "react";
import { QUICK_TERMINAL_COMMANDS } from "../fileManager";
import { DEFAULT_FONT_SIZE, MAX_FONT_SIZE, MIN_FONT_SIZE } from "../layout";
import type { ModifierState, SpecialKey } from "../mobileKeys";
import type { Snippet } from "../types";
import { Icon } from "./Icon";

interface MobileKeyBarProps {
  disabled: boolean;
  modifiers: ModifierState;
  hasJumpPaths: boolean;
  fontSize: number;
  snippets: Snippet[];
  onStepFontSize: (delta: number) => void;
  onResetFontSize: () => void;
  onToggleCtrl: () => void;
  onToggleAlt: () => void;
  onSpecialKey: (key: SpecialKey) => void;
  onSendCommand: (command: string) => void;
  onOpenFiles: () => void;
  onOpenPalette: () => void;
  onOpenJump: () => void;
  onPasteText: (text: string) => void;
  onUploadImages: (files: File[]) => void;
}

const KEY_BUTTONS: { key: SpecialKey; label: string; title: string }[] = [
  { key: "Esc", label: "Esc", title: "Escape" },
  { key: "Tab", label: "Tab", title: "Tab (Auto-complete)" },
  { key: "Up", label: "↑", title: "Panah Atas (Riwayat sebelumnya)" },
  { key: "Down", label: "↓", title: "Panah Bawah (Riwayat berikutnya)" },
  { key: "Left", label: "←", title: "Panah Kiri" },
  { key: "Right", label: "→", title: "Panah Kanan" },
  { key: "Home", label: "Home", title: "Awal baris" },
  { key: "End", label: "End", title: "Akhir baris" },
  { key: "PgUp", label: "PgUp", title: "Halaman atas" },
  { key: "PgDn", label: "PgDn", title: "Halaman bawah" },
  { key: "Hyphen", label: "-", title: "Tanda kurang / opsi (-)" },
  { key: "Slash", label: "/", title: "Garis miring (/)" },
  { key: "Pipe", label: "|", title: "Pipe (|)" },
  { key: "Tilde", label: "~", title: "Home (~)" },
  { key: "CtrlC", label: "^C", title: "Ctrl+C (Interrupt)" },
  { key: "CtrlD", label: "^D", title: "Ctrl+D (EOF / Logout)" },
  { key: "CtrlL", label: "^L", title: "Ctrl+L (Bersihkan layar)" },
];

export function MobileKeyBar({
  disabled,
  modifiers,
  hasJumpPaths,
  fontSize,
  snippets,
  onStepFontSize,
  onResetFontSize,
  onToggleCtrl,
  onToggleAlt,
  onSpecialKey,
  onSendCommand,
  onOpenFiles,
  onOpenPalette,
  onOpenJump,
  onPasteText,
  onUploadImages,
}: MobileKeyBarProps) {
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [showQuickChips, setShowQuickChips] = useState(false);
  const [showCompose, setShowCompose] = useState(false);
  const [composeText, setComposeText] = useState("");

  const keepFocus = (e: React.SyntheticEvent) => {
    e.preventDefault();
  };

  const handlePaste = async () => {
    try {
      const text = await navigator.clipboard.readText();
      if (text) onPasteText(text);
    } catch {
      // Clipboard API permission denied or unavailable; user can long-press inside terminal.
    }
  };

  const submitCompose = (e: React.FormEvent) => {
    e.preventDefault();
    if (!composeText || disabled) return;
    onSendCommand(`${composeText}\r`);
    setComposeText("");
  };

  return (
    <div className="mobile-keybar" role="toolbar" aria-label="Tombol bantu terminal">
      {showCompose && (
        <form className="mobile-compose-bar" onSubmit={submitCompose}>
          <input
            type="text"
            className="mobile-compose-input"
            value={composeText}
            onChange={(e) => setComposeText(e.target.value)}
            placeholder="Ketik / tempel perintah lengkap lalu tekan Kirim…"
            disabled={disabled}
            autoCapitalize="off"
            autoCorrect="off"
            spellCheck={false}
          />
          <button type="submit" className="btn btn-small btn-primary" disabled={disabled || !composeText}>
            <Icon name="send" size={12} /> Kirim
          </button>
        </form>
      )}

      {showQuickChips && (
        <div className="mobile-quick-strip" aria-label="Perintah cepat & snippet">
          {snippets.map((snip) => (
            <button
              key={`snip-${snip.id}`}
              type="button"
              className="mkey mkey-chip mkey-chip-snip"
              disabled={disabled}
              onMouseDown={keepFocus}
              onClick={() => onSendCommand(`${snip.command.replace(/\r?\n/g, "\r")}\r`)}
              title={snip.command}
            >
              <Icon name="bolt" size={11} /> {snip.name}
            </button>
          ))}
          {QUICK_TERMINAL_COMMANDS.map((cmd) => (
            <button
              key={cmd.label}
              type="button"
              className="mkey mkey-chip"
              disabled={disabled}
              onMouseDown={keepFocus}
              onClick={() => onSendCommand(cmd.command)}
              title={`Jalankan ${cmd.label}`}
            >
              <code>{cmd.label}</code>
            </button>
          ))}
        </div>
      )}

      <div className="mobile-keybar-scroll">
        <button
          type="button"
          className={`mkey mkey-mod${modifiers.ctrl ? " is-active" : ""}`}
          aria-pressed={modifiers.ctrl}
          disabled={disabled}
          onMouseDown={keepFocus}
          onPointerDown={keepFocus}
          onClick={onToggleCtrl}
          title="Modifier Ctrl (aktif untuk 1 tombol berikutnya)"
        >
          Ctrl
        </button>
        <button
          type="button"
          className={`mkey mkey-mod${modifiers.alt ? " is-active" : ""}`}
          aria-pressed={modifiers.alt}
          disabled={disabled}
          onMouseDown={keepFocus}
          onPointerDown={keepFocus}
          onClick={onToggleAlt}
          title="Modifier Alt (aktif untuk 1 tombol berikutnya)"
        >
          Alt
        </button>

        {KEY_BUTTONS.map((item) => (
          <button
            key={item.key}
            type="button"
            className="mkey"
            disabled={disabled}
            onMouseDown={keepFocus}
            onPointerDown={keepFocus}
            onClick={() => onSpecialKey(item.key)}
            title={item.title}
          >
            {item.label}
          </button>
        ))}

        <span className="mkey-sep" aria-hidden="true" />

        <button
          type="button"
          className={`mkey mkey-action${showCompose ? " is-active" : ""}`}
          onMouseDown={keepFocus}
          onClick={() => setShowCompose((v) => !v)}
          title="Buka kotak ketik perintah lengkap"
        >
          <Icon name="terminal" size={12} /> Ketik
        </button>

        <button
          type="button"
          className={`mkey mkey-action${showQuickChips ? " is-active" : ""}`}
          onMouseDown={keepFocus}
          onClick={() => setShowQuickChips((v) => !v)}
          title="Tampilkan perintah cepat & snippet"
        >
          <Icon name="bolt" size={12} /> Cepat
        </button>

        <button
          type="button"
          className="mkey mkey-action"
          onMouseDown={keepFocus}
          onClick={onOpenFiles}
          title="Buka File Manager (Cockpit)"
        >
          <Icon name="folder" size={12} /> File
        </button>

        <button
          type="button"
          className="mkey mkey-action"
          disabled={disabled}
          onMouseDown={keepFocus}
          onClick={() => void handlePaste()}
          title="Tempel teks dari clipboard"
        >
          <Icon name="clipboard" size={12} /> Tempel
        </button>

        <button
          type="button"
          className="mkey mkey-action"
          disabled={disabled}
          onMouseDown={keepFocus}
          onClick={() => fileInputRef.current?.click()}
          title="Unggah & tempel gambar ke terminal"
        >
          <Icon name="image" size={12} /> Gambar
        </button>

        {hasJumpPaths && (
          <button
            type="button"
            className="mkey mkey-action"
            disabled={disabled}
            onMouseDown={keepFocus}
            onClick={onOpenJump}
            title="Pindah ke path tersimpan lain (Alt+J)"
          >
            <Icon name="jump" size={12} /> Path
          </button>
        )}

        <button
          type="button"
          className="mkey mkey-action"
          onMouseDown={keepFocus}
          onClick={onOpenPalette}
          title="Lompat ke proyek/server (Alt+P)"
        >
          <Icon name="search" size={12} /> Proyek
        </button>

        <span className="mkey-sep" aria-hidden="true" />

        <button
          type="button"
          className="mkey"
          disabled={fontSize <= MIN_FONT_SIZE}
          onMouseDown={keepFocus}
          onClick={() => onStepFontSize(-1)}
          title="Kecilkan ukuran huruf terminal (Alt+-)"
        >
          A−
        </button>
        <button
          type="button"
          className="mkey mkey-action"
          onMouseDown={keepFocus}
          onClick={onResetFontSize}
          title={`Ukuran huruf ${fontSize}px — ketuk untuk reset ke ${DEFAULT_FONT_SIZE}px`}
        >
          {fontSize}px
        </button>
        <button
          type="button"
          className="mkey"
          disabled={fontSize >= MAX_FONT_SIZE}
          onMouseDown={keepFocus}
          onClick={() => onStepFontSize(1)}
          title="Besarkan ukuran huruf terminal (Alt+=)"
        >
          A+
        </button>
      </div>

      <input
        ref={fileInputRef}
        type="file"
        accept="image/png,image/jpeg,image/gif,image/webp"
        multiple
        className="sr-only"
        onChange={(e) => {
          const files = e.target.files ? Array.from(e.target.files) : [];
          e.target.value = "";
          if (files.length > 0) onUploadImages(files);
        }}
      />
    </div>
  );
}
