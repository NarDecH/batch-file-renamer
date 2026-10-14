<script>
  import { open } from "@tauri-apps/plugin-dialog";
  import { t, setLang, setTheme, ui } from "../i18n.svelte.js";

  let { scanPath = $bindable(""), recursive = $bindable(true), filterGlobs = $bindable(""), applyTo = $bindable("full"), conflictStrategy = $bindable("block"), minSizeMb = $bindable(null), maxSizeMb = $bindable(null), modifiedAfter = $bindable(""), modifiedBefore = $bindable(""), onSettingsChanged, onScan, onClear, scanning } = $props();

  // Picking folders with the native dialog now scans immediately:
  // - browse(false): picked folders replace the working set
  // - browse(true):  picked folders are appended to the current set
  async function browse(append = false) {
    const picked = await open({ directory: true, multiple: true, title: t("scanPath") });
    if (!picked) return;
    const paths = Array.isArray(picked) ? picked.join(";") : picked;
    if (append) {
      // Only scan the NEW paths (backend append would duplicate if we re-scanned
      // the whole field), but keep the field showing the full set.
      scanPath = scanPath ? scanPath + ";" + paths : paths;
      await onScan?.(true, paths);
    } else {
      scanPath = paths;
      await onScan?.(false);
    }
  }
</script>

<div class="bar">
  <label for="scan-path">{t("scanPath")}</label>
  <input id="scan-path" class="path" placeholder="C:\Users\..." bind:value={scanPath} />
  <button onclick={browse}>…</button>
  <label class="chk"><input type="checkbox" bind:checked={recursive} /> {t("recursive")}</label>
  <input class="filter" placeholder={t("filter")} bind:value={filterGlobs} />
  <input class="num" type="number" min="0" step="0.1" placeholder="min MB" title="minSize" bind:value={minSizeMb} />
  <input class="num" type="number" min="0" step="0.1" placeholder="max MB" title="maxSize" bind:value={maxSizeMb} />
  <input class="date" type="date" title="modified from" bind:value={modifiedAfter} />
  <input class="date" type="date" title="modified to" bind:value={modifiedBefore} />
  <button class="primary" onclick={() => browse(false)} disabled={scanning} title={t("openHint")}>
    📂 {t("openScan")}
  </button>
  <button onclick={() => browse(true)} disabled={scanning} title={t("addHint")}>✚ {t("addFolders")}</button>
  <button onclick={() => onScan(false)} disabled={scanning || !scanPath} title={t("rescanHint")}>{t("scan")}</button>
  <button class="danger" onclick={() => onClear?.()} disabled={scanning} title={t("clearHint")}>🗑 {t("clearList")}</button>

  <select bind:value={applyTo} onchange={() => onSettingsChanged?.()} title={t("applyTo")}>
    <option value="name">name</option>
    <option value="extension">extension</option>
    <option value="full">name + extension</option>
  </select>

  <select bind:value={conflictStrategy} onchange={() => onSettingsChanged?.()} title={t("conflict")}>
    <option value="block">{t("strategyBlock")}</option>
    <option value="auto_suffix">{t("strategyAuto")}</option>
    <option value="skip">{t("strategySkip")}</option>
  </select>

  <span class="spacer"></span>
  <select onchange={(e) => setLang(e.target.value)} value={ui.lang} title={t("language")}>
    <option value="th">ไทย</option>
    <option value="en">English</option>
  </select>
  <select onchange={(e) => setTheme(e.target.value)} value={ui.theme} title={t("theme")}>
    <option value="dark">🌙 dark</option>
    <option value="light">☀️ light</option>
  </select>
</div>

<style>
  .bar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px;
    background: var(--panel);
    border-bottom: 1px solid var(--border);
    flex-wrap: wrap;
  }
  label { color: var(--muted); font-size: 13px; }
  .chk { display: flex; align-items: center; gap: 4px; }
  input, select {
    background: var(--bg);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 5px 8px;
  }
  .path { flex: 1; min-width: 200px; font-family: Consolas, monospace; }
  .filter { width: 140px; }
  .num { width: 80px; }
  .date { width: 135px; }
  button {
    background: var(--bg);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 5px 12px;
    cursor: pointer;
  }
  button.primary {
    background: var(--accent);
    border-color: var(--accent);
    color: #fff;
    font-weight: 600;
  }
  button.danger {
    color: var(--err);
  }
  button:disabled { opacity: 0.5; cursor: default; }
  .spacer { flex: 1; }
</style>
