// Image paste for terminal tools (Claude Code, Antigravity, ...): the image is
// uploaded to the server and its path is pasted, since those CLIs cannot read
// the browser clipboard.

interface TypedFile {
  type: string;
}

/** Images to upload; clipboard text wins so copying from apps that add a picture of the text still pastes text. */
export function pickImages<T extends TypedFile>(files: ArrayLike<T>, text: string): T[] {
  if (text.trim() !== "") return [];
  return Array.from(files).filter((file) => file.type.startsWith("image/"));
}

/** Space-separated paths, spaces escaped the way terminals paste dropped files. */
export function formatPastedPaths(paths: string[]): string {
  return paths.map((path) => path.replace(/ /g, "\\ ")).join(" ");
}
