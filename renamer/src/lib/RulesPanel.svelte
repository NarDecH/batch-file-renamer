<script>
  import { invoke } from "@tauri-apps/api/core";
  import { open } from "@tauri-apps/plugin-dialog";
  import { t } from "../i18n.svelte.js";

  let { rules = $bindable([]), onRulesChanged } = $props();

  const RULE_TYPES = [
    { id: "replace", label: "Replace" },
    { id: "regex", label: "Regex" },
    { id: "insert", label: "Insert" },
    { id: "remove", label: "Remove" },
    { id: "case", label: "Case" },
    { id: "numbering", label: "Numbering" },
    { id: "extension", label: "Extension" },
    { id: "date_time", label: "Date/Time" },
    { id: "metadata", label: "Metadata" },
    { id: "template", label: "Template" },
    { id: "cleanup", label: "Clean up" },
    { id: "swap", label: "Swap" },
    { id: "import_list", label: "Import List" },
    { id: "hash", label: "Hash/UUID" },
  ];

  // NOTE: param keys must match the Rust enum field names (serde snake_case).
  function blank(type) {
    switch (type) {
      case "replace":
        return { type, find: "", replace_with: "", case_sensitive: false, occurrence: "all", use_regex: false };
      case "regex":
        return { type, pattern: "", replacement: "$1" };
      case "insert":
        return { type, text: "", at: "end", position: 1, from_right: false };
      case "remove":
        return { type, mode: "chars", count: 1, from_right: false, start: 0, end: 0 };
      case "case":
        return { type, style: "upper" };
      case "numbering":
        return { type, start: 1, step: 1, pad: 3, mode: "suffix", separator: "_", reset_per_dir: true };
      case "extension":
        return { type, action: "lowercase", extension: "" };
      case "date_time":
        return { type, source: "modified", format: "%Y-%m-%d", mode: "suffix", separator: "_" };
      case "metadata":
        return { type, field: "artist", mode: "suffix", separator: " - ", fallback: "" };
      case "template":
        return { type, template: "{name}_{n:03}.{ext}" };
      case "cleanup":
        return { type, trim: true, collapse_spaces: true, space_replacement: "", strip_forbidden: true, strip_accents: false, normalize_nfc: true };
      case "swap":
        return { type, delimiter: " - " };
      case "import_list":
        return { type, mapping: [], ordered_names: [] };
      case "hash":
        return { type, algo: "md5", length: 0, keep_extension: true };
      default:
        return { type };
    }
  }

  function addRule(e) {
    const type = e.target.value;
    if (!type) return;
    rules = [...rules, { params: blank(type), enabled: true, apply_to: null, only_if_glob: null }];
    e.target.value = "";
    changed();
  }

  function removeRule(i) {
    rules = rules.filter((_, idx) => idx !== i);
    changed();
  }

  function move(i, dir) {
    const j = i + dir;
    if (j < 0 || j >= rules.length) return;
    const next = [...rules];
    [next[i], next[j]] = [next[j], next[i]];
    rules = next;
    changed();
  }

  function toggle(i) {
    rules[i] = { ...rules[i], enabled: !rules[i].enabled };
    changed();
  }

  function changed() {
    onRulesChanged?.(rules);
  }

  async function importList() {
    const file = await open({
      multiple: false,
      title: "Import list (CSV/TXT)",
      filters: [{ name: "Text", extensions: ["csv", "txt", "tsv"] }],
    });
    if (typeof file !== "string") return;
    try {
      const data = await invoke("parse_import_list", { path: file });
      if (!data.mapping.length && !data.orderedNames.length) {
        alert("ไม่พบรายชื่อในไฟล์");
        return;
      }
      rules = [
        ...rules,
        {
          params: { type: "import_list", mapping: data.mapping, ordered_names: data.orderedNames },
          enabled: true,
          apply_to: null,
          only_if_glob: null,
        },
      ];
      changed();
      alert(`นำเข้าแล้ว: ${data.mapping.length} คู่ชื่อ, ${data.orderedNames.length} ชื่อเรียงลำดับ`);
    } catch (e) {
      alert("อ่านไฟล์ไม่สำเร็จ: " + e);
    }
  }

  async function savePreset() {
    const name = prompt("Preset name:");
    if (!name) return;
    try {
      await invoke("save_preset", { name, rules });
      alert("saved: " + name);
    } catch (e) {
      alert(e);
    }
  }

  async function loadPreset() {
    try {
      const list = await invoke("list_presets");
      if (!list.length) {
        alert("no presets saved yet");
        return;
      }
      const name = prompt("Preset name (one of: " + list.join(", ") + "):", list[0]);
      if (!name) return;
      rules = await invoke("load_preset", { name });
      changed();
    } catch (e) {
      alert(e);
    }
  }
</script>

<div class="panel">
  <div class="head">
    <strong>{t("rules")}</strong>
    <span class="spacer"></span>
    <button on:click={savePreset}>{t("savePreset")}</button>
    <button on:click={loadPreset}>{t("presets")}</button>
  </div>

  <select class="add" on:change={addRule}>
    <option value="">+ {t("addRule")}</option>
    {#each RULE_TYPES as rt}
      <option value={rt.id}>{rt.label}</option>
    {/each}
  </select>
  <button class="import-btn" on:click={importList}>📂 Import List จาก CSV/TXT</button>

  {#each rules as rule, i}
    <div class="rule" class:disabled={!rule.enabled}>
      <div class="rule-head">
        <input type="checkbox" checked={rule.enabled} on:change={() => toggle(i)} />
        <strong>{RULE_TYPES.find((r) => r.id === rule.params.type)?.label ?? rule.params.type}</strong>
        {#if rule.params.type === "import_list"}
          <span class="dim">{rule.params.mapping.length} คู่ / {rule.params.ordered_names.length} ชื่อ</span>
        {/if}
        <span class="spacer"></span>
        <button title="up" on:click={() => move(i, -1)}>↑</button>
        <button title="down" on:click={() => move(i, 1)}>↓</button>
        <button title="remove" on:click={() => removeRule(i)}>✕</button>
      </div>
      <div class="rule-body">
        {#if rule.params.type === "replace"}
          <label>find <input bind:value={rule.params.find} on:input={changed} /></label>
          <label>→ <input bind:value={rule.params.replace_with} on:input={changed} /></label>
          <label><input type="checkbox" bind:checked={rule.params.case_sensitive} on:change={changed} /> case-sensitive</label>
          <label><input type="checkbox" bind:checked={rule.params.use_regex} on:change={changed} /> regex</label>
        {:else if rule.params.type === "regex"}
          <label>pattern <input bind:value={rule.params.pattern} on:input={changed} /></label>
          <label>replace <input bind:value={rule.params.replacement} on:input={changed} /></label>
        {:else if rule.params.type === "insert"}
          <label>text <input bind:value={rule.params.text} on:input={changed} /></label>
          <select bind:value={rule.params.at} on:change={changed}>
            <option value="start">start</option><option value="end">end</option><option value="position">position</option>
          </select>
        {:else if rule.params.type === "remove"}
          <select bind:value={rule.params.mode} on:change={changed}>
            <option value="chars">chars</option><option value="digits">digits</option>
            <option value="symbols">symbols</option><option value="spaces">spaces</option>
            <option value="brackets">brackets</option>
          </select>
          {#if rule.params.mode === "chars"}
            <label>count <input type="number" bind:value={rule.params.count} on:input={changed} /></label>
          {/if}
        {:else if rule.params.type === "case"}
          <select bind:value={rule.params.style} on:change={changed}>
            <option value="upper">UPPER</option><option value="lower">lower</option>
            <option value="title">Title</option><option value="sentence">Sentence</option>
            <option value="camel">camelCase</option><option value="snake">snake_case</option>
            <option value="kebab">kebab-case</option>
          </select>
        {:else if rule.params.type === "numbering"}
          <label>start <input type="number" bind:value={rule.params.start} on:input={changed} /></label>
          <label>pad <input type="number" bind:value={rule.params.pad} on:input={changed} /></label>
          <label>sep <input bind:value={rule.params.separator} on:input={changed} /></label>
        {:else if rule.params.type === "extension"}
          <select bind:value={rule.params.action} on:change={changed}>
            <option value="lowercase">lowercase</option><option value="set">set</option><option value="remove">remove</option>
          </select>
          {#if rule.params.action === "set"}
            <label>ext <input bind:value={rule.params.extension} on:input={changed} /></label>
          {/if}
        {:else if rule.params.type === "date_time"}
          <select bind:value={rule.params.source} on:change={changed}>
            <option value="modified">modified</option><option value="created">created</option><option value="exif">EXIF</option>
          </select>
          <label>format <input bind:value={rule.params.format} on:input={changed} /></label>
        {:else if rule.params.type === "metadata"}
          <select bind:value={rule.params.field} on:change={changed}>
            <option value="artist">artist</option><option value="title">title</option>
            <option value="album">album</option><option value="track">track</option>
            <option value="exif_camera">camera</option><option value="exif_size">size</option>
          </select>
        {:else if rule.params.type === "template"}
          <label>template <input bind:value={rule.params.template} on:input={changed} /></label>
        {:else if rule.params.type === "cleanup"}
          <label>space→ <input bind:value={rule.params.space_replacement} on:input={changed} /></label>
          <label><input type="checkbox" bind:checked={rule.params.strip_forbidden} on:change={changed} /> forbidden</label>
        {:else if rule.params.type === "swap"}
          <label>delimiter <input bind:value={rule.params.delimiter} on:input={changed} /></label>
        {:else if rule.params.type === "hash"}
          <select bind:value={rule.params.algo} on:change={changed}>
            <option value="md5">MD5</option><option value="sha1">SHA-1</option><option value="uuid">UUID</option>
          </select>
        {:else if rule.params.type === "import_list"}
          <span class="dim">mapping: {rule.params.mapping.length} · ordered: {rule.params.ordered_names.length}</span>
        {/if}
      </div>
    </div>
  {/each}
</div>

<style>
  .panel {
    width: 320px;
    min-width: 280px;
    background: var(--panel);
    border-right: 1px solid var(--border);
    padding: 10px;
    overflow-y: auto;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-bottom: 8px;
  }
  .spacer { flex: 1; }
  .add {
    width: 100%;
    margin-bottom: 6px;
    padding: 5px;
    background: var(--bg);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 6px;
  }
  .import-btn {
    width: 100%;
    margin-bottom: 8px;
    padding: 5px;
    background: var(--bg);
    color: var(--text);
    border: 1px dashed var(--border);
    border-radius: 6px;
    cursor: pointer;
    font-size: 12px;
  }
  .rule {
    border: 1px solid var(--border);
    border-radius: 6px;
    margin-bottom: 8px;
    padding: 6px;
  }
  .rule.disabled { opacity: 0.45; }
  .rule-head {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .rule-head button {
    border: none;
    background: transparent;
    color: var(--muted);
    cursor: pointer;
    padding: 0 4px;
  }
  .rule-body {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 6px;
    font-size: 12px;
  }
  .rule-body label {
    display: flex;
    align-items: center;
    gap: 3px;
    color: var(--muted);
  }
  .dim { color: var(--muted); font-size: 11px; }
  input, select {
    background: var(--bg);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 2px 5px;
  }
  .rule-body input:not([type="checkbox"]) { width: 100px; }
  button {
    background: var(--bg);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 3px 8px;
    cursor: pointer;
  }
</style>
