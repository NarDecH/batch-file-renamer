<script>
  import { invoke } from "@tauri-apps/api/core";
  import { createEventDispatcher, onMount } from "svelte";
  import { t } from "../i18n.svelte.js";
  import { remainingBatchesAfter } from "./undoFlow.js";

  export let batches = [];
  const dispatch = createEventDispatcher();

  let undoingId = null;
  let message = "";

  function formatDate(ms) {
    const d = new Date(ms);
    return d.toLocaleString(undefined, {
      year: "numeric", month: "2-digit", day: "2-digit",
      hour: "2-digit", minute: "2-digit",
    });
  }

  async function undoBatch(id) {
    if (!id) return;
    undoingId = id;
    message = "";
    try {
      const msg = await invoke("undo_batch", { batchId: id });
      message = msg;
      // The Rust side reverts the clicked batch and everything after it.
      batches = remainingBatchesAfter(batches, id);
      dispatch("undone");
    } catch (e) {
      message = "ย้อนกลับไม่ได้: " + e;
    } finally {
      undoingId = null;
    }
  }

  async function refresh() {
    try {
      batches = await invoke("list_undo_history");
    } catch (e) {
      console.error(e);
    }
  }

  onMount(refresh);
</script>

<details class="undo-history" data-testid="undo-history">
  <summary>
    🕘 {t("undoHistory")} ({batches.length})
  </summary>
  {#if message}
    <div class="msg" data-testid="undo-message">{message}</div>
  {/if}
  {#if batches.length === 0}
    <div class="empty">{t("undoHistoryEmpty")}</div>
  {:else}
    <table>
      <thead>
        <tr><th>{t("undoBatchTime")}</th><th>{t("undoBatchCount")}</th><th>{t("undoBatchId")}</th><th></th></tr>
      </thead>
      <tbody>
        {#each batches as b, i (b.batch_id)}
          <tr data-testid={`undo-row-${i}`}>
            <td>{formatDate(b.created_at_ms)}</td>
            <td>{b.renamed}</td>
            <td class="id">{b.batch_id.slice(0, 8)}</td>
            <td>
              <button
                class="undo-btn"
                data-testid={`undo-btn-${i}`}
                disabled={undoingId !== null}
                on:click={() => undoBatch(b.batch_id)}
              >{undoingId === b.batch_id ? "…" : t("undoBatchBtn")}</button>
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}
</details>

<style>
  .undo-history {
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--panel);
    margin-top: 8px;
    font-size: 13px;
  }
  summary {
    cursor: pointer;
    padding: 8px 10px;
    font-weight: 600;
    user-select: none;
  }
  .empty {
    padding: 0 10px 8px;
    color: var(--muted);
  }
  .msg {
    padding: 0 10px 8px;
    color: var(--ok);
    word-break: break-all;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    margin-bottom: 4px;
  }
  th, td {
    padding: 4px 10px;
    text-align: left;
    border-top: 1px solid var(--border);
  }
  th {
    color: var(--muted);
    font-weight: 600;
  }
  .id {
    font-family: monospace;
    color: var(--muted);
  }
  .undo-btn {
    padding: 2px 10px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--bg);
    color: var(--text);
    cursor: pointer;
  }
  .undo-btn:hover:not(:disabled) {
    border-color: var(--accent);
    color: var(--accent);
  }
  .undo-btn:disabled {
    opacity: 0.5;
    cursor: default;
  }
</style>
