import { type FormEvent, useEffect, useMemo, useRef, useState } from "react";
import { api } from "../../api";
import {
  buildReplaceTargets,
  countSelectedMatches,
  DEFAULT_SEARCH_EXCLUDES,
  parsePatternList,
  splitMatchLine
} from "../../fileSearch";
import type { FileReplaceOut, FileSearchFileResult, FileSearchOut } from "../../types";
import {
  ChevronRightIcon,
  FileEditIcon,
  FolderIcon,
  SearchIcon,
  Settings2Icon,
  XIcon
} from "./FileIcons";

export interface FindReplaceModalProps {
  serverId: number;
  initialPath: string;
  onClose: () => void;
  onOpenFile: (file: { path: string; relPath: string; mtime: number }) => void;
  onReplaced: () => void;
}

const DEFAULT_EXCLUDE_TEXT = DEFAULT_SEARCH_EXCLUDES.join(", ");

export function FindReplaceModal({
  serverId,
  initialPath,
  onClose,
  onOpenFile,
  onReplaced
}: FindReplaceModalProps): JSX.Element {
  const [targetPath, setTargetPath] = useState(initialPath);
  const [query, setQuery] = useState("");
  const [replacement, setReplacement] = useState("");
  const [caseSensitive, setCaseSensitive] = useState(false);
  const [wholeWord, setWholeWord] = useState(false);
  const [useRegex, setUseRegex] = useState(false);
  const [showFilters, setShowFilters] = useState(false);
  const [includeText, setIncludeText] = useState("");
  const [excludeText, setExcludeText] = useState(DEFAULT_EXCLUDE_TEXT);
  const [useGitignore, setUseGitignore] = useState(true);

  const [searching, setSearching] = useState(false);
  const [replacing, setReplacing] = useState(false);
  const [confirmReplace, setConfirmReplace] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<FileSearchOut | null>(null);
  const [replaceSummary, setReplaceSummary] = useState<FileReplaceOut | null>(null);
  const [selectedFiles, setSelectedFiles] = useState<Set<string>>(new Set());
  const [collapsedFiles, setCollapsedFiles] = useState<Set<string>>(new Set());

  const findInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    const t = setTimeout(() => {
      findInputRef.current?.focus();
    }, 30);
    return () => clearTimeout(t);
  }, []);

  useEffect(() => {
    const onKey = (ev: KeyboardEvent) => {
      if (ev.key === "Escape" && !searching && !replacing) {
        ev.preventDefault();
        onClose();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [searching, replacing, onClose]);

  const busy = searching || replacing;

  const runSearch = async (clearReplaceSummary = true): Promise<void> => {
    if (!query) {
      setError("Enter a search query first.");
      return;
    }
    const cleanTarget = targetPath.trim();
    if (!cleanTarget || !cleanTarget.startsWith("/")) {
      setError("Target folder must be an absolute path starting with '/'.");
      return;
    }
    setSearching(true);
    setError(null);
    setConfirmReplace(false);
    if (clearReplaceSummary) setReplaceSummary(null);
    try {
      const include = parsePatternList(includeText);
      const exclude = parsePatternList(excludeText);
      const res = await api.filesSearch(serverId, {
        path: cleanTarget,
        query,
        case_sensitive: caseSensitive,
        whole_word: wholeWord,
        use_regex: useRegex,
        include: include.length > 0 ? include : undefined,
        exclude,
        use_gitignore: useGitignore
      });
      setResult(res);
      setSelectedFiles(new Set(res.files.map((f) => f.path)));
      setCollapsedFiles(new Set());
    } catch (e) {
      setError(e instanceof Error ? e.message : "Search failed");
    } finally {
      setSearching(false);
    }
  };

  const handleSubmit = (ev: FormEvent): void => {
    ev.preventDefault();
    void runSearch(true);
  };

  const executeReplace = async (): Promise<void> => {
    if (!result || selectedFiles.size === 0 || !query) return;
    setReplacing(true);
    setError(null);
    setConfirmReplace(false);
    try {
      const targets = buildReplaceTargets(result.files, selectedFiles);
      const include = parsePatternList(includeText);
      const exclude = parsePatternList(excludeText);
      const rep = await api.filesReplace(serverId, {
        path: result.path,
        query,
        replacement,
        case_sensitive: caseSensitive,
        whole_word: wholeWord,
        use_regex: useRegex,
        include: include.length > 0 ? include : undefined,
        exclude,
        use_gitignore: useGitignore,
        targets
      });
      setReplaceSummary(rep);
      if (rep.files_modified > 0) {
        onReplaced();
      }
      // Refresh search results so displayed matches & mtimes stay accurate
      await runSearch(false);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Replace failed");
    } finally {
      setReplacing(false);
    }
  };

  const selectedCount = selectedFiles.size;
  const selectedMatchCount = useMemo(
    () => (result ? countSelectedMatches(result.files, selectedFiles) : 0),
    [result, selectedFiles]
  );

  const toggleFile = (filePath: string): void => {
    setConfirmReplace(false);
    setSelectedFiles((prev) => {
      const next = new Set(prev);
      if (next.has(filePath)) next.delete(filePath);
      else next.add(filePath);
      return next;
    });
  };

  const toggleCollapse = (filePath: string): void => {
    setCollapsedFiles((prev) => {
      const next = new Set(prev);
      if (next.has(filePath)) next.delete(filePath);
      else next.add(filePath);
      return next;
    });
  };

  const allSelected = Boolean(result && result.files.length > 0 && selectedCount === result.files.length);

  const toggleSelectAll = (): void => {
    if (!result) return;
    setConfirmReplace(false);
    if (allSelected) {
      setSelectedFiles(new Set());
    } else {
      setSelectedFiles(new Set(result.files.map((f) => f.path)));
    }
  };

  const activeExcludeCount = useMemo(() => parsePatternList(excludeText).length, [excludeText]);

  const renderMatchRow = (
    file: FileSearchFileResult,
    m: FileSearchFileResult["matches"][number],
    idx: number
  ) => {
    const parts = splitMatchLine(m.text, m.match_start, m.match_end);
    const showPreview = replacement.length > 0 && !useRegex;
    return (
      <button
        key={`${file.path}:${m.line}:${m.col}:${idx}`}
        type="button"
        className="fm-search-match-row"
        onClick={() => onOpenFile({ path: file.path, relPath: file.rel_path, mtime: file.mtime })}
        title={`Open ${file.rel_path} (line ${m.line})`}
      >
        <span className="fm-search-lineno">{m.line}</span>
        <code className="fm-search-code">
          {parts.before}
          <mark className={`fm-search-hit ${showPreview ? "is-replaced" : ""}`}>{parts.match}</mark>
          {showPreview && <span className="fm-search-rep-preview">{replacement}</span>}
          {parts.after}
        </code>
      </button>
    );
  };

  return (
    <div
      className="fm-modal-backdrop"
      onMouseDown={(ev) => {
        if (ev.target === ev.currentTarget && !busy) onClose();
      }}
    >
      <div
        className="fm-modal fm-modal-lg fm-search-modal"
        role="dialog"
        aria-modal="true"
        aria-label="Find and Replace in Files"
      >
        <div className="fm-modal-header">
          <div className="fm-search-title-wrap">
            <SearchIcon size={15} />
            <span className="fm-modal-title">Find &amp; Replace in Files</span>
            <span className="fm-search-scope-pill" title={targetPath}>
              <FolderIcon size={12} />
              <span>{targetPath}</span>
            </span>
          </div>
          <button type="button" className="fm-icon-btn" onClick={onClose} disabled={busy}>
            <XIcon size={15} />
          </button>
        </div>

        <form className="fm-search-controls" onSubmit={handleSubmit}>
          <div className="fm-search-row">
            <div className="fm-search-input-wrap">
              <input
                ref={findInputRef}
                type="text"
                className="fm-input"
                placeholder="Find in files…"
                value={query}
                onChange={(e) => {
                  setQuery(e.target.value);
                  setConfirmReplace(false);
                }}
                disabled={busy}
              />
              <div className="fm-search-flags">
                <button
                  type="button"
                  className={`fm-flag-btn ${caseSensitive ? "active" : ""}`}
                  onClick={() => setCaseSensitive((v) => !v)}
                  title="Match Case"
                  disabled={busy}
                >
                  Aa
                </button>
                <button
                  type="button"
                  className={`fm-flag-btn ${wholeWord ? "active" : ""}`}
                  onClick={() => setWholeWord((v) => !v)}
                  title="Match Whole Word"
                  disabled={busy}
                >
                  ab
                </button>
                <button
                  type="button"
                  className={`fm-flag-btn ${useRegex ? "active" : ""}`}
                  onClick={() => setUseRegex((v) => !v)}
                  title="Use Regular Expression"
                  disabled={busy}
                >
                  .*
                </button>
              </div>
            </div>
            <button
              type="submit"
              className="fm-btn fm-btn-primary"
              disabled={busy || !query}
            >
              <SearchIcon size={14} />
              <span>{searching ? "Searching…" : "Find"}</span>
            </button>
          </div>

          <div className="fm-search-row">
            <input
              type="text"
              className="fm-input"
              placeholder="Replace with… (leave empty to delete matches)"
              value={replacement}
              onChange={(e) => {
                setReplacement(e.target.value);
                setConfirmReplace(false);
              }}
              disabled={busy}
            />
            <button
              type="button"
              className={`fm-btn ${showFilters ? "fm-btn-primary" : ""}`}
              onClick={() => setShowFilters((v) => !v)}
              title="Folder, include & exclude filters"
              disabled={busy}
            >
              <Settings2Icon size={14} />
              <span>Filters ({activeExcludeCount})</span>
            </button>
          </div>

          {showFilters && (
            <div className="fm-search-filters">
              <div className="fm-search-filter-grid">
                <label className="fm-field">
                  <span className="fm-label">Target folder</span>
                  <input
                    type="text"
                    className="fm-input fm-input-sm"
                    value={targetPath}
                    onChange={(e) => setTargetPath(e.target.value)}
                    disabled={busy}
                  />
                </label>
                <label className="fm-field">
                  <span className="fm-label">Include files (comma-separated globs, optional)</span>
                  <input
                    type="text"
                    className="fm-input fm-input-sm"
                    placeholder="e.g. *.ts, *.tsx, *.rs"
                    value={includeText}
                    onChange={(e) => setIncludeText(e.target.value)}
                    disabled={busy}
                  />
                </label>
              </div>
              <label className="fm-field">
                <div className="fm-label-row">
                  <span className="fm-label">
                    Exclude folders / patterns (comma-separated)
                  </span>
                  <button
                    type="button"
                    className="fm-btn fm-btn-xs"
                    onClick={() => setExcludeText(DEFAULT_EXCLUDE_TEXT)}
                    disabled={busy}
                  >
                    Reset defaults
                  </button>
                </div>
                <textarea
                  className="fm-textarea fm-search-exclude-area"
                  rows={2}
                  value={excludeText}
                  onChange={(e) => setExcludeText(e.target.value)}
                  disabled={busy}
                />
              </label>
              <label className="fm-search-check">
                <input
                  type="checkbox"
                  checked={useGitignore}
                  onChange={(e) => setUseGitignore(e.target.checked)}
                  disabled={busy}
                />
                <span>Also exclude rules from .gitignore</span>
              </label>
            </div>
          )}
        </form>

        <div className="fm-modal-body fm-search-body">
          {error && <div className="fm-alert fm-alert-error">{error}</div>}

          {replaceSummary && (
            <div
              className={`fm-alert ${
                replaceSummary.errors.length > 0 ? "fm-alert-warn" : "fm-alert-ok"
              }`}
            >
              <div>
                <strong>
                  Replaced {replaceSummary.total_replacements} occurrence(s) in{" "}
                  {replaceSummary.files_modified} file(s).
                </strong>
                {replaceSummary.errors.length > 0 && (
                  <div className="fm-search-err-list">
                    {replaceSummary.errors.map((e, i) => (
                      <div key={`${e.path}-${i}`}>
                        {e.path}: {e.error}
                      </div>
                    ))}
                  </div>
                )}
              </div>
              <button
                type="button"
                className="fm-icon-btn"
                onClick={() => setReplaceSummary(null)}
              >
                <XIcon size={12} />
              </button>
            </div>
          )}

          {!result && !searching && (
            <div className="fm-search-empty">
              Enter text above and click <strong>Find</strong> to scan text files in{" "}
              <code>{targetPath}</code>. Common folders like <code>node_modules</code> and{" "}
              <code>.git</code> are excluded automatically.
            </div>
          )}

          {searching && !result && (
            <div className="fm-search-empty">Scanning files in {targetPath}…</div>
          )}

          {result && (
            <>
              <div className="fm-search-summary-bar">
                <div className="fm-search-stats">
                  <span>
                    <strong>{result.total_matches}</strong> match
                    {result.total_matches === 1 ? "" : "es"} in{" "}
                    <strong>{result.files_matched}</strong> file
                    {result.files_matched === 1 ? "" : "s"}
                  </span>
                  <span className="fm-search-muted">
                    ({result.files_scanned} scanned
                    {result.skipped_binary > 0 ? `, ${result.skipped_binary} binary skipped` : ""}
                    {result.skipped_large > 0 ? `, ${result.skipped_large} large skipped` : ""})
                  </span>
                  {result.truncated && (
                    <span className="fm-search-trunc-badge">
                      Result limit reached — refine query or filters
                    </span>
                  )}
                </div>

                {result.files.length > 0 && (
                  <label className="fm-search-check">
                    <input
                      type="checkbox"
                      checked={allSelected}
                      onChange={toggleSelectAll}
                      disabled={busy}
                    />
                    <span>
                      Select all ({selectedCount}/{result.files.length})
                    </span>
                  </label>
                )}
              </div>

              {result.files.length === 0 ? (
                <div className="fm-search-empty">No matches found for &ldquo;{query}&rdquo;.</div>
              ) : (
                <div className="fm-search-results">
                  {result.files.map((file) => {
                    const isChecked = selectedFiles.has(file.path);
                    const isCollapsed = collapsedFiles.has(file.path);
                    return (
                      <div key={file.path} className="fm-search-file-group">
                        <div className="fm-search-file-head">
                          <input
                            type="checkbox"
                            checked={isChecked}
                            onChange={() => toggleFile(file.path)}
                            disabled={busy}
                            aria-label={`Select ${file.rel_path}`}
                          />
                          <button
                            type="button"
                            className="fm-search-file-toggle"
                            onClick={() => toggleCollapse(file.path)}
                          >
                            <ChevronRightIcon
                              size={13}
                              className={`fm-search-chev ${isCollapsed ? "" : "open"}`}
                            />
                            <span className="fm-search-file-path" title={file.path}>
                              {file.rel_path}
                            </span>
                            <span className="fm-search-count-badge">{file.match_count}</span>
                          </button>
                          <button
                            type="button"
                            className="fm-btn fm-btn-xs"
                            onClick={() =>
                              onOpenFile({
                                path: file.path,
                                relPath: file.rel_path,
                                mtime: file.mtime
                              })
                            }
                            title="Open file in editor"
                          >
                            <FileEditIcon size={12} />
                            <span>Edit</span>
                          </button>
                        </div>
                        {!isCollapsed && (
                          <div className="fm-search-file-matches">
                            {file.matches.map((m, idx) => renderMatchRow(file, m, idx))}
                          </div>
                        )}
                      </div>
                    );
                  })}
                </div>
              )}
            </>
          )}
        </div>

        <div className="fm-modal-footer fm-search-footer">
          <div className="fm-search-footer-left">
            {confirmReplace && selectedCount > 0 ? (
              <span className="fm-search-confirm-text">
                Replace <strong>{selectedMatchCount}</strong> match
                {selectedMatchCount === 1 ? "" : "es"} in <strong>{selectedCount}</strong> file
                {selectedCount === 1 ? "" : "s"}?
              </span>
            ) : (
              <span className="fm-search-muted">
                Click any match line to open the file in the editor.
              </span>
            )}
          </div>
          <div className="fm-search-footer-actions">
            {confirmReplace ? (
              <>
                <button
                  type="button"
                  className="fm-btn"
                  onClick={() => setConfirmReplace(false)}
                  disabled={busy}
                >
                  Cancel
                </button>
                <button
                  type="button"
                  className="fm-btn fm-btn-danger"
                  onClick={() => void executeReplace()}
                  disabled={busy || selectedCount === 0}
                >
                  {replacing ? "Replacing…" : "Confirm Replace"}
                </button>
              </>
            ) : (
              <>
                <button type="button" className="fm-btn" onClick={onClose} disabled={busy}>
                  Close
                </button>
                <button
                  type="button"
                  className="fm-btn fm-btn-primary"
                  onClick={() => setConfirmReplace(true)}
                  disabled={busy || !result || selectedCount === 0}
                >
                  Replace Selected ({selectedCount})
                </button>
              </>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
