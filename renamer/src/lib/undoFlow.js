//! Pure logic for the batch undo history, kept out of the Svelte component
//! so it can be regression-tested with plain node --test.

/// Result shape of the "undo to this batch" invocation: all batches from the
/// clicked one onward are reverted (newest first) by the Rust side.
export function remainingBatchesAfter(history, clickedBatchId) {
  const pos = history.findIndex((b) => b.batch_id === clickedBatchId);
  if (pos === -1) return history;
  // Batches older than the clicked one remain; the clicked one and every
  // newer one were reverted together.
  return history.slice(0, pos);
}

/// Invoke `undo_batch` for one batch id, returning the message and the
/// remaining history rows to render.
export async function undoBatchFlow(invoke, history, batchId) {
  const msg = await invoke("undo_batch", { batchId });
  return { message: msg, batches: remainingBatchesAfter(history, batchId) };
}
