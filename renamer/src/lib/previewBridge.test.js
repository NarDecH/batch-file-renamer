import assert from "node:assert/strict";
import test from "node:test";
import { requestPreview } from "./previewBridge.js";

test("sends current rules and settings before requesting preview", async () => {
  const rules = [{ enabled: true, params: { type: "case", style: "upper" } }];
  const calls = [];
  const expectedPreview = { items: [], total: 0, ready: 0, unchanged: 0, warnings: 0, errors: 0 };
  const invoke = async (command, args) => {
    calls.push([command, args]);
    return command === "preview" ? expectedPreview : undefined;
  };

  const preview = await requestPreview(invoke, {
    rules,
    conflictStrategy: "auto_suffix",
    applyTo: "name",
  });

  assert.deepEqual(calls, [
    ["update_rules", { rules }],
    ["set_conflict_strategy", { strategy: "auto_suffix" }],
    ["set_apply_to", { applyTo: "name" }],
    ["preview", undefined],
  ]);
  assert.equal(preview, expectedPreview);
});

test("does not request a preview when syncing settings fails", async () => {
  const calls = [];
  const invoke = async (command, args) => {
    calls.push(command);
    if (command === "set_conflict_strategy") throw new Error("IPC failure");
  };

  await assert.rejects(
    requestPreview(invoke, { rules: [], conflictStrategy: "block", applyTo: "full" }),
    /IPC failure/
  );
  assert.deepEqual(calls, ["update_rules", "set_conflict_strategy"]);
});
