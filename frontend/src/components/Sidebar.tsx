import { useMemo, useRef, useState } from "react";

import { serverColor } from "../format";
import { moveBeside, moveWithinGroup } from "../order";
import type { Bookmark, Server } from "../types";
import { Icon } from "./Icon";

const EXPANDED_KEY = "termdeck.expanded";

function loadExpanded(): Set<number> {
  try {
    const raw = localStorage.getItem(EXPANDED_KEY);
    return new Set(raw ? (JSON.parse(raw) as number[]) : []);
  } catch {
    return new Set();
  }
}

function saveExpanded(ids: Set<number>): void {
  try {
    localStorage.setItem(EXPANDED_KEY, JSON.stringify([...ids]));
  } catch {
    // Storage unavailable; expansion state just won't persist.
  }
}

interface SidebarProps {
  servers: Server[];
  bookmarks: Bookmark[];
  selectedServerId: number | null;
  liveServerIds: Set<number>;
  /** Paths that have an open terminal tab (shown with a marker). */
  openBookmarkIds: Set<number>;
  /** Path of the terminal in the focused pane. */
  activeBookmarkId: number | null;
  onSelectServer: (id: number) => void;
  /** newTab=false jumps to an already open tab for that path when there is one. */
  onOpen: (serverId: number, bookmarkId: number | null, newTab: boolean) => void;
  onEditServer: (server: Server) => void;
  onDeleteServer: (server: Server) => void;
  onAddBookmark: (server: Server) => void;
  onEditBookmark: (bookmark: Bookmark) => void;
  onDeleteBookmark: (bookmark: Bookmark) => void;
  onOpenContainers?: (serverId: number) => void;
  /** New display order of all servers (full id list). */
  onReorder: (ids: number[]) => void;
}

interface DropHint {
  id: number;
  after: boolean;
}

export function Sidebar(props: SidebarProps) {
  const { servers, bookmarks, selectedServerId, liveServerIds, openBookmarkIds, activeBookmarkId } = props;
  const [query, setQuery] = useState("");
  const [expanded, setExpanded] = useState<Set<number>>(loadExpanded);
  const [dragId, setDragId] = useState<number | null>(null);
  const [dropHint, setDropHint] = useState<DropHint | null>(null);
  // Drag handlers read refs: dragover/drop can fire before React re-renders with the new state.
  const dragRef = useRef<number | null>(null);
  const hintRef = useRef<DropHint | null>(null);

  const startDrag = (id: number) => {
    dragRef.current = id;
    setDragId(id);
  };

  const showHint = (hint: DropHint | null) => {
    hintRef.current = hint;
    setDropHint(hint);
  };

  const groupOf = (id: number) => servers.find((s) => s.id === id)?.group_name;

  const moveByKeyboard = (id: number, step: -1 | 1) => {
    const ids = moveWithinGroup(servers, id, step);
    if (!ids) return;
    props.onReorder(ids);
    // The row moves in the DOM; put focus back on it so repeated Alt+arrow keeps working.
    requestAnimationFrame(() => document.querySelector<HTMLElement>(`[data-server-main="${id}"]`)?.focus());
  };

  const endDrag = () => {
    dragRef.current = null;
    setDragId(null);
    showHint(null);
  };

  const toggle = (id: number) => {
    const opening = !expanded.has(id);
    setExpanded((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      saveExpanded(next);
      return next;
    });
    if (opening) {
      // Scroll the list so the paths that just appeared are in view.
      requestAnimationFrame(() => {
        const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
        document
          .querySelector(`[data-server-item="${id}"]`)
          ?.scrollIntoView({ block: "nearest", behavior: reduce ? "auto" : "smooth" });
      });
    }
  };

  const groups = useMemo(() => {
    const q = query.trim().toLowerCase();
    const byServer = new Map<number, Bookmark[]>();
    for (const bm of bookmarks) {
      const list = byServer.get(bm.server_id) ?? [];
      list.push(bm);
      byServer.set(bm.server_id, list);
    }
    const result = new Map<string, { server: Server; bookmarks: Bookmark[] }[]>();
    for (const server of servers) {
      const own = byServer.get(server.id) ?? [];
      const serverHit =
        !q || [server.name, server.host, server.username, server.group_name].some((v) => v.toLowerCase().includes(q));
      const matching = q ? own.filter((b) => `${b.name} ${b.path}`.toLowerCase().includes(q)) : own;
      if (!serverHit && matching.length === 0) continue;
      const list = result.get(server.group_name) ?? [];
      list.push({ server, bookmarks: serverHit ? own : matching });
      result.set(server.group_name, list);
    }
    return [...result.entries()].sort(([a], [b]) => (a === "" ? -1 : b === "" ? 1 : a.localeCompare(b)));
  }, [servers, bookmarks, query]);

  const searching = query.trim() !== "";

  return (
    <div className="tree">
      <label className="search">
        <Icon name="search" size={14} />
        <input
          type="search"
          placeholder="Cari server atau path"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
      </label>

      {servers.length === 0 && (
        <p className="empty-note">Belum ada server. Tambahkan server pertama untuk mulai membuka terminal.</p>
      )}
      {servers.length > 0 && groups.length === 0 && <p className="empty-note">Tidak ada yang cocok dengan “{query}”.</p>}

      {groups.map(([group, items]) => (
        <section key={group || "__none"} className="tree-group">
          {group && <h3 className="group-name">{group}</h3>}
          <ul>
            {items.map(({ server, bookmarks: own }) => {
              const open = searching || expanded.has(server.id);
              return (
                <li
                  key={server.id}
                  data-server-item={server.id}
                  className={`server-item${dragId === server.id ? " is-dragging" : ""}${
                    dropHint?.id === server.id ? (dropHint.after ? " drop-after" : " drop-before") : ""
                  }`}
                >
                  <div
                    className={`server-row${selectedServerId === server.id ? " is-selected" : ""}`}
                    style={{ ["--srv" as string]: serverColor(server.color) }}
                    draggable={!searching}
                    onDragStart={(e) => {
                      e.dataTransfer.effectAllowed = "move";
                      e.dataTransfer.setData("text/plain", String(server.id));
                      startDrag(server.id);
                    }}
                    onDragOver={(e) => {
                      const dragging = dragRef.current;
                      if (dragging === null || dragging === server.id || groupOf(dragging) !== server.group_name) return;
                      e.preventDefault();
                      e.dataTransfer.dropEffect = "move";
                      const rect = e.currentTarget.getBoundingClientRect();
                      const after = e.clientY > rect.top + rect.height / 2;
                      const hint = hintRef.current;
                      if (hint?.id !== server.id || hint.after !== after) showHint({ id: server.id, after });
                    }}
                    onDrop={(e) => {
                      e.preventDefault();
                      const dragging = dragRef.current;
                      const hint = hintRef.current;
                      if (dragging !== null && hint) {
                        const ids = moveBeside(servers, dragging, hint.id, hint.after);
                        if (ids) props.onReorder(ids);
                      }
                      endDrag();
                    }}
                    onDragEnd={endDrag}
                  >
                    {!searching && (
                      <span className="grip" aria-hidden="true" title="Seret untuk memindah posisi">
                        <Icon name="grip" size={12} />
                      </span>
                    )}
                    <button
                      type="button"
                      className={`chev${open ? " is-open" : ""}`}
                      onClick={() => toggle(server.id)}
                      aria-label={open ? "Tutup daftar path" : "Lihat daftar path"}
                      aria-expanded={open}
                    >
                      <Icon name="chevron" size={13} />
                    </button>
                    <button
                      type="button"
                      className="server-main"
                      data-server-main={server.id}
                      onClick={() => props.onSelectServer(server.id)}
                      onDoubleClick={() => props.onOpen(server.id, null, true)}
                      onKeyDown={(e) => {
                        if (!e.altKey || searching) return;
                        if (e.key === "ArrowUp" || e.key === "ArrowDown") {
                          e.preventDefault();
                          moveByKeyboard(server.id, e.key === "ArrowUp" ? -1 : 1);
                        }
                      }}
                      title={`${server.name} (${server.username}@${server.host}${server.port !== 22 ? `:${server.port}` : ""}) — Klik untuk info, klik ganda untuk buka terminal`}
                    >
                      <span className={`dot${liveServerIds.has(server.id) ? " is-live" : ""}`} />
                      <span className="server-mini-icon" aria-hidden="true">
                        <Icon name="server" size={16} />
                      </span>
                      <span className="server-text">
                        <span className="server-name">{server.name}</span>
                        <span className="server-sub">
                          {server.username}@{server.host}
                          {server.port !== 22 ? `:${server.port}` : ""}
                        </span>
                      </span>
                    </button>
                    <span className="row-actions">
                      <button
                        type="button"
                        className="icon-btn"
                        onClick={() => props.onOpen(server.id, null, true)}
                        title="Buka terminal baru"
                        aria-label={`Buka terminal ${server.name}`}
                      >
                        <Icon name="terminal" size={14} />
                      </button>
                      <button
                        type="button"
                        className="icon-btn"
                        onClick={() => props.onAddBookmark(server)}
                        title="Simpan path"
                        aria-label={`Simpan path di ${server.name}`}
                      >
                        <Icon name="plus" size={14} />
                      </button>
                      {props.onOpenContainers && (
                        <button
                          type="button"
                          className="icon-btn"
                          onClick={(e) => {
                            e.stopPropagation();
                            props.onOpenContainers?.(server.id);
                          }}
                          title="Buka Containers (Podman / Docker)"
                          aria-label={`Buka containers di ${server.name}`}
                        >
                          <Icon name="box" size={14} />
                        </button>
                      )}
                      <button
                        type="button"
                        className="icon-btn"
                        onClick={() => props.onEditServer(server)}
                        title="Ubah server"
                        aria-label={`Ubah ${server.name}`}
                      >
                        <Icon name="edit" size={14} />
                      </button>
                      <button
                        type="button"
                        className="icon-btn danger"
                        onClick={() => props.onDeleteServer(server)}
                        title="Hapus server"
                        aria-label={`Hapus ${server.name}`}
                      >
                        <Icon name="trash" size={14} />
                      </button>
                    </span>
                  </div>

                  {open && (
                    <ul className="bookmarks">
                      {own.map((bm) => (
                        <li
                          key={bm.id}
                          className={`bookmark-row${openBookmarkIds.has(bm.id) ? " is-open" : ""}${
                            activeBookmarkId === bm.id ? " is-active" : ""
                          }`}
                        >
                          <button
                            type="button"
                            className="bookmark-main"
                            onClick={() => props.onOpen(server.id, bm.id, false)}
                            title={
                              openBookmarkIds.has(bm.id) ? `Lompat ke terminal ${bm.path}` : `Buka ${bm.path}`
                            }
                          >
                            <span className="bookmark-icon-wrap">
                              <span className="bookmark-icon">
                                <Icon name="folder" size={14} />
                              </span>
                              {openBookmarkIds.has(bm.id) && <span className="open-dot" aria-label="Terbuka" />}
                            </span>
                            <span className="bookmark-text">
                              <span className="bookmark-name">{bm.name}</span>
                              <span className="bookmark-path" title={bm.path}>{bm.path}</span>
                            </span>
                          </button>
                          <span className="row-actions">
                            <button
                              type="button"
                              className="icon-btn"
                              onClick={() => props.onOpen(server.id, bm.id, true)}
                              aria-label={`Buka ${bm.name} di tab baru`}
                              title="Buka di tab baru"
                            >
                              <Icon name="newTab" size={14} />
                            </button>
                            <button
                              type="button"
                              className="icon-btn"
                              onClick={() => props.onEditBookmark(bm)}
                              aria-label={`Ubah path ${bm.name}`}
                              title="Ubah path"
                            >
                              <Icon name="edit" size={14} />
                            </button>
                            <button
                              type="button"
                              className="icon-btn danger"
                              onClick={() => props.onDeleteBookmark(bm)}
                              aria-label={`Hapus path ${bm.name}`}
                              title="Hapus path"
                            >
                              <Icon name="trash" size={14} />
                            </button>
                          </span>
                        </li>
                      ))}
                      {own.length === 0 && (
                        <li>
                          <button type="button" className="add-path" onClick={() => props.onAddBookmark(server)}>
                            <Icon name="plus" size={14} /> Simpan path pertama
                          </button>
                        </li>
                      )}
                    </ul>
                  )}
                </li>
              );
            })}
          </ul>
        </section>
      ))}
    </div>
  );
}
