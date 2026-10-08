<script>
  import PreviewTable from "./lib/PreviewTable.svelte";
  import RulesPanel from "./lib/RulesPanel.svelte";
  import Toolbar from "./lib/Toolbar.svelte";
  import StatusBar from "./lib/StatusBar.svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import { t, setLang, setTheme, ui } from "./i18n.svelte.js";

  let items = [];
  let summary = { total: 0, ready: 0, unchanged: 0, warnings: 0, errors: 0 };
  let rules = [];
  let applyTo = "full";
  let conflictStrategy = "block";
  let scanPath = "";
  let recursive = true;
  let filterGlobs = "";
  let scanning = false;
  let renaming = false;
  let progressText = "";
  let progress = { done: 0, total: 0 };
  let search = "";
  let previewTimer = null;
  let unlistenProgress = null;

  onMount(() => {
    document.documentElement.dataset.theme = ui.theme;
    schedulePreview();
    // Real-time progress from the Rust executor
    listen("rename-progress", (e) => {
      progress = { done: e.payload.done, total: e.payload.total };
    }).then((un) => (unlistenProgress = un));
    return () => unlistenProgress?.();
  });

  export async function doScan() {
    scanning = true;
    progressText = t("scanning");
    try {
      const opts = {
        path: scanPath,
        recursive,
        maxDepth: 0,
        scope: "both",
        includeGlobs: filterGlobs
          .split(",")
          .map((s) => s.trim())
          .filter(Boolean),
        excludeGlobs: [],
        includeRegex: null,
        minSize: null,
        maxSize: null,
        modifiedAfterMs: null,
        modifiedBeforeMs: null,
        includeHidden: false,
        skipSymlinks: true,
      };
      await invoke("scan_paths", { opts });
      await refreshPreview();
    } catch (e) {
      console.error(e);
    } finally {
      scanning = false;
      progressText = "";
    }
  }

  export async function refreshPreview() {
    try {
      const res = await invoke("preview");
      items = res.items;
      summary = { total: res.total, ready: res.ready, unchanged: res.unchanged, warnings: res.warnings, errors: res.errors };
    } catch (e) {
      console.error(e);
    }
  }

  function schedulePreview() {
    // Debounce: rules changes trigger preview refresh after 200ms
    clearTimeout(previewTimer);
    previewTimer = setTimeout(refreshPreview, 200);
  }

  export function onRulesChanged(newRules) {
    rules = newRules;
    schedulePreview();
  }

  export async function doRename() {
    const n = summary.ready + summary.warnings;
    if (n === 0) return;
    if (summary.errors > 0 && conflictStrategy === "block") {
      alert(`มี ${summary.errors} รายการผิดพลาด ต้องแก้ก่อนเปลี่ยนชื่อ`);
      return;
    }
    const ok = confirm(`เปลี่ยนชื่อ ${n} รายการ?\n(ก่อนรันจะมีการตรวจสอบซ้ำอีกครั้งบนดิสก์)`);
    if (!ok) return;
    renaming = true;
    progressText = t("renaming");
    progress = { done: 0, total: summary.ready + summary.warnings };
    try {
      await invoke("update_rules", { rules });
      await invoke("set_conflict_strategy", { strategy: conflictStrategy });
      await invoke("set_apply_to", { applyTo });
      const report = await invoke("apply_renames");
      alert(`สำเร็จ ${report.renamed} รายการ, ล้มเหลว ${report.failed}`);
      await refreshPreview();
    } catch (e) {
      alert("ผิดพลาด: " + e);
    } finally {
      renaming = false;
      progressText = "";
      progress = { done: 0, total: 0 };
    }
  }

  export async function doUndo() {
    try {
      const msg = await invoke("undo_last");
      alert(msg);
      await refreshPreview();
    } catch (e) {
      alert("ย้อนกลับไม่ได้: " + e);
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

<svelte:window on:keydown={handleKeydown} />

<div class="app" data-theme={ui.theme}>
  <Toolbar
    bind:scanPath
    bind:recursive
    bind:filterGlobs
    bind:applyTo
    bind:conflictStrategy
    onScan={doScan}
    scanning={scanning}
  />
  <div class="main">
    <RulesPanel
      bind:rules
      onRulesChanged={onRulesChanged}
    />
    <div class="table-pane">
      <input class="search" type="search" placeholder={t("search")} bind:value={search} />
      <PreviewTable {items} {search} onManualEdit={onManualEdit} />
    </div>
  </div>
  <StatusBar {summary} {progressText} onRename={doRename} onUndo={doUndo} {renaming} {progress} />
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
  .table-pane {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
    padding: 8px;
  }
  .search {
    padding: 6px 10px;
    margin-bottom: 6px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--panel);
    color: var(--text);
  }
</style>
