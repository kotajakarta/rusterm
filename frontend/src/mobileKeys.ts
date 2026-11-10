export interface ModifierState {
  ctrl: boolean;
  alt: boolean;
}

export type SpecialKey =
  | "Esc"
  | "Tab"
  | "Up"
  | "Down"
  | "Left"
  | "Right"
  | "Home"
  | "End"
  | "PgUp"
  | "PgDn"
  | "CtrlC"
  | "CtrlD"
  | "CtrlZ"
  | "CtrlL"
  | "Hyphen"
  | "Slash"
  | "Pipe"
  | "Tilde";

export function toCtrlChar(ch: string): string {
  if (!ch) return "";
  const first = ch[0];
  const upper = first.toUpperCase();
  const code = upper.charCodeAt(0);
  if (code >= 65 && code <= 90) {
    return String.fromCharCode(code - 64);
  }
  switch (first) {
    case "@":
    case " ":
    case "2":
      return "\x00";
    case "[":
    case "3":
      return "\x1b";
    case "\\":
    case "4":
      return "\x1c";
    case "]":
    case "5":
      return "\x1d";
    case "^":
    case "6":
      return "\x1e";
    case "_":
    case "/":
    case "7":
      return "\x1f";
    case "?":
    case "8":
      return "\x7f";
    default:
      return first;
  }
}

export function applyModifiers(
  input: string,
  mods: ModifierState,
): { data: string; consumed: boolean } {
  if ((!mods.ctrl && !mods.alt) || input.length !== 1) {
    return { data: input, consumed: false };
  }
  let out = input;
  if (mods.ctrl) {
    out = toCtrlChar(out);
  }
  if (mods.alt) {
    out = `\x1b${out}`;
  }
  return { data: out, consumed: true };
}

export function specialKeySequence(
  key: SpecialKey,
  appCursor = false,
  mods: ModifierState = { ctrl: false, alt: false },
): string {
  const modParam =
    mods.ctrl && mods.alt ? 7 : mods.ctrl ? 5 : mods.alt ? 3 : 0;

  switch (key) {
    case "Esc":
      return "\x1b";
    case "Tab":
      return "\t";
    case "Up":
      if (modParam) return `\x1b[1;${modParam}A`;
      return appCursor ? "\x1bOA" : "\x1b[A";
    case "Down":
      if (modParam) return `\x1b[1;${modParam}B`;
      return appCursor ? "\x1bOB" : "\x1b[B";
    case "Right":
      if (modParam) return `\x1b[1;${modParam}C`;
      return appCursor ? "\x1bOC" : "\x1b[C";
    case "Left":
      if (modParam) return `\x1b[1;${modParam}D`;
      return appCursor ? "\x1bOD" : "\x1b[D";
    case "Home":
      if (modParam) return `\x1b[1;${modParam}H`;
      return appCursor ? "\x1bOH" : "\x1b[H";
    case "End":
      if (modParam) return `\x1b[1;${modParam}F`;
      return appCursor ? "\x1bOF" : "\x1b[F";
    case "PgUp":
      return "\x1b[5~";
    case "PgDn":
      return "\x1b[6~";
    case "CtrlC":
      return "\x03";
    case "CtrlD":
      return "\x04";
    case "CtrlZ":
      return "\x1a";
    case "CtrlL":
      return "\x0c";
    case "Hyphen":
      return mods.alt ? "\x1b-" : "-";
    case "Slash":
      return mods.ctrl ? "\x1f" : mods.alt ? "\x1b/" : "/";
    case "Pipe":
      return mods.alt ? "\x1b|" : "|";
    case "Tilde":
      return mods.alt ? "\x1b~" : "~";
  }
}
