import { test } from "node:test";
import assert from "node:assert/strict";
import { filterItems } from "./filters.js";

const items = [
  { id: 1, oldName: "IMG_0001.jpg", newName: "2024-01-01_001.jpg", parent: "D:\\Trip", status: "ready" },
  { id: 2, oldName: "IMG_0002.jpg", newName: "2024-01-01_002.jpg", parent: "D:\\Trip", status: "unchanged" },
  { id: 3, oldName: "song.mp3", newName: "síntesis.mp3", parent: "C:\\Music", status: "warning" },
  { id: 4, oldName: "doc.pdf", newName: "doc-v2.pdf", parent: "C:\\Docs", status: "error" },
  { id: 5, oldName: "manual.txt", newName: "my-name.txt", parent: "C:\\Docs", status: "manual" },
];

test("search matches old/new/parent case-insensitively", () => {
  const got = filterItems(items, "img_", "all");
  assert.deepEqual(got.map((i) => i.id), [1, 2]);
  assert.equal(filterItems(items, "music", "all").length, 1);
  assert.equal(filterItems(items, "SÍNTE", "all").length, 1);
});

test("status filter keeps only that status", () => {
  const got = filterItems(items, "", "ready");
  assert.deepEqual(got.map((i) => i.id), [1]);
  assert.equal(filterItems(items, "", "error").length, 1);
  assert.equal(filterItems(items, "", "manual").length, 1);
});

test("empty query and 'all' returns everything", () => {
  assert.equal(filterItems(items, "", "all").length, items.length);
});

test("search also filters folder names", () => {
  const got = filterItems(items, "docs", "all");
  assert.deepEqual(got.map((i) => i.id), [4, 5]);
});
