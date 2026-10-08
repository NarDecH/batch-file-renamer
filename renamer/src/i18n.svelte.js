// Simple i18n + theme store (Svelte 5 runes-compatible module state)

export const translations = {
  th: {
    scanPath: "โฟลเดอร์:",
    scan: "สแกน",
    recursive: "รวมโฟลเดอร์ย่อย",
    filter: "กรอง (*.jpg, *.png)",
    rules: "กฎการเปลี่ยนชื่อ",
    addRule: "เพิ่มกฎ",
    preview: "พรีวิว",
    oldName: "ชื่อเดิม",
    newName: "ชื่อใหม่",
    folder: "โฟลเดอร์",
    size: "ขนาด",
    modified: "แก้ไขเมื่อ",
    status: "สถานะ",
    rename: "เปลี่ยนชื่อ",
    undo: "ย้อนกลับ",
    total: "ทั้งหมด",
    ready: "พร้อม",
    unchanged: "ไม่เปลี่ยน",
    warnings: "เตือน",
    errors: "ผิดพลาด",
    applyTo: "ใช้กับ",
    conflict: "เมื่อชื่อซ้ำ",
    strategyBlock: "ห้ามรันจนแก้",
    strategyAuto: "เติม (1) อัตโนมัติ",
    strategySkip: "ข้ามไฟล์นั้น",
    presets: "ชุดกฎ (Preset)",
    savePreset: "บันทึกชุดกฎ",
    language: "ภาษา",
    theme: "ธีม",
    scanning: "กำลังสแกน...",
    renaming: "กำลังเปลี่ยนชื่อ...",
    manual: "แก้มือ",
    search: "ค้นหาในตาราง",
  },
  en: {
    scanPath: "Folder:",
    scan: "Scan",
    recursive: "Include subfolders",
    filter: "Filter (*.jpg, *.png)",
    rules: "Rename rules",
    addRule: "Add rule",
    preview: "Preview",
    oldName: "Old name",
    newName: "New name",
    folder: "Folder",
    size: "Size",
    modified: "Modified",
    status: "Status",
    rename: "Rename",
    undo: "Undo",
    total: "Total",
    ready: "Ready",
    unchanged: "Unchanged",
    warnings: "Warnings",
    errors: "Errors",
    applyTo: "Apply to",
    conflict: "On conflict",
    strategyBlock: "Block until fixed",
    strategyAuto: "Auto-suffix (1)",
    strategySkip: "Skip",
    presets: "Presets",
    savePreset: "Save preset",
    language: "Language",
    theme: "Theme",
    scanning: "Scanning...",
    renaming: "Renaming...",
    manual: "manual",
    search: "Search in table",
  },
};

const storedLang = localStorage.getItem("lang") || "th";
const storedTheme = localStorage.getItem("theme") || "dark";

export const ui = $state({ lang: storedLang, theme: storedTheme });

export function t(key) {
  return translations[ui.lang]?.[key] ?? translations.en[key] ?? key;
}

export function setLang(lang) {
  ui.lang = lang;
  localStorage.setItem("lang", lang);
}

export function setTheme(theme) {
  ui.theme = theme;
  localStorage.setItem("theme", theme);
  document.documentElement.dataset.theme = theme;
}
