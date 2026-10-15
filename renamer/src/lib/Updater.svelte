<script>
  import { check } from "@tauri-apps/plugin-updater";
  import { relaunch } from "@tauri-apps/plugin-process";
  import { t } from "../i18n.svelte.js";

  let { notify = null } = $props(); // (kind, text, ms) -> non-blocking notice in App.svelte

  let state = $state("idle"); // idle | available | downloading | stuck | error
  let message = $state("");
  let update = $state(null);

  // Watchdog: a pending downloadAndInstall() used to wait silently forever when
  // the NSIS installer stalled on a hidden wizard page (v0.12.2 bug). Now the
  // app tells the user after a grace period instead of waiting quietly.
  // Tunable for testing on a real machine: set VITE_UPDATER_STUCK_MS to a
  // small value in a dev session so the watchdog fires while the (real, slow)
  // installer is still running. Production default stays 90s.
  const STUCK_AFTER_MS = Number(import.meta.env?.VITE_UPDATER_STUCK_MS) || 90_000;
  let stuckTimer = null;

  function startWatchdog() {
    stopWatchdog();
    stuckTimer = setTimeout(() => {
      stuckTimer = null;
      state = "stuck";
      message = t("updateStuck");
      console.error(`[updater] install still pending after ${STUCK_AFTER_MS / 1000}s — flagging as stuck`);
      notify?.("err", message, 0); // 0 = stays until the user closes it
    }, STUCK_AFTER_MS);
  }

  function stopWatchdog() {
    if (stuckTimer) {
      clearTimeout(stuckTimer);
      stuckTimer = null;
    }
  }

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
    startWatchdog();
    try {
      await update.downloadAndInstall();
      stopWatchdog();
      // Windows: installer exits for us; relaunch otherwise.
      await relaunch();
    } catch (e) {
      stopWatchdog();
      console.error(e);
      message = `${t("updateError")}: ${e}`;
      state = "error";
    }
  }

  checkNow();
</script>

{#if state === "available" || state === "downloading" || state === "stuck" || state === "error"}
  <div class="updater updater-{state}" role="status">
    <span>{message}</span>
    {#if state === "available"}
      <button class="primary" onclick={install}>{t("updateInstall")}</button>
      <button class="ghost" onclick={() => { state = "idle"; stopWatchdog(); }}>{t("updateDismiss")}</button>
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
  .updater-stuck,
  .updater-error {
    border-top-color: var(--err, #f87171);
    color: var(--err, #f87171);
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
  }
</style>
