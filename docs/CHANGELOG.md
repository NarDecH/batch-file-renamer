# Changelog

รูปแบบอ้างอิง [Keep a Changelog](https://keepachangelog.com/) และใช้ [SemVer](https://semver.org/)

## [0.2.0] — 2026-10-09

### Added
- **Native folder picker** ในหน้าสแกน ผ่าน `tauri-plugin-dialog`
- **Import List จากไฟล์** — กดปุ่มในแผงกฎ เลือกไฟล์ CSV/TXT แล้วสร้างกฎ Import List อัตโนมัติ (รองรับทั้งแบบคู่ชื่อ `old,new` และแบบเรียงลำดับ)
- **Progress bar แบบ real-time** ระหว่างเปลี่ยนชื่อ — Rust ส่ง event `rename-progress` ทุก 50 รายการขึ้น UI
- **ระบบ log รายวัน** (`core/logger.rs`) — ไฟล์ `%APPDATA%/batch-renamer/logs/renamer-YYYY-MM-DD.log` ควบคุม level ด้วย `RENAMER_LOG=debug` ใช้ร่วม GUI และ CLI; log ครอบการสแกน, พรีวิวทุกรอบ, rename ทุกไฟล์ (debug), ความล้มเหลว, undo, rollback
- คำสั่ง IPC ใหม่: `parse_import_list`, `log_file_path`
- Tauri capabilities file (`core:default`, `dialog:default`) — แก้ IPC โดน deny ตอน runtime
- `AGENT.md` คู่มือสำหรับ AI agent
- เอกสารใน `docs/`: README / RESEARCH / CHANGELOG ทั้ง Markdown และ HTML + แผนภาพ SVG + index page
- Frontend regression tests สำหรับลำดับการซิงก์กฎ/ตัวเลือกผ่าน IPC ก่อนพรีวิว และการหยุดเมื่อ IPC ล้มเหลว

### Fixed
- **Cargo entry point:** `cargo run` เปิด GUI ตามที่ระบุในคู่มือ; เพิ่ม binary `renamer-cli` สำหรับใช้งาน CLI โดยตรง
- **GUI dev launch:** ใช้ `npm run tauri dev` เพื่อให้ Tauri เริ่ม Vite dev server ก่อนเปิด WebView; `cargo run` อย่างเดียวจะเจอ `localhost:5173` refused
- ลบ Rust imports/variables/functions ที่ไม่ได้ใช้ เพื่อให้ `cargo run` และ `cargo test` ไม่มี compiler warnings
- **GUI preview IPC:** ลงทะเบียนคำสั่ง `update_rules`, `set_conflict_strategy`, `set_apply_to` และซิงก์ค่าปัจจุบันก่อนเรียก `preview`; เปลี่ยนตัวเลือกแล้วพรีวิวอัปเดตทันที
- **Virtual preview table:** วางแถวที่มองเห็นตามตำแหน่ง scroll เพื่อไม่ให้แถวซ้อนกันเมื่อเลื่อน
- **Preview HTML safety:** escape ชื่อไฟล์ก่อนแสดง diff ด้วย HTML
- **Performance:** กฎ Regex/Replace(regex) compile ซ้ำทุกไฟล์ → เพิ่ม RegexCache พรีวิว 10k ไฟล์เร็วขึ้น **54×** (3,270 ms → 60.6 ms)
- **Performance (executor):** collision check O(n²) → O(1) map lookup, dependency sort O(n³) → Kahn topological O(n) — execute 10k ไฟล์ **4.23 s → 2.72 s (−36%)**
- **Performance (journal):** เปลี่ยนจากเขียน JSON ทั้งไฟล์ทุก flush → append-only JSONL (เขียนเฉพาะรายการใหม่) ลด disk writes ใหญ่ ตอน crash ยังอ่านกลับได้ (torn-tail tolerant)
- **Performance (rename):** ไฟล์ที่ไม่เกี่ยวกัน rename ขนานด้วย rayon — chain/cycle ยังวิ่งแบบเรียงลำดับพร้อม temp-name เหมือนเดิม; เพิ่ม acceptance test 500 ไฟล์ขนาน + mixed batch
- **Cycle rename:** การสลับชื่อไฟล์ A↔B ล้มเหลวเพราะเช็คปลายทางบนดิสก์บังการ rename และรอบเปลี่ยนชื่อ temp กลับหา item ไม่เจอ — แก้ทั้งสองจุดใน executor
- **Numbering ในโหมด Full:** เลขรันเคยแทรกหลังนามสกุล (`file.jpg_001`) → แทรกก่อนนามสกุลเสมอ
- **Frontend/serde mismatch:** พารามิเตอร์กฎจาก UI เปลี่ยนเป็น snake_case ให้ตรงกับ Rust enum

## [0.1.0] — 2026-10-09

### Added
- Core engine (Rust): สแกนขนาน (jwalk), กฎ 14 ประเภท grapheme-aware, validator, executor จัดการ chain/cycle, journal write-ahead + Undo ข้าม restart
- GUI (Tauri 2 + Svelte 5): live preview virtual list, ไฮไลต์ diff, แก้ชื่อมือได้, preset save/load, i18n ไทย/อังกฤษ, ธีม Dark/Light, คีย์ลัด
- CLI: `preview` / `undo` / `presets`, dry-run ค่าเริ่มต้น, exit codes
- Automated tests ครบเกณฑ์ยอมรับ 1–7 + benchmark เกณฑ์ 8 (~60 ms สำหรับ 10k ไฟล์)
- Build installer NSIS + MSI

### วิธีทดสอบปัจจุบัน
- Frontend: `cd renamer && npm test` (2 tests) และ `npm run build`
- Rust: `cd renamer/src-tauri && cargo test` (14 tests) และ `cargo bench`
