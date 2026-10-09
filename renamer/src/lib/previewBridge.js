export async function requestPreview(invoke, { rules, conflictStrategy, applyTo }) {
  await invoke("update_rules", { rules });
  await invoke("set_conflict_strategy", { strategy: conflictStrategy });
  await invoke("set_apply_to", { applyTo });
  return invoke("preview");
}
