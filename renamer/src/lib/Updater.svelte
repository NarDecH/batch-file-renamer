<script>
  import { check } from "@tauri-apps/plugin-updater";
  import { relaunch } from "@tauri-apps/plugin-process";
  import { t } from "../i18n.svelte.js";

  let state = "idle"; // idle | available | downloading | error
  let message = "";
  let update = null;

  async function checkNow() {
    try {
      update = await check();
      if (update) {
        message = `${t("updateAvailable")} (v${update.version})`;
        state = "available";
      }
    } catch (e) {
      console.warn("updater check failed:", e);
    }
  }

  async function install() {
    if (!update) return;
    state = "downloading";
    message = t("updateDownloading");
    try {
      await update.downloadAndInstall();
      // Windows: installer exits for us; relaunch otherwise.
      await relaunch();
    } catch (e) {
      console.error(e);
      message = `${t("updateError")}: ${e}`;
      state = "error";
    }
  }

  checkNow();
</script>

{#if state === "available" || state === "downloading" || state === "error"}
  <div class="updater" role="status">
    <span>{message}</span>
    {#if state === "available"}
      <button class="primary" on:click={install}>{t("updateInstall")}</button>
      <button class="ghost" on:click={() => (state = "idle")}>{t("updateDismiss")}</button>
    {/if}
  </div>
{/if}

<style>
  .updater {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 14px;
    background: var(--panel, #1e293b);
    border-top: 1px solid var(--border, #334155);
    font-size: 13px;
    color: var(--text, inherit);
    z-index: 50;
  }
  .primary {
    padding: 3px 12px;
    border: 1px solid var(--accent, #60a5fa);
    background: var(--accent, #60a5fa);
    color: var(--bg, #0f172a);
    border-radius: 6px;
    cursor: pointer;
    font-weight: 600;
  }
  .ghost {
    padding: 3px 10px;
    border: 1px solid var(--border, #334155);
    background: transparent;
    color: var(--muted, #94a3b8);
    border-radius: 6px;
    cursor: pointer;
  }
</style>
