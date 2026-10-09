<script>
  // Virtualized table: renders only the visible slice of rows so 100k rows
  // scroll smoothly.
  import { t } from "../i18n.svelte.js";

  let { items = [], search = "", onManualEdit } = $props();

  const ROW_H = 28;
  let viewport;
  let scrollTop = $state(0);
  let viewportH = $state(600);
  let editingId = $state(null);
  let editValue = $state("");

  const filtered = $derived(
    search
      ? items.filter((i) =>
          (i.oldName + " " + i.newName + " " + i.parent)
            .toLowerCase()
            .includes(search.toLowerCase())
        )
      : items
  );

  const start = $derived(Math.max(0, Math.floor(scrollTop / ROW_H) - 5));
  const end = $derived(
    Math.min(filtered.length, Math.ceil((scrollTop + viewportH) / ROW_H) + 5)
  );
  const visible = $derived(filtered.slice(start, end));

  function onScroll(e) {
    scrollTop = e.target.scrollTop;
  }

  function statusClass(s) {
    return { ready: "st-ready", unchanged: "st-unchanged", warning: "st-warn", error: "st-err", manual: "st-manual" }[s] ?? "";
  }

  function fmtSize(n) {
    if (n < 1024) return n + " B";
    if (n < 1048576) return (n / 1024).toFixed(1) + " KB";
    if (n < 1073741824) return (n / 1048576).toFixed(1) + " MB";
    return (n / 1073741824).toFixed(2) + " GB";
  }

  function fmtDate(ms) {
    if (!ms) return "-";
    return new Date(ms).toLocaleString();
  }

  function highlightDiff(oldName, newName) {
    if (oldName === newName) return escapeHtml(newName);
    // Simple char-level prefix/suffix diff highlight
    let i = 0;
    const min = Math.min(oldName.length, newName.length);
    while (i < min && oldName[i] === newName[i]) i++;
    let j = 0;
    while (
      j < min - i &&
      oldName[oldName.length - 1 - j] === newName[newName.length - 1 - j]
    )
      j++;
    const pre = newName.slice(0, i);
    const mid = newName.slice(i, newName.length - j);
    const post = newName.slice(newName.length - j);
    return `${escapeHtml(pre)}<mark>${escapeHtml(mid)}</mark>${escapeHtml(post)}`;
  }

  function escapeHtml(value) {
    return value.replace(/[&<>"']/g, (char) => ({
      "&": "&amp;",
      "<": "&lt;",
      ">": "&gt;",
      '"': "&quot;",
      "'": "&#39;",
    })[char]);
  }

  function startEdit(item) {
    editingId = item.id;
    editValue = item.newName;
  }

  function commitEdit(item) {
    editingId = null;
    if (editValue && editValue !== item.newName) {
      onManualEdit?.(item.id, editValue);
    }
  }

  function focusInput(node) {
    node.focus();
  }

  function handleEditKeydown(event, item) {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      startEdit(item);
    }
  }

</script>

<div class="vp" bind:this={viewport} onscroll={onScroll}>
  <div class="spacer" style="height: {filtered.length * ROW_H}px">
    <table>
      <thead>
        <tr>
          <th style="width:90px">{t("status")}</th>
          <th>{t("oldName")}</th>
          <th>{t("newName")}</th>
          <th style="width:220px">{t("folder")}</th>
          <th style="width:90px">{t("size")}</th>
          <th style="width:150px">{t("modified")}</th>
        </tr>
      </thead>
      <tbody style="transform: translateY({start * ROW_H}px)">
        {#each visible as item (item.id)}
          <tr class="row {statusClass(item.status)}" style="height:{ROW_H}px">
            <td class="status-cell">
              {item.status === "ready" ? "✓" : item.status === "unchanged" ? "·" : item.status === "warning" ? "⚠" : item.status === "error" ? "✗" : "✎"}
              {#if item.message}<span class="msg" title={item.message}>ⓘ</span>{/if}
            </td>
            <td class="mono">{item.oldName}</td>
            <td class="mono new-name">
              {#if editingId === item.id}
                <input
                  value={editValue}
                  oninput={(e) => (editValue = e.target.value)}
                  onkeydown={(e) => e.key === "Enter" && commitEdit(item)}
                  onblur={() => commitEdit(item)}
                  use:focusInput
                />
              {:else}
                <span
                  role="button"
                  tabindex="0"
                  title={item.message || ""}
                  ondblclick={() => startEdit(item)}
                  onkeydown={(e) => handleEditKeydown(e, item)}
                  >{@html highlightDiff(item.oldName, item.newName)}</span
                >
              {/if}
            </td>
            <td class="mono dim" title={item.parent}>{item.parent}</td>
            <td>{item.isDir ? "—" : fmtSize(item.size)}</td>
            <td>{fmtDate(item.modifiedMs)}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
</div>

<style>
  .vp {
    flex: 1;
    overflow: auto;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--panel);
    position: relative;
  }
  .spacer {
    position: relative;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    table-layout: fixed;
    font-size: 13px;
  }
  thead th {
    position: sticky;
    top: 0;
    background: var(--panel);
    text-align: left;
    padding: 6px 8px;
    border-bottom: 2px solid var(--border);
    z-index: 1;
  }
  td {
    padding: 2px 8px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    border-bottom: 1px solid var(--border);
  }
  .mono {
    font-family: Consolas, monospace;
  }
  .dim {
    color: var(--muted);
    font-size: 12px;
  }
  :global(.st-ready .new-name mark) {
    background: color-mix(in srgb, var(--ok) 30%, transparent);
    color: inherit;
  }
  .st-warn {
    background: color-mix(in srgb, var(--warn) 12%, transparent);
  }
  .st-err {
    background: color-mix(in srgb, var(--err) 15%, transparent);
  }
  .st-manual {
    background: color-mix(in srgb, var(--accent) 12%, transparent);
  }
  .status-cell {
    color: var(--muted);
  }
  .st-ready .status-cell { color: var(--ok); }
  .st-warn .status-cell { color: var(--warn); }
  .st-err .status-cell { color: var(--err); }
  .msg {
    margin-left: 4px;
    cursor: help;
  }
  input {
    width: 95%;
    background: var(--bg);
    color: var(--text);
    border: 1px solid var(--accent);
    border-radius: 4px;
    padding: 1px 4px;
    font-family: Consolas, monospace;
  }
</style>
