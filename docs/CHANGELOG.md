# Changelog

รูปแบบอ้างอิง [Keep a Changelog](https://keepachangelog.com/) และใช้ [SemVer](https://semver.org/)

## [0.12.4] — 2026-10-16

### กระบวนการ
- **CI release มี NSIS quiet smoke test** — ติดตั้ง `setup.exe /S` สองรอบ (fresh + already-installed) แล้วปล่อยผ่านต่อเมื่อ installer จบเองภายในเวลาที่กำหนด ไม่มีหน้าต่าง UI, ไม่มี process ค้าง, มี app exe และเวอร์ชันตรงกับ tag — ป้องกันบั๊กหน้า "Already Installed" ซ่อน (v0.12.3) กลับมาซ้ำ
- **Updater watchdog** — ถ้าการดาวน์โหลด/ติดตั้งยังไม่จบใน 90 วินาทีหลังกด "ติดตั้งและรีสตาร์ท" แอปจะ log ข้อผิดพลาดและเปลี่ยนแถบ updater เป็นข้อความแจ้งว่าการติดตั้งดูเหมือนติดขัด พร้อมแนะนำให้ปิดโปรแกรมและติดตั้งจาก setup โดยตรง (แทนการรอเงียบ ๆ ตลอดไป) — ปรับเวลาได้ในโหมดพัฒนาด้วย environment ตัวแปร `VITE_UPDATER_STUCK_MS` (มิลลิวินาที) เพื่อทดสอบกับ installer จริงที่กินเวลานานกว่า
- แก้บั๊ก Svelte 5: `state`/`message` ใน Updater ไม่ได้ประกาศ `$state(...)` ทำให้ค่าเปลี่ยนแล้วไม่ re-render — ชนิดเดียวกับบั๊กตารางว่างใน v0.12.1
- **หน้า Postmortem ของ docs** — `RELEASES.html`: รวมไทม์ไลน์ 0.12.0 → 0.12.3 อาการจริง + วิธีแก้ + การรับประกันการกลับมาซ้ำ + คำแนะนำอัปเดตแยกตามเวอร์ชันปัจจุบันของผู้ใช้

## [0.12.3] — 2026-10-15

### Fixed
- **การอัปเดตอัตโนมัติแฮงก์เมื่อมีเวอร์ชันเก่าอยู่ในเครื่อง** — ตัวติดตั้ง NSIS ของ updater ใช้โหมด passive (/P) โดยเริ่มต้น ทำให้เด้งหน้าเลือก "Already Installed" (หน้าต่างซ่อนจากผู้ใช้) แล้วค้างจนติดตั้งไม่เดินต่อ — พิสูจน์บนเครื่องจริงตอนอัปเดต; ตั้ง `plugins.updater.windows.installMode = "quiet"` (/S) และ lock `bundle.windows.nsis.installMode = "currentUser"` ให้ติดตั้งต่อผู้ใช้เงียบ ๆ จนจบโดยไม่ถาม (ตรวจสอบกับ source template ของ Tauri + เอกสาร updater อย่างเป็นทางการ)
  หมายเหตุ: ผู้ใช้ที่อยู่รุ่น 0.12.x หรือเก่ากว่า ที่เคยแฮงก์ตอนอัปเดตผ่านแอป ให้ดาวน์โหลดและรันตัวติดตั้งรุ่นนี้โดยตรง (หรือรันด้วย /S) หลังจากนี้การอัปเดตผ่านแอปจะเดินต่ออัตโนมัติเอง

### กระบวนการ
- เพิ่มประกาศรุ่นของ release บน GitHub ที่อธิบายการแก้บั๊ก WebView2 confirm/alert และคำแนะนำการอัปเดตอย่างชัดเจน

## [0.12.2] — 2026-10-15

### Fixed
- **กล่องยืนยันหายไปทั้งแอป (สำคัญ สุด)** — WebView2 runtime ล่าสุดทำให้ `window.confirm()`/`alert()` ถูกตอบรับโดยอัตโนมัติโดยไม่ถามผู้ใช้ — กด "ล้างรายการ" จึงล้างข้อมูลทันทีโดยไม่เด้งกล่องผ่านครั้งเดียว (พิสูจน์แล้วบนเครื่องจริงทั้ง dev และตัวติดตั้ง v0.12.0); ย้ายทุกจุดยืนยันและแจ้งเตือนไปใช้ dialog แบบ native ของ `plugin-dialog` (ask()/message()) ที่รับประกันแสดงจริง — ยืนยันด้วย E2E: ล้างรายการ → กล่องถามเด้ง → กด Yes → รายการว่าง
- **แถบแจ้งเตือน non-blocking ในแอป** — สแกนสำเร็จ/ล้มเหลว, พรีวิวล้มเหลว, ส่งออก CSV, ย้อนกลับ ฯลฯ แจ้งบนแถบเฉดสีที่หายเองใน 8 วิ + ปุ่มปิด, แทนที่ alert แบบบล็อกที่ผู้ใช้ต้องกดปิดทุกครั้ง; แก้ปัญหาที่ความล้มเหลวแจ้งแค่ใน log หน้าจอไม่ขึ้นอะไรเลย

### กระบวนการ
- **รวม E2E UIA เข้า CI release** — workflow สร้างสแกน, กดปุ่มจริงบนหน้าต่างแอปตัวติดตั้งของ release ผ่าน Windows UIA, ตรวจว่าตารางแสดงรายชื่อไฟล์จริงและกล่องยืนยันกลาง flow "ล้างรายการ" ทำงานครบ
- e2e.ps1 ขยายความสามารถ: รอแถบแจ้งเตือนหายก่อนจับดัชนีปุ่ม (หลบ double-hit), ค้นหากล่อง task-dialog (#32770) ผ่าน EnumWindows และคลิกปุ่ม Yes ผ่านเมาส์จริง (task dialog ไม่รับ BM_CLICK/InvokePattern)

## [0.12.1] — 2026-10-15

### Fixed
- **ตารางพรีวิวไม่แสดงรายชื่อไฟล์** — สแกนสำเร็จทาง backend ทุกครั้งแต่ UI แสดง "ทั้งหมด: 0" และตารางว่างเปล่า เป็นเพราะแอปเข้าโหมด runes ของ Svelte 5 แต่ตัวแปร state ยังคงเป็น `let` ธรรมดา เลยไม่รีเรนเดอร์ตอนข้อมูลมาถึง; แก้เป็น `$state()` ทั้งหมด — ตรวจพิสูจน์ด้วย E2E คลิกจริง (UIA): กด "เปิดโฟลเดอร์" → ตารางแสดงรายชื่อไฟล์จริงทันที
- **ปุ่ม "ล้างรายการ" ยังไม่ทำงาน** — `clear_entries` ไม่ถูกลงทะเบียนใน `generate_handler` จะกดก็ได้ "command not found"; ลงทะเบียนแล้วและยืนยันด้วย E2E: กดปุ่ม → ยืนยัน -> ตารางว่างสนิทพร้อม log "preview round: 0 items"
- ตรวจสอบปุ่ม "เปิดโฟลเดอร์" / "+ เพิ่มโฟลเดอร์" / "สแกนใหม่" ด้วย E2E จริงทั้งสาย — ทำงานถูกต้องตามความหมายที่แยกไว้ใน v0.12.0 (แทนชุด / append / รีสแกน path ในช่อง)
- **vite dev server ค้างหรือชนกับ build ฝั่ง Rust** — หยุด watcher ไม่ให้อ่านโฟลเดอร์ `target/debug` ที่ dev app ใช้อยู่

### กระบวนการ
- เพิ่มเครื่องมือ E2E บนเดสก์ท็อป: `renamer/ops/e2e.ps1` ขับปุ่มจริงด้วย Windows UIA (scan / open / add / clear / dialog), อ่าน log backend และต้นไม้ accessibility เพื่อยืนยันผลหน้าจอจริง

## [0.12.0] — 2026-10-14

### Added
- **ปุ่ม "🗑 ล้างรายการ"** — เคลียร์รายการทั้งหมดในชุดทำงาน (กฎ/พรีเซ็ตไม่ถูกแตะ) พร้อมกล่องยืนยันก่อนล้างทุกครั้ง; ปุ่ม disabled เมื่อชุดว่าง
- **แยกความหมายปุ่มโฟลเดอร์ 3 อย่างชัดเจน** — "📂 เปิดโฟลเดอร์" = ล้างทุกอย่างแล้วสแกนตัวใหม่เข้าชุดทันที, "✚ เพิ่มโฟลเดอร์" = append เข้าชุดปัจจุบันไม่ทำรายการเดิมหาย, "สแกนใหม่" = รีสแกน path ในช่องซ้ายแบบแทนที่ (รวม tooltip กำกับทุกปุ่ม)
- **เลือกแถวในตารางได้ (checkbox)** — กดเลือก/ยกเลิกทีละแถวในตารางพรีวิว (สถานะเก็บใน backend), ตัวเลือก "เลือกทั้งหมด/ไม่เลือกทั้งหมด" ในแถบบนตารางพร้อมนับจำนวน, ปุ่ม "🗑 ลบที่เลือกออกจากชุด" เอาเฉพาะแถวที่เลือกออก (ไฟล์บนดิสก์ไม่ถูกลบ เช็คสองหน้า)
- **Export คู่ชื่อ old,new (Mapping CSV)** — ปุ่ม "⤓ คู่ชื่อ old,new" ส่งออกคู่ชื่อเดิม→ชื่อใหม่ของทั้งชุดในรูปแบบเดียวกับกฎ Import List (quoting ตาม RFC 4180, ทดสอบครอบ), หยิบกลับมาใช้กับกฎ Import List รอบถัดไปได้ทันที

### พิสูจน์บนเครื่องจริง (จากรุ่น v0.11.0)
- ผู้ใช้สแกนโฟลเดอร์จริง 6,076 ไฟล์ผ่านปุ่มเพิ่มโฟลเดอร์ใหม่: 405 ms และ 3,037 ไฟล์: 306 ms — ประสิทธิภาพสแกนคงเดิม

## [0.11.0] — 2026-10-14

### Added
- **เลือกโฟลเดอร์แล้วสแกนทันที** — กดปุ่มเลือกโฟลเดอร์/"+ เพิ่มโฟลเดอร์" แล้วโปรแกรมสแกนให้เลยโดยไม่ต้องกดปุ่ม "สแกน" อีกครั้ง (เลือกใหม่ = แทนชุดเดิม, + เพิ่ม = เพิ่มเข้าชุดปัจจุบันโดยสแกนเฉพาะโฟลเดอร์ใหม่ ไม่ซ้ำรายการเดิม)
- **ลากโฟลเดอร์จาก Explorer ลงหน้าต่าง** (drag & drop) — โฟลเดอร์ที่ปั่นลงมาถูกสแกนและเพิ่มเข้าชุดปัจจุบันทันที
- **กรองตารางตามสถานะ** — ตัวเลือกด้านหน้าตาราง: ทุกสถานะ / พร้อม / ไม่เปลี่ยน / เตือน / ผิดพลาด / แก้มือ ทำงานคู่กับช่องค้นหาโดยใช้ฟังก์ชันกรองร่วมกันกับการส่งออก
- **ส่งออกพรีวิวเป็น CSV** — ปุ่ม "⤓ ส่งออก CSV" บันทึกแถวตามเงื่อนไขค้นหา/กรองที่เห็นในตาราง (ไดอะลอกบันทึกไฟล์, UTF-8 + BOM, CRLF, escape ตาม RFC 4180 — เปิดกับ Excel ได้ตรง ๆ) + ทดสอบครอบการ escape ทุกเคสและฟังก์ชันกรองหลัก

## [0.10.3] — 2026-10-13

### Fixed
- **แก้ URL `latest.json` ให้ตรงชื่อ asset จริงบน GitHub** — GitHub rename ไฟล์ที่อัปโหลดด้วยชื่อมีช่องว่างเป็นเวอร์ชันจุด (`Batch Renamer_0.10.2_x64-setup.exe` ถูกเก็บเป็น `Batch.Renamer_0.10.2_x64-setup.exe`) ทำให้ต่อจาก encode `%20` (v0.10.2) ยังเจอ 404 อยู่; CI ใช้ชื่อรูปจุดให้ตรงทันที (ยืนยันด้วย curl: `%20` = 404, จุด = 200)

### หมายเหตุ
- v0.10.1/v0.10.2 ใน release เอาไปติดตั้งด้วยมือได้ แต่ดาวน์โหลดจากในแอปไม่ได้ (manifest 404) ตั้งแต่ v0.10.3 เข้าที่ใช้ได้ครบ

## [0.10.2] — 2026-10-13

### Fixed
- **ในแอปอัปเดต 404 จริงถูกพบและแก้** — ทดสอบ end-to-end ที่แอปติดตั้ง 0.10.0 (ดาวน์โหลดตาม `latest.json`) เจอ `Download request failed with status: 404` เพราะ URL ของ installer ใน `latest.json` ไม่ได้ encode ช่องว่างในชื่อไฟล์ (`Batch Renamer_x64-setup.exe`); CI แก้เป็น `%20` แล้วสร้างแท็กนี้เป็นเวอร์ชันที่ถูกต้องเพื่อพิสูจน์ (v0.10.1 ที่ publish ไปก่อนหน้าจึงดาวน์โหลดจากในแอปไม่ได้ — ติดตั้งด้วยมือหรือรอใช้ 0.10.2 พร้อมนี้)

### หมายเหตุ (release ทดสอบ)
- นอกจากแก้ CI แล้วเนื้อหาโปรแกรมเหมือน 0.10.0/0.10.1 จุดประสงค์คือพิสูจน์ผ่านปลายทางจริงว่าแอปที่ติดตั้ง 0.10.0 เห็นแบนเนอร์ "มีเวอร์ชันใหม่" และกดอัปเดต+รีสตาร์ทเป็น 0.10.2 ได้จริง

## [0.10.1] — 2026-10-13

### หมายเหตุ (release ทดสอบ)
- Release นี้มีเนื้อหาเดียวกับ 0.10.0 เพียง bump เวอร์ชัน จุดประสงค์คือพิสูจน์ผ่านปลายทางจริงว่าแอปที่ติดตั้ง 0.10.0 เห็นแบนเนอร์ "มีเวอร์ชันใหม่" และกดอัปเดต+รีสตาร์ทเป็น 0.10.1 ได้จริง (แท็กก่อนหน้าเคยยืนยันได้เพียง "endpoint ตอบสนอง ผ่าน")

### Added
- **Auto-update ในตัว (Tauri updater)** — เปิดโปรแกรมจะตรวจ release ใหม่จาก GitHub Releases อัตโนมัติ (ช่วง dev ไม่ตรวจ); เมื่อพบเวอร์ชันใหม่จะขึ้นแถบด้านล่าง พร้อมปุ่ม "ติดตั้งและรีสตาร์ท" — ดาวน์โหลด NSIS installer ที่เซ็นลายเซ็น minisign (TAURI_SIGNING_PRIVATE_KEY ใน GitHub secrets) แล้วตรวจ pubkey ฝั่งแอปก่อนติดตั้งเสมอ
- CI สร้าง updater artifacts อัตโนมัติ — `latest.json` (manifest) + `.exe.sig` ถูกสร้างและแนบไปกับ GitHub Release ทุกครั้งที่ tag
- CLI รับหลาย path — `-f` ใช้ซ้ำได้หรือคั่นด้วย `;` เหมือน backend GUI (ปิดข้อจำกัด v0.9.0)

### หมายเหตุ
- แชนเนลอัปเดตใช้ NSIS installer (per-user) — ถ้าติดตั้งด้วย MSI ต้องติดตั้งเวอร์ชันใหม่ด้วยมือครั้งหนึ่ง จากนั้น auto-update จะดูแลต่อเอง
- ติดตั้งครั้งแรกยังเจอ SmartScreen เพราะไม่มี Authenticode cert (Tauri signature ต่างจาก code signing)

## [0.9.0] — 2026-10-12

### Added
- **สแกนหลายโฟลเดอร์ในชุดเดียว (GUI)** — เลือกโฟลเดอร์หลายที่พร้อมกัน (multi-select) หรือกด "เพิ่มโฟลเดอร์" เพื่อ append เข้าชุดปัจจุบัน; backend `scan_paths` รองรับ path หลายที่คั่นด้วย `;` และโหมด append ที่คุม id ไม่ให้ชนกัน
- **ตัวกรองไฟล์ตามขนาดและวันที่ (GUI)** — ช่อง min/max MB + วันที่แก้ไขจาก/ถึง ใน Toolbar (backend รองรับมานานแล้ว แต่ยังไม่มีหน้า UI)
- **Landing page (GitHub Pages) อัปเดต** — ปุ่มดาวน์โหลด release ล่าสุด, เนื้อหากฎเลขใหม่ Renumber สองโหมด, ลิงก์ changelog ปัจจุบัน
- GUI ติดตั้งจริง v0.8.0: ติดตั้งผ่าน NSIS `/S` แล้วเปิดรันเสถียร, CLI-on-install ยืนยันกฎ Renumber relative ได้ผลถูกต้อง, ปิดสะอาด

## [0.8.0] — 2026-10-11

### Added
- **เตือน shift-chain ใน preview** — เมื่อกฎ Renumber (relative) ทำให้ชื่อใหม่ของไฟล์ A ตรงกับชื่อเดิมของไฟล์ B ที่กำลังจะถูกเปลี่ยนชื่อในชุดเดียวกัน (เช่น `a_002`+16 = `a_018` ขณะที่ `a_021` กำลังย้ายออก) แถวนั้นจะขึ้นสถานะ Warning พร้อมข้อความบอกว่าชื่อดังกล่าวจะว่างในภายหลังของ batch เดียวกัน — executor จัดการลำดับ safe อยู่แล้ว แต่ผู้ใช้เห็นความเสี่ยงชัดเจน

### Changed
- **รวมกฎเลขเป็นกฎเดียว: Renumber มี 2 โหมด** — `relative` (บวก/ลบเลขเดิมของไฟล์เองด้วย offset, ต่างจาก v0.7.0) และ `absolute` (แทนที่ด้วยเลขลำดับตำแหน่งในชุด — ย้ายมาจากกฎ Numbering เดิม รวม prefix/suffix/replace, separator, reset_per_dir) — ลดจำนวนกฎใน UI และหนึ่งพารามิเตอร์ชุดเดียว
- preset `photos.json` + built-in preset ของ CLI migrate เป็นรูปแบบใหม่แล้ว
- Tests ใหม่: shift-chain warning + absolute mode — รวม 19 tests

## [0.7.0] — 2026-10-11

### Changed
- **กฎ Renumber เป็นการบวก/ลบแบบสัมพันธ์ (relative offset)** — เปลี่ยนเลขในชื่อไฟล์แต่ละไฟล์ตามค่าที่ผู้ใช้กำหนด โดยคำนวณจาก **เลขเดิมของไฟล์นั้นเอง** เช่น `IMG_0100.jpg` +25 → `IMG_0125.jpg`, `IMG_0999.jpg` +1 → `IMG_1000.jpg` (ไม่จำกัด 4 หลัก) — ต่างจาก v0.6.0 ที่แทนด้วยเลขลำดับในชุด
- พารามิเตอร์ใหม่: `offset` (จำนวนจริง, ติดลบได้), `target` (`first` / `last` / `all` — เลือกว่าจะเลื่อน digit-run ไหนเมื่อชื่อมีหลายเลข), `pad` (ความยาวขั้นต่ำ, ไม่ตัดเลขที่ยาวกว่า), `floor_one` (ค่าติดลบ clamp เป็น 1 แทน 0)
- ไฟล์ที่ไม่มีตัวเลขในชื่อจะไม่ถูกแตะ (ปลอดภัยกว่าเดิมที่ append เลข)
- Tests: อัปเดต 3 เคสครอบ offset ติดลบ, floor_one/without, target=all และ execute บนดิสก์จริง

## [0.6.0] — 2026-10-11

### Added
- **กฎ Renumber** — เปลี่ยนเลขเดิมในชื่อไฟล์ให้เป็นเลขลำดับใหม่ตามตำแหน่งในชุด เช่น `IMG_1234.jpg` → `IMG_001.jpg`, `video_2023_001.mp4` → `video_2023_015.mp4` — คนละงานกับ Numbering (ที่แทรกเลขต่อท้าย): Renumber **แทน** digit-run แรกของ base name ด้วยเลขใหม่ แล้วตัด digit-run ที่เหลือออก
- พารามิเตอร์: `start`, `step`, `pad`, `reset_per_dir` — ไฟล์ที่ไม่มีตัวเลขในชื่อจะได้เลขต่อท้ายเป็น `_001` อัตโนมัติ (ไม่มีชื่อซ้ำ)
- Preset ตัวอย่าง `renumber.json` — เปลี่ยนเลขเดิมทุกไฟล์เป็นลำดับใหม่ทั้งชุด (ไม่ reset ต่อโฟลเดอร์)
- เขียน base-only ในโหมด Full: Renumber ไม่แตะนามสกุล (`.jpg` ถูกเก็บไว้เสมอ)
- Tests: 3 automation tests ครอบ replace-number / no-digit fallback / conflict-free execute บนดิสก์จริง

## [0.5.0] — 2026-10-10

### Added
- **CI version guard** — เมื่อ tag `v*` workflow เทียบเวอร์ชันใน `Cargo.toml` / `tauri.conf.json` กับ tag ก่อน build ไม่ตรงจะ fail ทันที กันลืม bump
- **E2E test ใน CI** — ทุก build รัน CLI `apply` + `undo` จริงบน temp dir (rename 2 ไฟล์ แล้ว undo ต้องกลับครบ) กัน regression ฝั่ง journal/history
- docs/README + AGENT.md อัปเดตสารสำคัญ v0.3.0–v0.4.0 (Undo history, CLI undo, CI auto release notes)

## [0.4.0] — 2026-10-10

### Added
- **Release notes อัตโนมัติใน CI** — เมื่อ tag `v*` workflow สร้าง notes จากรายการ commits ระหว่าง tag ก่อนหน้ากับ tag ใหม่ (`git describe` + `git log`) เขียนลง release ผ่าน `--notes-file` แทน `--generate-notes` ของ GitHub
- ทดสอบ GUI ที่ติดตั้งจริง (MSI) เปิดรันได้ ไม่ crash, preview round เริ่มทำงาน, log รายวันบันทึกถูกต้อง

## [0.3.0] — 2026-10-10

### Added
- **Undo UI: ประวัติการเปลี่ยนชื่อทั้งชุด** — ซ้อนการพับเพิ่มเติมในแผงตาราง แสดงรายการ batch เก่าทั้งหมด (เวลา, จำนวน, รหัสชุด) พร้อมปุ่ม `ย้อนกลับถึงจุดนี้` ต่อชุด; ย้อนไปที่ชุดกลาง ๆ จะถอนชุดที่ใหม่กว่าทั้งหมดพร้อมกัน กลับสู่สถานะดิสก์ก่อนชุดนั้น
- คำสั่ง IPC ใหม่: `undo_batch` (undo ตาม batch id + กัน busy ซ้อนรอบ)
- **CI อัตโนมัติ (GitHub Actions)** — `cargo test` + frontend tests ทุก push/PR ไป main; เมื่อ tag `v*` บิลด์ installers (NSIS + MSI) และ publish GitHub Release อัตโนมัติ
- Frontend tests ใหม่ 6 รายการสำหรับ undo flow และ Rust test "undo ชุดกลางเมื่อชุดใหม่ทับ path เดิม"

### Fixed
- **CLI `undo` ไม่เคยเห็น batch ของตัวเอง** — `renamer-cli --apply` ไม่เคยบันทึก batch ลง history ทำให้ `undo` ชี้ไป batch เก่าของ GUI เสมอ; ตอนนี้บันทึกทุกครั้งที่ apply สำเร็จ (ยืนยันด้วยการติดตั้ง MSI จริงแล้ว apply + undo รอบเดียวสำเร็จ)

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
