// Reordering servers in the sidebar. The sidebar shows servers per group, so a
// move only changes positions among servers of the same group. Both functions
// return the new full id order for the backend, or null when nothing changes.

interface Orderable {
  id: number;
  group_name: string;
}

/** Swap a server with its previous (-1) or next (+1) neighbour in the same group. */
export function moveWithinGroup(servers: Orderable[], id: number, step: -1 | 1): number[] | null {
  const index = servers.findIndex((s) => s.id === id);
  if (index === -1) return null;
  const group = servers[index].group_name;
  const members = servers.flatMap((s, i) => (s.group_name === group ? [i] : []));
  const target = members[members.indexOf(index) + step];
  if (target === undefined) return null;
  const ids = servers.map((s) => s.id);
  [ids[index], ids[target]] = [ids[target], ids[index]];
  return ids;
}

/** Drop a dragged server just before or after another server of the same group. */
export function moveBeside(servers: Orderable[], id: number, targetId: number, after: boolean): number[] | null {
  const moving = servers.find((s) => s.id === id);
  const target = servers.find((s) => s.id === targetId);
  if (!moving || !target || id === targetId || moving.group_name !== target.group_name) return null;
  const ids = servers.map((s) => s.id).filter((sid) => sid !== id);
  ids.splice(ids.indexOf(targetId) + (after ? 1 : 0), 0, id);
  // Only the order inside the group is visible; ignore drops that keep it as is.
  const inGroup = new Set(servers.filter((s) => s.group_name === moving.group_name).map((s) => s.id));
  const before = servers.map((s) => s.id).filter((sid) => inGroup.has(sid));
  const afterMove = ids.filter((sid) => inGroup.has(sid));
  return afterMove.every((sid, i) => sid === before[i]) ? null : ids;
}
