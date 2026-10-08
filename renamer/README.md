# Batch File Renamer

โปรแกรมเปลี่ยนชื่อไฟล์และโฟลเดอร์จำนวนมากพร้อมกัน — ปลอดภัย (ไม่มีไฟล์หาย/ถูกเขียนทับ), เร็ว (พรีวิว 10,000 ไฟล์ × 5 กฎ ≈ 60 ms), เห็นผลก่อนเปลี่ยนจริงเสมอ ทำงานบน Rust core + Tauri 2 GUI พร้อม CLI ใช้ engine เดียวกัน

## สถาปัตยกรรม

```
renamer/
├── src/                    # Svelte 5 GUI
│   ├── App.svelte          # Layout + debounce preview + คีย์ลัด
│   ├── i18n.svelte.js      # ไทย/อังกฤษ + Dark/Light
│   └── lib/
│       ├── Toolbar.svelte  # สแกน, scope, conflict strategy, ธีม
│       ├── RulesPanel.svelte  # เพิ่ม/ลบ/สลับกฎ 14 ประเภท + preset
│       ├── PreviewTable.svelte# Virtual list + ไฮไลต์ส่วนที่เปลี่ยน
│       └── StatusBar.svelte# สรุปจำนวน + Rename/Undo
├── src-tauri/
│   ├── src/core/           # Engine ล้วน ไม่ import UI
│   │   ├── scanner.rs      # สแกนขนาน (jwalk), ไม่ตาม symlink
│   │   ├── rules/          # 14 กฎ + grapheme-safe ops + regex cache
│   │   ├── preview.rs      # Pipeline + validation + collision resolve
│   │   ├── executor.rs     # Chain/cycle temp-name, journal, undo
│   │   ├── metadata.rs     # EXIF (kamadak-exif) + แท็กเพลง (lofty)
│   │   └── natsort.rs      # Natural sort
│   ├── src/commands.rs     # Tauri IPC (background threads ทั้งหมด)
│   ├── src/cli/            # CLI (clap, dry-run เป็นค่าเริ่มต้น)
│   └── tests/acceptance.rs # เกณฑ์ยอมรับ 1-7 อัตโนมัติ
└── scripts/make_icon.ps1   # สร้าง icon.ico
```

เหตุผลสำคัญ:

- **ความปลอดภัย > ความถูกต้อง > ความเร็ว**: rename ตรวจปลายทางบนดิสก์อีกครั้งก่อนทุกครั้ง, เขียน journal (write-ahead) ก่อนเริ่ม, ยกเลิกกลางทางแล้ว rollback คืนทั้งชุด, วน cycle ผ่าน temp name ที่ต้องถูกเปลี่ยนเป็นชื่อจริงเสมอ
- **เร็ว**: สแกนขนาน, regex compile ครั้งเดียวต่อรอบ (cache), metadata อ่านแบบ lazy และ cache, ตาราง render เฉพาะแถวที่มองเห็น (virtual list), ส่งข้อมูลชุด ไม่ทีละไฟล์

## ติดตั้งและรัน (dev)

ต้องมี Rust (stable-msvc), Node 18+ และ WebView2

```bash
cd renamer
npm install
npm run build          # สร้าง dist/
cd src-tauri
cargo run              # เปิดแอป GUI (หรือ npm run tauri dev ที่ root)
```

## วิธี build แจกจ่าย

```bash
cd renamer
npm run tauri build    # ได้ .msi / NSIS installer ใน src-tauri/target/release/bundle/
```

ตัว exe ปกติอยู่ที่ `renamer/src-tauri/target/release/renamer.exe` (ใช้เป็น GUI หรือ CLI ได้จากไฟล์เดียวกัน)

## ใช้งาน CLI

ค่าเริ่มต้นคือ dry-run ต้องใส่ `--apply` จึงเปลี่ยนชื่อจริง:

```bash
renamer preview -f "D:\Photos" -r --include "*.jpg" --include "*.png" -p photos
renamer preview -f "D:\Music" -r --apply -p music
renamer undo
renamer presets
```

Exit codes: `0` สำเร็จ, `1` ไม่มีอะไรให้ undo, `2` สแกน/พรีวิวผิดพลาด, `3` เปลี่ยนชื่อบางรายการล้มเหลว

## ตัวอย่างการใช้งานจริง

### 1) ตั้งชื่อรูปตามวันที่ถ่ายจาก EXIF
กฎ: `Date/Time` (source=exif, format=`%Y-%m-%d`, prefix `_`) + `Numbering` (pad 3, suffix `_`) → `IMG_20240315_143022.JPG` → `2024-03-15_001.JPG`
ใช้ preset ในตัว: **photos**

### 2) จัดชื่อเพลงเป็น Artist - Title
กฎ: `Swap` (delimiter ` - `) หรือใช้ `Template` `{artist} - {title}` + `Clean up` (ลบอักขระต้องห้าม)
ใช้ preset ในตัว: **music**

### 3) แปลงชื่อเป็นชื่อไฟล์ URL-friendly สำหรับเว็บ
กฎ: `Clean up` (strip accents, space→`-`, strip forbidden) + `Case` (lower) → `Café Ñandú Ep 1.mp4` → `cafe-nandu-ep-1.mp4`

### 4) สลับชื่อไฟล์สองไฟล์ในชุดเดียว
แก้ชื่อใหม่ด้วยมือในตาราง (ดับเบิลคลิกช่องชื่อใหม่) ให้ a.txt→b.txt และ b.txt→a.txt — engine ตรวจพบ cycle และใช้ชื่อชั่วคราวให้อัตโนมัติ

### 5) ใส่เลขรันแบบ natural sort
เรียง ep1, ep2, …, ep10 ถูกลำดับแล้วใส่ `Numbering` (pad 3) ได้ `ep001`, `ep002`, `ep010`

## การทดสอบ

```bash
cd renamer/src-tauri
cargo test        # 14 tests: เกณฑ์ยอมรับ 1-7 + validation + chain + dry-run
cargo bench       # preview 10,000 ไฟล์ × 5 กฎ (ผลล่าสุด ~60 ms)
```

## เกณฑ์ยอมรับ (ผลจริง)

| เกณฑ์ | ผล |
|---|---|
| 1. Pipeline ครบ → `Trip_2024-03-15_001.jpg` | ✅ test `criterion_1` |
| 2. สลับ a.txt ↔ b.txt เนื้อหาถูกต้อง | ✅ test `criterion_2` |
| 3. photo.JPG → photo.jpg ไม่ถูกมองว่าซ้ำ | ✅ test `criterion_3` |
| 4. ชื่อซ้ำแจ้งเตือน + auto-suffix | ✅ test `criterion_4` |
| 5. Natural sort + เลขรัน 001, 002, 003 | ✅ test `criterion_5` |
| 6. Undo หลังปิดโปรแกรม (journal บนดิสก์) | ✅ test `criterion_6` |
| 7. ไทย/อีโมจิ ลบ 1 ตัวแรกไม่มีสระลอย | ✅ test `criterion_7` |
| 8. Preview 10k × 5 กฎ ≤ 200 ms | ✅ ~60 ms (bench) |

## หมายเหตุ

- ชื่อสงวนของ Windows (CON, PRN, …), อักขระต้องห้าม, ชื่อท้ายจุด/ช่องว่าง, ยาว >255 byte ถูกตรวจและกันไว้
- ชนชื่อกัน: เลือกได้ 3 โหมด (ห้ามรัน / เติม (1) อัตโนมัติ / ข้าม)
- ปิดโปรแกรมกลางทาง: journal ค้างสถานะ in-progress และ `undo` ยังกู้คืนได้
