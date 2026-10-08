<script>
  import { t } from "../i18n.svelte.js";

  let { summary, progressText, onRename, onUndo, renaming, progress = { done: 0, total: 0 } } = $props();

  const pct = $derived(
    progress.total > 0 ? Math.round((progress.done / progress.total) * 100) : 0
  );
  const showProgress = $derived(progress.total > 0 && progress.done < progress.total);
</script>

<div class="bar">
  <span class="stat">{t("total")}: <b>{summary.total}</b></span>
  <span class="stat ok">{t("ready")}: <b>{summary.ready}</b></span>
  <span class="stat">{t("unchanged")}: <b>{summary.unchanged}</b></span>
  <span class="stat warn">{t("warnings")}: <b>{summary.warnings}</b></span>
  <span class="stat err">{t("errors")}: <b>{summary.errors}</b></span>

  {#if showProgress}
    <div class="progress-wrap" title="{progress.done}/{progress.total}">
      <div class="progress-track">
        <div class="progress-fill" style="width: {pct}%"></div>
      </div>
      <span class="progress-label">{progress.done}/{progress.total} ({pct}%)</span>
    </div>
  {:else if progressText}
    <span class="progress-label">{progressText}</span>
  {/if}

  <span class="spacer"></span>
  <button class="undo" on:click={onUndo} disabled={renaming}>↩ {t("undo")} (Ctrl+Z)</button>
  <button class="rename" on:click={onRename} disabled={renaming || summary.ready + summary.warnings === 0}>
    ▶ {t("rename")} (Ctrl+Enter)
  </button>
</div>

<style>
  .bar {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 8px 12px;
    background: var(--panel);
    border-top: 1px solid var(--border);
    font-size: 13px;
  }
  .stat { color: var(--muted); }
  .ok b { color: var(--ok); }
  .warn b { color: var(--warn); }
  .err b { color: var(--err); }
  .progress-wrap {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 1;
    min-width: 160px;
    max-width: 340px;
  }
  .progress-track {
    flex: 1;
    height: 8px;
    border-radius: 4px;
    background: var(--bg);
    border: 1px solid var(--border);
    overflow: hidden;
  }
  .progress-fill {
    height: 100%;
    background: var(--accent);
    transition: width 0.15s ease;
  }
  .progress-label {
    color: var(--accent);
    font-size: 12px;
    white-space: nowrap;
  }
  .spacer { flex: 1; }
  button {
    border-radius: 6px;
    padding: 6px 16px;
    cursor: pointer;
    border: 1px solid var(--border);
    font-weight: 600;
  }
  .undo { background: var(--bg); color: var(--text); }
  .rename { background: var(--ok); border-color: var(--ok); color: #052e16; }
  button:disabled { opacity: 0.5; cursor: default; }
</style>
