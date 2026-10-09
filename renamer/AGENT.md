# AGENT.md — คู่มือสำหรับ AI Agent ที่ทำงานใน repo นี้

## ภาพรวมโปรเจกต์

**Batch File Renamer** — โปรแกรมเปลี่ยนชื่อไฟล์/โฟลเดอร์จำนวนมาก ปลอดภัยต่อข้อมูลเป็นอันดับหนึ่ง
- **Core engine:** Rust (`src-tauri/src/core/`) — ล้วน ๆ ไม่มี UI import ใช้ร่วมกันโดย GUI และ CLI
- **GUI:** Tauri 2 + Svelte 5 (`src/`)
- **CLI:** Rust binary แยก (`renamer-cli`, `src-tauri/src/cli/`) — dry-run เป็นค่าเริ่มต้น
- **แพลตฟอร์มเป้าหมายหลัก:** Windows (โค้ดเขียน cross-platform ไว้)

## คำสั่งที่ใช้บ่อย

```bash
cd renamer
npm install              # ครั้งแรก
npm run build            # build frontend → dist/
npm run tauri dev        # เปิด GUI dev พร้อมเริ่ม Vite
npm test                 # frontend IPC/preview regression tests
cd src-tauri
cargo check              # ตรวจ compile เร็ว
cargo test               # unit + acceptance tests (14 tests)
cargo bench              # preview benchmark (10k ไฟล์ × 5 กฎ)
# อย่าใช้ cargo run เปิด GUI dev: ต้องเริ่ม Vite; CLI: cargo run --bin renamer-cli -- --help
cd .. && npm run tauri build   # สร้าง installer NSIS/MSI
```

**ต้องมี:** Rust stable-msvc, Node 18+, WebView2 Runtime

## โครงสร้างและกฎการแก้โค้ด

```
src-tauri/src/core/     # ⛔ ห้าม import tauri/UI — ทดสอบด้วย cargo test ได้ต้อง
  models.rs             #   ชนิดข้อมูลกลาง (FileEntry, PlanItem, ...)
  scanner.rs            #   สแกนขนาน jwalk — ห้าม follow symlink โดยดีฟอลต์
  rules/pipeline.rs     #   RuleSpec/RuleParams (serde tag "type")
  rules/impls.rs        #   ตรรกะกฎทุกตัว — grapheme-aware เสมอ
  preview.rs            #   pipeline + validation + conflict resolve
  executor.rs           #   rename จริง: chain/cycle, journal, undo
  logger.rs             #   ระบบ log ไฟล์รายวัน (ดูหัว Logging)
src-tauri/src/commands.rs  # Tauri IPC — งานหนักต้อง spawn_blocking เสมอ
src/                    # Svelte — param ของกฎต้องเป็น snake_case ตาม serde
```

### ข้อห้าม (เคยเจอปัญหาแล้ว)

1. **ห้ามเขียนทับไฟล์** — `std::fs::rename` เขียนทับปลายทางได้ทั้งบน Windows และ Unix ต้องผ่าน `executor::do_rename` เสมอ (มีการเช็คปลายทาง + รองรับ case-only rename และ cycle)
2. **ห้าม compile regex ในลูปต่อไฟล์** — ใช้ `RuleContext.regex_cache` (ไม่งั้น 10k ไฟล์ช้า 54 เท่า)
3. **การตัด/แทรกข้อความต้องใช้ grapheme** (`unicode_segmentation`) ไม่ใช่ `char` — กันสระ/วรรณยุกต์ไทยลอย
4. **ชื่อ field ของกฎใน frontend ต้องเป็น snake_case** (`replace_with` ไม่ใช่ `replaceWith`) ตาม serde ของ `RuleParams`
5. แก้ serialization ของกฎทุกครั้งต้องเช็ค: preset JSON (`src/presets/`), frontend (`RulesPanel.svelte`), และ test

### เพิ่มกฎใหม่ (checklist)

1. เพิ่ม variant ใน `RuleParams` (`rules/pipeline.rs`) + ใส่ตรรกะใน `impls.rs::apply_rule`
2. ถ้ากฎต้องแตะเฉพาะ base name ในโหมด Full → เพิ่มใน `apply_full_aware`
3. เพิ่ม UI ใน `RulesPanel.svelte` (`RULE_TYPES` + `blank()` + template ของ body)
4. เขียน test ใน `tests/acceptance.rs`
5. เพิ่ม preset ตัวอย่างถ้าเหมาะสม

## Logging

- `core::logger::init(mirror_stdout)` เรียกครั้งเดียวตอน start (มีทั้ง GUI และ CLI แล้ว)
- ไฟล์: `<data>/batch-renamer/logs/renamer-YYYY-MM-DD.log`, ควบคุม level ด้วย `RENAMER_LOG=debug`
- ใช้ macro `log::info! / warn! / error! / debug!` พร้อม target ชัดเจน เช่น `log::debug!("renamed: {} -> {}", ...)`
- จุดที่ต้องมี log: สแกน (เริ่ม/จบ/จำนวน), preview round (เวลา+สรุป), rename ทุกครั้ง (debug), ความล้มเหลว (error), undo, ผู้ใช้ยกเลิก

## Testing

- Frontend IPC/preview tests อยู่ใน `src/lib/previewBridge.test.js`; รันจาก `renamer/` ด้วย `npm test`
- Test ทุกกฎใหม่ + ทุกแก้ไข executor ใน `tests/acceptance.rs`
- Benchmark อยู่ใน `benches/preview_bench.rs` — ห้ามให้ preview 10k ช้ากว่า ~200 ms
- `cargo test` ต้องผ่าน 100% ก่อนเสนองาน

## เกณฑ์ยอมรับ (อย่าทำลาย)

ดู `docs/README.md` ตารางเกณฑ์ 1–8 — โดยเฉพาะ: ห้ามไฟล์หาย, ห้ามเขียนทับไม่ได้ตั้งใจ, Undo ได้หลังปิดโปรแกรม, grapheme ไทยปลอดภัย, dry-run ไม่แตะดิสก์
