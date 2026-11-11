"""
File-manager agent executed ON THE MANAGED NODE as `python3 -c <this source>`.

Stdlib only (runs on any node with python3). Reads one JSON request from stdin, writes one JSON
response to stdout. Paths travel inside JSON, never on a command line, so no shell quoting is
involved. Non-UTF-8 filenames round-trip via surrogateescape.
"""
import grp
import json
import os
import pwd
import re
import shutil
import stat
import struct
import sys
import tarfile
import time

MAX_READ_BYTES = 2 * 1024 * 1024  # editor limit; downloads stream separately

# Deleting/moving these would brick the system; Cockpit asks twice, we refuse outright.
PROTECTED = {
    "/", "/bin", "/boot", "/dev", "/etc", "/home", "/lib", "/lib64", "/opt", "/proc", "/root",
    "/run", "/sbin", "/srv", "/sys", "/tmp", "/usr", "/var",
}

class AgentError(Exception):
    pass

def fs(p):
    return os.fsencode(p)

def norm(p):
    if not isinstance(p, str) or not p.startswith("/") or "\x00" in p:
        raise AgentError("Path must be absolute")
    return os.path.normpath(p) if p != "/" else "/"

def guard(p):
    if p in PROTECTED:
        raise AgentError("Refusing to modify protected system path: " + p)

_users, _groups = {}, {}

def user_name(uid):
    if uid not in _users:
        try:
            _users[uid] = pwd.getpwuid(uid).pw_name
        except KeyError:
            _users[uid] = str(uid)
    return _users[uid]

def group_name(gid):
    if gid not in _groups:
        try:
            _groups[gid] = grp.getgrgid(gid).gr_name
        except KeyError:
            _groups[gid] = str(gid)
    return _groups[gid]

def kind(mode):
    if stat.S_ISDIR(mode):
        return "directory"
    if stat.S_ISREG(mode):
        return "file"
    if stat.S_ISLNK(mode):
        return "link"
    if stat.S_ISBLK(mode) or stat.S_ISCHR(mode):
        return "device"
    if stat.S_ISFIFO(mode):
        return "fifo"
    if stat.S_ISSOCK(mode):
        return "socket"
    return "other"

def describe(path, name=None):
    st = os.lstat(fs(path))
    entry = {
        "name": name if name is not None else (os.path.basename(path) or "/"),
        "path": path,
        "type": kind(st.st_mode),
        "size": st.st_size,
        "mtime": st.st_mtime,
        "mode": stat.S_IMODE(st.st_mode),
        "perms": stat.filemode(st.st_mode),
        "owner": user_name(st.st_uid),
        "group": group_name(st.st_gid),
    }
    if entry["type"] == "link":
        entry["target"] = os.fsdecode(os.readlink(fs(path)))
        try:
            entry["target_type"] = kind(os.stat(fs(path)).st_mode)
        except OSError:
            entry["target_type"] = "broken"
    return entry

def op_list(req):
    path = norm(req["path"])
    show_hidden = bool(req.get("show_hidden"))
    entries = []
    with os.scandir(fs(path)) as it:
        for de in it:
            name = os.fsdecode(de.name)
            if not show_hidden and name.startswith("."):
                continue
            try:
                entries.append(describe(os.path.join(path, name), name))
            except OSError:
                continue  # vanished while listing
    return {"path": path, "entries": entries, "self": describe(path)}

def op_stat(req):
    return describe(norm(req["path"]))

def op_read(req):
    path = norm(req["path"])
    st = os.stat(fs(path))
    if not stat.S_ISREG(st.st_mode):
        raise AgentError("Not a regular file")
    if st.st_size > MAX_READ_BYTES:
        raise AgentError("File is too large to edit here (%d bytes); download it instead" % st.st_size)
    with open(fs(path), "rb") as f:
        data = f.read()
    if b"\x00" in data[:8192]:
        raise AgentError("Binary file; download it instead")
    try:
        text = data.decode("utf-8")
    except UnicodeDecodeError:
        raise AgentError("File is not valid UTF-8 text; download it instead")
    return {"path": path, "content": text, "mtime": st.st_mtime, "size": st.st_size}

def _private_tmp_name(directory):
    """Unguessable temp path; always open it with O_CREAT|O_EXCL|O_NOFOLLOW."""
    return os.path.join(directory, ".tukang-tmp-" + os.urandom(8).hex())

def op_write(req):
    path = norm(req["path"])
    content = req["content"].encode("utf-8")
    exists = os.path.lexists(fs(path))
    if req.get("create") and exists:
        raise AgentError("A file with that name already exists")
    if exists:
        st = os.stat(fs(path))
        if not stat.S_ISREG(st.st_mode):
            raise AgentError("Not a regular file")
        # Optimistic concurrency: refuse to clobber a file changed since it was opened
        expected = req.get("expected_mtime")
        if expected is not None and abs(st.st_mtime - float(expected)) > 1e-6:
            raise AgentError("File changed on disk since it was opened; reload it first")
    tmp = _private_tmp_name(os.path.dirname(path))
    fd = os.open(fs(tmp), os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o644)
    try:
        with os.fdopen(fd, "wb") as f:
            f.write(content)
            if exists:
                try:
                    os.fchown(f.fileno(), st.st_uid, st.st_gid)
                except OSError:
                    pass
                os.fchmod(f.fileno(), stat.S_IMODE(st.st_mode))
        os.replace(fs(tmp), fs(path))
    except BaseException:
        if os.path.lexists(fs(tmp)):
            os.unlink(fs(tmp))
        raise
    return describe(path)

def op_mkdir(req):
    path = norm(req["path"])
    os.mkdir(fs(path), 0o755)
    return describe(path)

def op_symlink(req):
    path = norm(req["path"])
    target = req["target"]
    if not target or "\x00" in target:
        raise AgentError("Invalid link target")
    os.symlink(fs(target), fs(path))
    return describe(path)

def op_rename(req):
    src, dst = norm(req["path"]), norm(req["new_path"])
    guard(src)
    if os.path.lexists(fs(dst)):
        raise AgentError("Destination already exists")
    os.rename(fs(src), fs(dst))
    return describe(dst)

def _unique_dest(dest_dir, name):
    candidate = os.path.join(dest_dir, name)
    n = 1
    while os.path.lexists(fs(candidate)):
        base, ext = os.path.splitext(name)
        candidate = os.path.join(dest_dir, "%s (copy%s)%s" % (base, "" if n == 1 else " %d" % n, ext))
        n += 1
    return candidate

def _copy_file(src, dst):
    if stat.S_ISLNK(os.lstat(fs(src)).st_mode):
        os.symlink(os.readlink(fs(src)), fs(dst))
        return dst
    with open(fs(src), "rb") as fi:
        st = os.fstat(fi.fileno())
        if not stat.S_ISREG(st.st_mode):
            raise AgentError("Cannot copy special file: " + os.fsdecode(src))
        fd = os.open(fs(dst), os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
        with os.fdopen(fd, "wb") as fo:
            shutil.copyfileobj(fi, fo, 1024 * 1024)
            fo.flush()
            os.fchmod(fo.fileno(), stat.S_IMODE(st.st_mode))
            os.utime(fo.fileno(), ns=(st.st_atime_ns, st.st_mtime_ns))
    return dst

def op_paste(req):
    dest_dir = norm(req["dest_dir"])
    move = req.get("mode") == "move"
    if not os.path.isdir(fs(dest_dir)):
        raise AgentError("Destination is not a directory")
    done = []
    for raw in req["paths"]:
        src = norm(raw)
        if move:
            guard(src)
        if dest_dir == src or dest_dir.startswith(src.rstrip("/") + "/"):
            raise AgentError("Cannot paste a directory into itself")
        name = os.path.basename(src)
        if move and os.path.dirname(src) == dest_dir:
            continue
        dst = _unique_dest(dest_dir, name)
        if move:
            shutil.move(fs(src), fs(dst))
        elif os.path.isdir(fs(src)) and not os.path.islink(fs(src)):
            shutil.copytree(fs(src), fs(dst), symlinks=True, copy_function=_copy_file)
        else:
            _copy_file(src, dst)
        done.append(dst)
    return {"paths": done}

def op_delete(req):
    for raw in req["paths"]:
        path = norm(raw)
        guard(path)
        if os.path.isdir(fs(path)) and not os.path.islink(fs(path)):
            shutil.rmtree(fs(path))
        else:
            os.unlink(fs(path))
    return {"deleted": len(req["paths"])}

def _resolve_ids(owner, group):
    uid = gid = -1
    if owner:
        uid = int(owner) if str(owner).isdigit() else pwd.getpwnam(owner).pw_uid
    if group:
        gid = int(group) if str(group).isdigit() else grp.getgrnam(group).gr_gid
    return uid, gid

def op_chmod(req):
    mode = req.get("mode")
    uid, gid = _resolve_ids(req.get("owner"), req.get("group"))
    recursive = bool(req.get("recursive"))
    if mode is not None and not (0 <= int(mode) <= 0o7777):
        raise AgentError("Invalid mode")

    def apply(p, is_dir):
        if mode is not None:
            m = int(mode)
            if recursive and not is_dir:
                m &= ~0o111 | (os.lstat(p).st_mode & 0o111)
            os.chmod(p, m)
        if uid != -1 or gid != -1:
            try:
                os.lchown(p, uid, gid)
            except PermissionError:
                if uid != os.lstat(p).st_uid or gid != os.lstat(p).st_gid:
                    raise

    for raw in req["paths"]:
        path = fs(norm(raw))
        if os.path.islink(path):
            if uid != -1 or gid != -1:
                os.lchown(path, uid, gid)
            continue
        is_dir = os.path.isdir(path)
        apply(path, is_dir)
        if recursive and is_dir:
            for root, dirs, files in os.walk(path):
                for d in dirs:
                    p = os.path.join(root, d)
                    if not os.path.islink(p):
                        apply(p, True)
                for f in files:
                    p = os.path.join(root, f)
                    if not os.path.islink(p):
                        apply(p, False)
    return {"paths": req["paths"]}

def _read_exact(stream, n):
    data = stream.read(n)
    if len(data) != n:
        raise AgentError("Upload interrupted")
    return data

def op_upload(req, stream):
    directory = norm(req["dir"])
    name = req["name"]
    if not isinstance(name, str) or name in ("", ".", "..") or "/" in name or "\x00" in name:
        raise AgentError("Invalid file name")
    dest = os.path.join(directory, name)
    dir_st = os.stat(fs(directory))
    if not stat.S_ISDIR(dir_st.st_mode):
        raise AgentError("Not a directory")
    if not req.get("overwrite") and os.path.lexists(fs(dest)):
        raise FileExistsError(17, "File already exists", fs(dest))
    tmp = _private_tmp_name(directory)
    fd = os.open(fs(tmp), os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    try:
        with os.fdopen(fd, "wb") as f:
            while True:
                size = struct.unpack(">I", _read_exact(stream, 4))[0]
                if not size:
                    break
                f.write(_read_exact(stream, size))
            try:
                os.fchown(f.fileno(), dir_st.st_uid, dir_st.st_gid)
            except OSError:
                pass
            os.fchmod(f.fileno(), 0o644)
        os.replace(fs(tmp), fs(dest))
    except BaseException:
        if os.path.lexists(fs(tmp)):
            os.unlink(fs(tmp))
        raise
    return describe(dest)

def op_ls(req):
    """Folder picker listing: accepts `~` / `~/sub`, follows symlinks for is_dir, dirs first."""
    raw = req.get("path") or "~"
    if not isinstance(raw, str):
        raise AgentError("Path must be absolute")
    if raw == "~" or raw.startswith("~/"):
        raw = os.path.expanduser("~") + raw[1:]
    path = norm(raw)
    entries = []
    with os.scandir(fs(path)) as it:
        for de in it:
            name = os.fsdecode(de.name)
            if name.startswith("."):
                continue
            full = os.path.join(path, name)
            try:
                st = os.lstat(fs(full))
            except OSError:
                continue  # vanished while listing
            is_link = stat.S_ISLNK(st.st_mode)
            is_dir = stat.S_ISDIR(st.st_mode)
            if is_link:
                try:
                    is_dir = stat.S_ISDIR(os.stat(fs(full)).st_mode)
                except OSError:
                    is_dir = False
            entries.append({
                "name": name,
                "path": full,
                "is_dir": is_dir,
                "size": st.st_size,
                "mtime": st.st_mtime,
                "mode": stat.filemode(st.st_mode),
                "is_symlink": is_link,
            })
    entries.sort(key=lambda e: (not e["is_dir"], e["name"].lower()))
    return {"path": path, "entries": entries}

PASTE_NAME_RE = re.compile(r"^paste-[0-9]{8}-[0-9]{6}-[0-9a-f]{6}\.(png|jpg|gif|webp)$")

def op_paste_image(req, stream):
    """Stores a pasted clipboard image (owner-only) under ~/<dir> and prunes stale pastes."""
    rel = req.get("dir")
    name = req.get("name")
    if (not isinstance(rel, str) or not rel or rel.startswith("/") or "\x00" in rel
            or ".." in rel.split("/")):
        raise AgentError("Invalid paste directory")
    if not isinstance(name, str) or not PASTE_NAME_RE.match(name):
        raise AgentError("Invalid file name")
    directory = os.path.join(os.path.expanduser("~"), rel)
    os.makedirs(fs(directory), 0o700, exist_ok=True)
    keep = float(req.get("keep_seconds") or 0)
    if keep > 0:
        cutoff = time.time() - keep
        with os.scandir(fs(directory)) as it:
            for de in it:
                if not os.fsdecode(de.name).startswith("paste-"):
                    continue
                try:
                    if de.is_file(follow_symlinks=False) and de.stat(follow_symlinks=False).st_mtime < cutoff:
                        os.unlink(de.path)
                except OSError:
                    pass
    dest = os.path.join(directory, name)
    fd = os.open(fs(dest), os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    try:
        with os.fdopen(fd, "wb") as f:
            os.fchmod(f.fileno(), 0o600)
            while True:
                size = struct.unpack(">I", _read_exact(stream, 4))[0]
                if not size:
                    break
                f.write(_read_exact(stream, size))
    except BaseException:
        if os.path.lexists(fs(dest)):
            os.unlink(fs(dest))
        raise
    return {"path": dest}

def op_principals(req):
    users = sorted({u.pw_name for u in pwd.getpwall()})
    groups = sorted({g.gr_name for g in grp.getgrall()})
    return {"users": users, "groups": groups, "home": os.path.expanduser("~")}

def _glob_re(pat):
    out, i = [], 0
    while i < len(pat):
        c = pat[i]
        if c == "*":
            if pat.startswith("**/", i):
                out.append("(?:.*/)?")
                i += 3
                continue
            if pat.startswith("**", i):
                out.append(".*")
                i += 2
                continue
            out.append("[^/]*")
        elif c == "?":
            out.append("[^/]")
        elif c == "[" and pat.find("]", i + 2) != -1:
            j = pat.find("]", i + 2)
            body = pat[i + 1:j]
            if body.startswith("!"):
                body = "^" + body[1:]
            out.append("[" + body.replace("\\", "\\\\") + "]")
            i = j + 1
            continue
        elif c == "\\" and i + 1 < len(pat):
            out.append(re.escape(pat[i + 1]))
            i += 2
            continue
        else:
            out.append(re.escape(c))
        i += 1
    return "".join(out)

class IgnoreRules:
    def __init__(self, patterns):
        self.rules = []
        for raw in patterns or []:
            p = str(raw).strip()
            if not p or p.startswith("#"):
                continue
            negate = p.startswith("!")
            if negate:
                p = p[1:]
            dir_only = p.endswith("/")
            p = p.rstrip("/")
            anchored = "/" in p
            p = p.lstrip("/")
            if not p:
                continue
            rx = ("^" if anchored else "^(?:.*/)?") + _glob_re(p) + "$"
            self.rules.append((re.compile(rx, re.S), negate, dir_only))

    def match(self, rel, is_dir):
        ignored = False
        for rx, negate, dir_only in self.rules:
            if (is_dir or not dir_only) and rx.match(rel):
                ignored = not negate
        return ignored

    def ignored(self, rel, is_dir):
        parts = rel.split("/")
        return any(self.match("/".join(parts[:k]), True) for k in range(1, len(parts))) or self.match(rel, is_dir)

def _sync_args(req):
    root = norm(req["root"])
    rels = []
    for raw in req["rels"]:
        rel = os.path.normpath(raw) if raw not in ("", ".") else "."
        if rel.startswith("/") or rel == ".." or rel.startswith("../") or "\x00" in rel:
            raise AgentError("Invalid relative path: " + raw)
        if rel not in rels:
            rels.append(rel)
    ignore_patterns = list(req.get("ignore") or [])
    has_gitignore = False
    gi_path = os.path.join(root, ".gitignore")
    try:
        if os.path.isfile(fs(gi_path)):
            has_gitignore = True
            if req.get("read_gitignore", True):
                with open(fs(gi_path), "r", encoding="utf-8", errors="replace") as f:
                    for line in f:
                        line = line.strip()
                        if line and not line.startswith("#"):
                            ignore_patterns.append(line)
    except OSError:
        pass
    return root, rels, IgnoreRules(ignore_patterns), has_gitignore

def _sync_entries(root, rels, rules, counts):
    for rel in rels:
        full = root if rel == "." else os.path.join(root, rel)
        st = os.lstat(fs(full))
        is_dir = stat.S_ISDIR(st.st_mode)
        if rel != ".":
            if rules.ignored(rel, is_dir):
                counts["skipped"].append(rel)
                continue
            yield rel, st
        if is_dir:
            for item in _sync_walk(root, rel, rules, counts):
                yield item

def _sync_walk(root, rel, rules, counts):
    with os.scandir(fs(root if rel == "." else os.path.join(root, rel))) as it:
        children = sorted(it, key=lambda d: d.name)
    for de in children:
        name = os.fsdecode(de.name)
        child = name if rel == "." else rel + "/" + name
        st = de.stat(follow_symlinks=False)
        is_dir = stat.S_ISDIR(st.st_mode)
        if rules.match(child, is_dir):
            counts["ignored"] += 1
            continue
        if not (is_dir or stat.S_ISREG(st.st_mode) or stat.S_ISLNK(st.st_mode)):
            counts["unsupported"] += 1
            continue
        yield child, st
        if is_dir:
            for item in _sync_walk(root, child, rules, counts):
                yield item

def _new_counts():
    return {"files": 0, "dirs": 0, "links": 0, "bytes": 0, "ignored": 0, "unsupported": 0, "skipped": [], "has_gitignore": False}

def op_scan(req):
    root, rels, rules, has_gitignore = _sync_args(req)
    counts = _new_counts()
    counts["has_gitignore"] = has_gitignore
    tar_bytes = 1024
    for rel, st in _sync_entries(root, rels, rules, counts):
        tar_bytes += 512 + (1024 if len(rel.encode("utf-8", "surrogateescape")) > 99 else 0)
        if stat.S_ISDIR(st.st_mode):
            counts["dirs"] += 1
        elif stat.S_ISLNK(st.st_mode):
            counts["links"] += 1
        else:
            counts["files"] += 1
            counts["bytes"] += st.st_size
            tar_bytes += (st.st_size + 511) // 512 * 512
    counts["tar_bytes"] = tar_bytes
    return counts

CREDIT_BYTES = 1024 * 1024

class _CreditWriter:
    """Flow control for server-to-server streaming: after every CREDIT_BYTES written, flush and
    wait for one credit byte on stdin, so the relay never buffers more than its credit window.
    EOF on stdin means the relay went away (cancelled)."""

    def __init__(self, out, credits):
        self.out = out
        self.credits = credits
        self.pending = 0

    def write(self, data):
        self.out.write(data)
        self.pending += len(data)
        while self.pending >= CREDIT_BYTES:
            self.pending -= CREDIT_BYTES
            self.out.flush()
            if not self.credits.read(1):
                raise AgentError("Transfer cancelled")
        return len(data)

    def flush(self):
        self.out.flush()

def op_pack(req, out):
    root, rels, rules, _ = _sync_args(req)
    if req.get("credit"):
        out = _CreditWriter(out, sys.stdin.buffer)
    for rel in rels:
        os.lstat(fs(root if rel == "." else os.path.join(root, rel)))
    counts = _new_counts()
    tar = tarfile.open(fileobj=out, mode="w|", format=tarfile.PAX_FORMAT, encoding="utf-8", errors="surrogateescape")
    for rel, st in _sync_entries(root, rels, rules, counts):
        full = os.path.join(root, rel)
        info = tarfile.TarInfo(rel)
        info.mode = stat.S_IMODE(st.st_mode)
        info.mtime = int(st.st_mtime)
        if stat.S_ISDIR(st.st_mode):
            info.type = tarfile.DIRTYPE
            tar.addfile(info)
        elif stat.S_ISLNK(st.st_mode):
            info.type = tarfile.SYMTYPE
            info.linkname = os.fsdecode(os.readlink(fs(full)))
            tar.addfile(info)
        else:
            with open(fs(full), "rb") as f:
                info.size = os.fstat(f.fileno()).st_size
                tar.addfile(info, f)
    tar.close()
    return None

def _within(real, real_root):
    return real == real_root or real.startswith(real_root.rstrip(b"/") + b"/")

def _lchown(path, uid, gid):
    try:
        os.lchown(fs(path), uid, gid)
    except OSError:
        pass

def _mkdir_like_parent(path, mode, parent_st):
    os.mkdir(fs(path), 0o700)
    dfd = os.open(fs(path), os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        try:
            os.fchown(dfd, parent_st.st_uid, parent_st.st_gid)
        except OSError:
            pass
        os.fchmod(dfd, mode)
    finally:
        os.close(dfd)

def _ensure_dirs(root, rel, real_root, leaf_mode=None):
    cur = root
    parts = [p for p in rel.split("/") if p] if rel else []
    for n, part in enumerate(parts):
        parent_st = os.stat(fs(cur))
        cur = os.path.join(cur, part)
        try:
            st = os.lstat(fs(cur))
        except FileNotFoundError:
            mode = leaf_mode if n == len(parts) - 1 and leaf_mode is not None else 0o755
            _mkdir_like_parent(cur, (mode & 0o777) | (parent_st.st_mode & stat.S_ISGID), parent_st)
            continue
        if stat.S_ISLNK(st.st_mode):
            real = os.path.realpath(fs(cur))
            if not _within(real, real_root) or not os.path.isdir(real):
                raise AgentError("Refusing to write through symlink leaving the destination: " + cur)
        elif not stat.S_ISDIR(st.st_mode):
            raise AgentError("Exists on the destination and is not a directory: " + cur)
    return cur

def _make_root(root):
    missing, cur = [], root
    while not os.path.lexists(fs(cur)):
        missing.append(cur)
        cur = os.path.dirname(cur)
    if not os.path.isdir(fs(cur)):
        raise AgentError("Exists on the destination and is not a directory: " + cur)
    for path in reversed(missing):
        parent_st = os.stat(fs(os.path.dirname(path)))
        _mkdir_like_parent(path, 0o755, parent_st)

def _safe_member_name(name):
    rel = name.rstrip("/")
    if not rel or rel.startswith("/") or "\x00" in rel:
        raise AgentError("Unsafe path in archive: " + name)
    parts = rel.split("/")
    if any(p in ("", ".", "..") for p in parts):
        raise AgentError("Unsafe path in archive: " + name)
    return rel

def _existing(path):
    try:
        st = os.lstat(fs(path))
    except FileNotFoundError:
        return None
    if stat.S_ISDIR(st.st_mode):
        raise AgentError("Is a directory on the destination: " + path)
    return st

def op_unpack(req, stream):
    root = norm(req["root"])
    if root in ("/", "/bin", "/boot", "/dev", "/etc", "/lib", "/lib64", "/proc", "/sys", "/usr", "/run", "/sbin"):
        raise AgentError("Refusing to unpack into system directory: " + root)
    _make_root(root)
    real_root = os.path.realpath(fs(root))
    counts = {"files": 0, "dirs": 0, "links": 0, "bytes": 0, "unsupported": 0}
    tar = tarfile.open(fileobj=stream, mode="r|", encoding="utf-8", errors="surrogateescape")
    for m in tar:
        rel = _safe_member_name(m.name)
        if m.isdir():
            _ensure_dirs(root, rel, real_root, leaf_mode=m.mode)
            counts["dirs"] += 1
            continue
        if not (m.isfile() or m.issym()):
            counts["unsupported"] += 1
            continue
        parent = _ensure_dirs(root, os.path.dirname(rel), real_root)
        dest = os.path.join(parent, os.path.basename(rel))
        old = _existing(dest)
        parent_st = os.stat(fs(parent))
        tmp = _private_tmp_name(parent)
        try:
            if m.issym():
                os.symlink(fs(m.linkname), fs(tmp))
                _lchown(tmp, parent_st.st_uid, parent_st.st_gid)
                counts["links"] += 1
            else:
                owner = old if old is not None and stat.S_ISREG(old.st_mode) else parent_st
                fd = os.open(fs(tmp), os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
                with os.fdopen(fd, "wb") as f:
                    shutil.copyfileobj(tar.extractfile(m), f, 1024 * 1024)
                    try:
                        os.fchown(f.fileno(), owner.st_uid, owner.st_gid)
                    except OSError:
                        pass
                    os.fchmod(f.fileno(), m.mode & 0o777)
                    f.flush()
                    os.utime(f.fileno(), (m.mtime, m.mtime))
                counts["files"] += 1
                counts["bytes"] += m.size
            os.replace(fs(tmp), fs(dest))
        except BaseException:
            if os.path.lexists(fs(tmp)):
                os.unlink(fs(tmp))
            raise
    return counts

OPS = {
    "list": op_list, "stat": op_stat, "read": op_read, "write": op_write, "mkdir": op_mkdir,
    "symlink": op_symlink, "rename": op_rename, "paste": op_paste, "delete": op_delete,
    "chmod": op_chmod, "principals": op_principals, "scan": op_scan, "ls": op_ls,
}

def main():
    req = {}
    try:
        req = json.loads(sys.stdin.buffer.readline())
        op = req.get("op")
        if op == "pack":
            op_pack(req, sys.stdout.buffer)
            sys.stdout.buffer.flush()
            return
        if op == "unpack":
            result = {"ok": True, "result": op_unpack(req, sys.stdin.buffer)}
        elif op == "upload":
            result = {"ok": True, "result": op_upload(req, sys.stdin.buffer)}
        elif op == "paste_image":
            result = {"ok": True, "result": op_paste_image(req, sys.stdin.buffer)}
        else:
            handler = OPS.get(op)
            if not handler:
                raise AgentError("Unknown operation")
            result = {"ok": True, "result": handler(req)}
    except AgentError as e:
        result = {"ok": False, "error": str(e)}
    except FileNotFoundError as e:
        result = {"ok": False, "error": "No such file or directory: %s" % os.fsdecode(e.filename or b"")}
    except PermissionError as e:
        result = {"ok": False, "error": "Permission denied: %s" % os.fsdecode(e.filename or b"")}
    except FileExistsError as e:
        result = {"ok": False, "error": "Already exists: %s" % os.fsdecode(e.filename or b""), "code": "exists"}
    except (OSError, KeyError, ValueError, tarfile.TarError) as e:
        result = {"ok": False, "error": str(e)}
    if req.get("op") == "pack":
        sys.stderr.write(json.dumps(result))
        sys.exit(1)
    sys.stdout.write(json.dumps(result))

if __name__ == "__main__":
    main()
