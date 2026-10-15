<script>
  import PreviewTable from "./lib/PreviewTable.svelte";
  import RulesPanel from "./lib/RulesPanel.svelte";
  import Toolbar from "./lib/Toolbar.svelte";
  import StatusBar from "./lib/StatusBar.svelte";
  import UndoHistory from "./lib/UndoHistory.svelte";
  import Updater from "./lib/Updater.svelte";
  import { requestPreview } from "./lib/previewBridge.js";
  import { filterItems } from "./lib/filters.js";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { save, ask, message } from "@tauri-apps/plugin-dialog";
  import { onMount } from "svelte";
  import { t, ui } from "./i18n.svelte.js";

  // Svelte 5: this component uses runes ($derived below), so every reactive
  // top-level variable MUST be $state — plain `let` silently stops updating
  // the DOM (preview table showed nothing while the backend had results).
  let items = $state([]);
  let summary = $state({ total: 0, ready: 0, unchanged: 0, warnings: 0, errors: 0 });
  let rules = $state([]);
  let applyTo = $state("full");
  let conflictStrategy = $state("block");
  let scanPath = $state("");
  let recursive = $state(true);
  let filterGlobs = $state("");
  let minSizeMb = $state(null);
  let maxSizeMb = $state(null);
  let modifiedAfter = $state("");
  let modifiedBefore = $state("");
  let scanning = $state(false);
  let renaming = $state(false);
  let progressText = $state("");
  let progress = $state({ done: 0, total: 0 });
  let search = $state("");
  let statusFilter = $state("all");
  let exporting = $state(false);
  // Non-blocking notice banner (replaces silent console-only failures and the
  // blocking alert() popups for scan/preview results)
  let notice = $state({ kind: "", text: "" }); // kind: ok | err
  let noticeTimer = null;
  const selectedCount = $derived(items.filter((i) => i.selected).length);
  const allSelected = $derived(items.length > 0 && items.every((i) => i.selected));
  let undoBatches = $state([]);
  let previewTimer = null;
  let unlistenProgress = null;

  onMount(() => {
    document.documentElement.dataset.theme = ui.theme;
    schedulePreview();
    // Real-time progress from the Rust executor
    listen("rename-progress", (e) => {
      progress = { done: e.payload.done, total: e.payload.total };
    }).then((un) => (unlistenProgress = un));
    invoke("list_undo_history").then((b) => (undoBatches = b)).catch(() => {});
    // Drag & drop folders from Explorer -> append to the current working set
    let unlistenDrag = null;
    getCurrentWebview()
      .onDragDropEvent(async (event) => {
        if (event.payload.type === "drop" && event.payload.paths?.length) {
          const paths = event.payload.paths.join(";");
          scanPath = scanPath ? scanPath + ";" + paths : paths;
          await doScan(true, paths);
        }
      })
      .then((un) => (unlistenDrag = un));
    return () => {
      unlistenProgress?.();
      unlistenDrag?.();
    };
  });

  // Non-blocking notice, auto-dismisses after a few seconds
  function showNotice(kind, text, ms = 8000) {
    clearTimeout(noticeTimer);
    notice = { kind, text };
    if (ms > 0) noticeTimer = setTimeout(() => (notice = { kind: "", text: "" }), ms);
  }

  function closeNotice() {
    clearTimeout(noticeTimer);
    notice = { kind: "", text: "" };
  }

  // Updater callback bridge (watchdog banners live in the same notice strip)
  function onUpdaterNotice(kind, text, ms) {
    showNotice(kind, text, ms);
  }

  // append=true keeps existing entries; pathsOverride scans only those paths
  // (used by "+ add folders" and drag&drop so re-sending the whole field
  // cannot duplicate entries already in the set).
  export async function doScan(append = false, pathsOverride = null) {
    scanning = true;
    progressText = t("scanning");
    try {
      const mb = (v) => (v == null || v === "" || isNaN(v) ? null : Math.round(Number(v) * 1024 * 1024));
      const dateMs = (d) => (d ? new Date(d + "T00:00:00").getTime() : null);
      const opts = {
        path: pathsOverride ?? scanPath,
        recursive,
        maxDepth: 0,
        scope: "both",
        includeGlobs: filterGlobs
          .split(",")
          .map((s) => s.trim())
          .filter(Boolean),
        excludeGlobs: [],
        includeRegex: null,
        minSize: mb(minSizeMb),
        maxSize: mb(maxSizeMb),
        modifiedAfterMs: dateMs(modifiedAfter),
        modifiedBeforeMs: modifiedBefore ? new Date(modifiedBefore + "T23:59:59").getTime() : null,
        includeHidden: false,
        skipSymlinks: true,
      };
      await invoke("scan_paths", { opts, append });
      await refreshPreview();
      if (summary.total > 0) showNotice("ok", t("scanDone").replace("{n}", summary.total));
    } catch (e) {
      console.error(e);
      showNotice("err", t("scanFailed").replace("{e}", String(e)), 0);
    } finally {
      scanning = false;
      progressText = "";
    }
  }

  export async function refreshPreview() {
    try {
      const res = await requestPreview(invoke, { rules, conflictStrategy, applyTo });
      items = res.items;
      summary = { total: res.total, ready: res.ready, unchanged: res.unchanged, warnings: res.warnings, errors: res.errors };
    } catch (e) {
      console.error(e);
      showNotice("err", t("previewFailed").replace("{e}", String(e)), 0);
    }
  }

  function schedulePreview() {
    // Debounce: rules changes trigger preview refresh after 200ms
    clearTimeout(previewTimer);
    previewTimer = setTimeout(refreshPreview, 200);
  }

  // Export old,new name pairs for the WHOLE working set (for Import List)
  export async function doExportMapping() {
    if (items.length === 0) return;
    const path = await save({
      title: t("mappingHint"),
      filters: [{ name: "CSV", extensions: ["csv"] }],
      defaultPath: "renamer-mapping.csv",
    });
    if (!path) return;
    exporting = true;
    try {
      const n = await invoke("export_mapping_csv", { items, path });
      showNotice("ok", t("csvSaved").replace("{n}", n));
    } catch (e) {
      showNotice("err", t("errPrefix").replace("{e}", String(e)), 0);
    } finally {
      exporting = false;
    }
  }

  export function onRulesChanged(newRules) {
    rules = newRules;
    schedulePreview();
  }

  export async function doRename() {
    const n = summary.ready + summary.warnings;
    if (n === 0) return;
    if (summary.errors > 0 && conflictStrategy === "block") {
      showNotice("err", `มี ${summary.errors} รายการผิดพลาด ต้องแก้ก่อนเปลี่ยนชื่อ`, 0);
      return;
    }
    const ok = await ask(`เปลี่ยนชื่อ ${n} รายการ?\n(ก่อนรันจะมีการตรวจสอบซ้ำอีกครั้งบนดิสก์)`, { title: t("rename") });
    if (!ok) return;
    renaming = true;
    progressText = t("renaming");
    progress = { done: 0, total: summary.ready + summary.warnings };
    try {
      await invoke("update_rules", { rules });
      await invoke("set_conflict_strategy", { strategy: conflictStrategy });
      await invoke("set_apply_to", { applyTo });
      const report = await invoke("apply_renames");
      undoBatches = await invoke("list_undo_history");
      showNotice("ok", `สำเร็จ ${report.renamed} รายการ, ล้มเหลว ${report.failed}`);
      await refreshPreview();
    } catch (e) {
      showNotice("err", t("errPrefix").replace("{e}", String(e)), 0);
    } finally {
      renaming = false;
      progressText = "";
      progress = { done: 0, total: 0 };
    }
  }

  export async function doUndo() {
    try {
      const msg = await invoke("undo_last");
      showNotice("ok", msg);
      await refreshPreview();
    } catch (e) {
      showNotice("err", t("errPrefix").replace("{e}", String(e)), 0);
    }
  }

  function onUndoDone() {
    return refreshPreview();
  }

  // Clear the whole working set (rules/presets untouched)
  export async function doClearEntries() {
    if (items.length === 0) return;
    if (!await ask(t("confirmClear"), { title: t("clearList") })) return;
    try {
      await invoke("clear_entries");
      await refreshPreview();
    } catch (e) {
      showNotice("err", t("errPrefix").replace("{e}", String(e)), 0);
    }
  }

  function onToggleSelect(id, checked) {
    invoke("set_selection", { ids: [id], selected: checked })
      .catch(console.error);
    // Optimistic local update; full refresh happens on the next preview round
    const it = items.find((i) => i.id === id);
    if (it) it.selected = checked;
  }

  function onSelectAll(checked) {
    invoke("set_selection", { ids: [], selected: checked })
      .then(() => refreshPreview())
      .catch(console.error);
  }

  async function doRemoveSelected() {
    const ids = items.filter((i) => i.selected).map((i) => i.id);
    if (ids.length === 0) return;
    if (!await ask(t("confirmRemoveSelected").replace("{n}", ids.length), { title: t("removeSelected") })) return;
    try {
      await invoke("remove_entries", { ids });
      showNotice("ok", t("removed").replace("{n}", ids.length));
      await refreshPreview();
    } catch (e) {
      showNotice("err", t("errPrefix").replace("{e}", String(e)), 0);
    }
  }

  // Export the CURRENT VIEW (same filters as the table) to a CSV file
  export async function doExportCsv() {
    const rows = filterItems(items, search, statusFilter);
    if (rows.length === 0) {
      await message(t("csvEmpty"), { title: t("exportCsv") });
      return;
    }
    const path = await save({
      title: t("exportCsv"),
      filters: [{ name: "CSV", extensions: ["csv"] }],
      defaultPath: "renamer-preview.csv",
    });
    if (!path) return;
    exporting = true;
    try {
      const n = await invoke("export_preview_csv", { items: rows, path });
      showNotice("ok", t("csvSaved").replace("{n}", n));
    } catch (e) {
      showNotice("err", t("errPrefix").replace("{e}", String(e)), 0);
    } finally {
      exporting = false;
    }
  }

  async function onManualEdit(id, newName) {
    await invoke("set_manual_name", { id, name: newName });
    await refreshPreview();
  }

  function handleKeydown(e) {
    if (e.ctrlKey && e.key === "Enter") doRename();
    else if (e.ctrlKey && e.key.toLowerCase() === "z") doUndo();
  }
</script>

<svelte:window onkeydown={handleKeydown} />  <div class="app" data-theme={ui.theme}>
  {#if notice.text}
    <div class="notice notice-{notice.kind}" role="status" aria-live="polite">
      <span class="notice-text">{notice.text}</span>
      <button class="notice-close" aria-label="close" onclick={closeNotice}>✕</button>
    </div>
  {/if}
  <Toolbar
    bind:scanPath
    bind:recursive
    bind:filterGlobs
    bind:applyTo
    bind:conflictStrategy
    bind:minSizeMb
    bind:maxSizeMb
    bind:modifiedAfter
    bind:modifiedBefore
    onSettingsChanged={schedulePreview}
    onScan={doScan}
    onClear={doClearEntries}
    scanning={scanning}
  />
  <div class="main">
    <RulesPanel
      bind:rules
      onRulesChanged={onRulesChanged}
    />
    <div class="table-pane">
      <div class="pane-bar">
        <input class="search" type="search" placeholder={t("search")} bind:value={search} />
        <select bind:value={statusFilter} title={t("statusFilter")}>
          <option value="all">{t("statusAll")}</option>
          <option value="ready">{t("ready")}</option>
          <option value="unchanged">{t("unchanged")}</option>
          <option value="warning">{t("warnings")}</option>
          <option value="error">{t("errors")}</option>
          <option value="manual">{t("manual")}</option>
        </select>
        <input
          id="sel-all"
          type="checkbox"
          checked={allSelected}
          oninput={(e) => onSelectAll(e.target.checked)}
          title={t("selectAllHint")}
        />
        <label class="sel-count" for="sel-all">{selectedCount} {t("selected")}</label>
        <button class="ghost" onclick={doRemoveSelected} disabled={exporting || selectedCount === 0} title={t("removeSelectedHint")}>🗑 {t("removeSelected")}</button>
        <button class="ghost" onclick={doExportCsv} disabled={exporting} title={t("exportHint")}>⤓ {t("exportCsv")}</button>
        <button class="ghost" onclick={doExportMapping} disabled={exporting || items.length === 0} title={t("mappingHint")}>⤓ {t("exportMapping")}</button>
      </div>
      <PreviewTable {items} {search} {statusFilter} onManualEdit={onManualEdit} onToggleSelect={onToggleSelect} />
      <UndoHistory bind:batches={undoBatches} on:undone={onUndoDone} />
    </div>
  </div>
  <StatusBar {summary} {progressText} onRename={doRename} onUndo={doUndo} {renaming} {progress} />
  <Updater notify={onUpdaterNotice} />
</div>

<style>
  :global(html) {
    height: 100%;
  }
  :global(body) {
    margin: 0;
    height: 100%;
    font-family: "Segoe UI", system-ui, sans-serif;
  }
  :global([data-theme="dark"]) {
    --bg: #0f172a;
    --panel: #1e293b;
    --text: #e2e8f0;
    --muted: #94a3b8;
    --accent: #60a5fa;
    --ok: #4ade80;
    --warn: #facc15;
    --err: #f87171;
    --border: #334155;
  }
  :global([data-theme="light"]) {
    --bg: #f8fafc;
    --panel: #ffffff;
    --text: #0f172a;
    --muted: #64748b;
    --accent: #2563eb;
    --ok: #16a34a;
    --warn: #ca8a04;
    --err: #dc2626;
    --border: #e2e8f0;
  }
  .app {
    display: flex;
    flex-direction: column;
    height: 100vh;
    background: var(--bg);
    color: var(--text);
  }
  .main {
    display: flex;
    flex: 1;
    min-height: 0;
  }
  .notice {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 12px;
    font-size: 13px;
    border-bottom: 1px solid var(--border);
  }
  .notice-ok {
    background: var(--panel);
    background: color-mix(in srgb, var(--ok) 12%, var(--panel));
    color: var(--text);
  }
  .notice-err {
    background: var(--panel);
    background: color-mix(in srgb, var(--err) 12%, var(--panel));
    color: var(--err);
  }
  .notice-text { flex: 1; min-width: 0; overflow-wrap: anywhere; }
  .notice-close {
    border: none;
    background: transparent;
    color: inherit;
    cursor: pointer;
    padding: 2px 6px;
  }
  .table-pane {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
    padding: 8px;
  }
  .pane-bar {
    display: flex;
    gap: 8px;
    margin-bottom: 6px;
    align-items: center;
  }
  .search {
    flex: 1;
    padding: 6px 10px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--panel);
    color: var(--text);
  }
  .sel-count {
    color: var(--muted);
    font-size: 12px;
    white-space: nowrap;
  }
  .pane-bar select,
  .pane-bar button {
    padding: 6px 10px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--panel);
    color: var(--text);
    cursor: pointer;
    white-space: nowrap;
  }
  .pane-bar button:disabled { opacity: 0.5; cursor: default; }
</style>
