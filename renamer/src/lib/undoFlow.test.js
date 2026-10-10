import assert from "node:assert/strict";
import test from "node:test";
import { remainingBatchesAfter, undoBatchFlow } from "./undoFlow.js";

const history = [
  { batch_id: "b1", created_at_ms: 1, renamed: 3 },
  { batch_id: "b2", created_at_ms: 2, renamed: 5 },
  { batch_id: "b3", created_at_ms: 3, renamed: 2 },
];

test("undo to the oldest batch clears the whole list", () => {
  assert.deepEqual(remainingBatchesAfter(history, "b1"), []);
});

test("undo to a middle batch keeps only older batches", () => {
  const rest = remainingBatchesAfter(history, "b2");
  assert.deepEqual(
    rest.map((b) => b.batch_id),
    ["b1"]
  );
});

test("undo to the newest batch keeps all older ones", () => {
  const rest = remainingBatchesAfter(history, "b3");
  assert.deepEqual(
    rest.map((b) => b.batch_id),
    ["b1", "b2"]
  );
});

test("unknown batch id leaves the list unchanged", () => {
  assert.deepEqual(remainingBatchesAfter(history, "nope"), history);
});

test("undo_batch IPC is called with camelCase batchId param", async () => {
  const calls = [];
  const invoke = async (command, args) => {
    calls.push([command, args]);
    return "undone=5, skipped=0, errors=[]";
  };

  const { message, batches } = await undoBatchFlow(invoke, history, "b2");

  assert.deepEqual(calls, [["undo_batch", { batchId: "b2" }]]);
  assert.match(message, /undone=5/);
  assert.deepEqual(
    batches.map((b) => b.batch_id),
    ["b1"]
  );
});

test("undo flow propagates IPC failures", async () => {
  const invoke = async () => {
    throw new Error("another rename is already running");
  };
  await assert.rejects(undoBatchFlow(invoke, history, "b2"), /already running/);
});
