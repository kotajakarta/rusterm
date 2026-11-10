import { useEffect, useMemo, useRef, useState } from "react";

import { serverColor } from "../format";
import type { Bookmark, Server } from "../types";
import { Icon } from "./Icon";

interface PaletteItem {
  key: string;
  serverId: number;
  bookmarkId: number | null;
  title: string;
  detail: string;
  color: string;
  haystack: string;
}

interface CommandPaletteProps {
  servers: Server[];
  bookmarks: Bookmark[];
  onPick: (serverId: number, bookmarkId: number | null) => void;
  onClose: () => void;
  title?: string;
  placeholder?: string;
  /** Only list paths of this server and leave servers out (used for "Pindah path"). */
  onlyServerId?: number;
}

export function CommandPalette({
  servers,
  bookmarks,
  onPick,
  onClose,
  title = "Lompat ke proyek",
  placeholder = "Ketik nama server, proyek, atau path",
  onlyServerId,
}: CommandPaletteProps) {
  const [query, setQuery] = useState("");
  const [cursor, setCursor] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLUListElement>(null);

  const items = useMemo<PaletteItem[]>(() => {
    const byId = new Map(servers.map((s) => [s.id, s]));
    const result: PaletteItem[] = [];
    for (const bm of bookmarks) {
      const server = byId.get(bm.server_id);
      if (!server || (onlyServerId !== undefined && server.id !== onlyServerId)) continue;
      result.push({
        key: `b${bm.id}`,
        serverId: server.id,
        bookmarkId: bm.id,
        title: `${server.name} / ${bm.name}`,
        detail: bm.path,
        color: server.color,
        haystack: `${server.name} ${server.host} ${bm.name} ${bm.path}`.toLowerCase(),
      });
    }
    for (const server of onlyServerId !== undefined ? [] : servers) {
      result.push({
        key: `s${server.id}`,
        serverId: server.id,
        bookmarkId: null,
        title: server.name,
        detail: `${server.username}@${server.host}`,
        color: server.color,
        haystack: `${server.name} ${server.host} ${server.group_name}`.toLowerCase(),
      });
    }
    return result;
  }, [servers, bookmarks, onlyServerId]);

  const filtered = useMemo(() => {
    const words = query.toLowerCase().split(/\s+/).filter(Boolean);
    return words.length ? items.filter((item) => words.every((w) => item.haystack.includes(w))) : items;
  }, [items, query]);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  useEffect(() => {
    setCursor(0);
  }, [query]);

  useEffect(() => {
    listRef.current?.querySelector(".is-cursor")?.scrollIntoView({ block: "nearest" });
  }, [cursor]);

  const pick = (item: PaletteItem | undefined) => {
    if (!item) return;
    onPick(item.serverId, item.bookmarkId);
  };

  return (
    <div className="modal-backdrop palette-backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="palette" role="dialog" aria-modal="true" aria-label={title}>
        <p className="palette-title-bar">{title}</p>
        <label className="palette-input">
          <Icon name="search" size={16} />
          <input
            ref={inputRef}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={placeholder}
            aria-controls="palette-list"
            aria-activedescendant={filtered[cursor] ? `pal-${filtered[cursor].key}` : undefined}
            onKeyDown={(e) => {
              if (e.key === "ArrowDown") {
                e.preventDefault();
                setCursor((c) => Math.min(c + 1, filtered.length - 1));
              } else if (e.key === "ArrowUp") {
                e.preventDefault();
                setCursor((c) => Math.max(c - 1, 0));
              } else if (e.key === "Enter") {
                e.preventDefault();
                pick(filtered[cursor]);
              } else if (e.key === "Escape") {
                e.preventDefault();
                onClose();
              }
            }}
          />
        </label>
        <ul id="palette-list" ref={listRef} className="palette-list" role="listbox">
          {filtered.length === 0 && (
            <li className="picker-note">
              {items.length === 0 ? "Belum ada path tersimpan untuk server ini." : "Tidak ada yang cocok."}
            </li>
          )}
          {filtered.map((item, idx) => (
            <li
              key={item.key}
              id={`pal-${item.key}`}
              role="option"
              aria-selected={idx === cursor}
              className={`palette-item${idx === cursor ? " is-cursor" : ""}`}
              style={{ ["--srv" as string]: serverColor(item.color) }}
              onMouseEnter={() => setCursor(idx)}
              onClick={() => pick(item)}
            >
              <Icon name={item.bookmarkId === null ? "server" : "folder"} size={15} />
              <span className="palette-title">{item.title}</span>
              <span className="palette-detail">{item.detail}</span>
            </li>
          ))}
        </ul>
      </div>
    </div>
  );
}
