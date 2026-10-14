// Shared filtering for preview items (display + CSV export use the same logic)
export const STATUS_VALUES = ["ready", "unchanged", "warning", "error", "manual"];

export function filterItems(items, query = "", status = "all") {
  const q = (query || "").trim().toLowerCase();
  return items.filter((i) => {
    if (q && !(
      (i.oldName || "").toLowerCase().includes(q) ||
      (i.newName || "").toLowerCase().includes(q) ||
      (i.parent || "").toLowerCase().includes(q)
    )) {
      return false;
    }
    if (status && status !== "all" && i.status !== status) return false;
    return true;
  });
}
