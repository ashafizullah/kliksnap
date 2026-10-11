import { invoke } from "@tauri-apps/api/core";

/**
 * The windows' language: English, or Indonesian when Settings (or the
 * system, on "auto") asks for it. Text is looked up by its English wording,
 * which is also the fallback. Windows load the language once, before they
 * mount; Settings reloads itself when it changes.
 */
let indonesian = false;

export async function initI18n() {
  try {
    indonesian = (await invoke<string>("ui_language")) === "id";
  } catch {
    indonesian = false;
  }
  document.documentElement.lang = indonesian ? "id" : "en";
}

/** `en` in the current language, with `{name}` placeholders filled from `vars`. */
export function tr(en: string, vars: Record<string, string | number> = {}) {
  const text = (indonesian && ID[en]) || en;
  return text.replace(/\{(\w+)\}/g, (m, k) => (k in vars ? String(vars[k]) : m));
}

const ID: Record<string, string> = {
  // Shared
  Copy: "Salin",
  Save: "Simpan",
  Pin: "Sematkan",
  Edit: "Edit",
  Close: "Tutup",
  Cancel: "Batal",
  Done: "Selesai",
  Delete: "Hapus",
  Off: "Mati",
  Copied: "Disalin",
  Annotate: "Anotasi",
  "Color picker": "Pengambil warna",
  "Click to copy the color": "Klik untuk menyalin warna",
  "back to selecting": "kembali memilih area",
  "Keep on screen": "Tetap di layar",
  "Save & Copy": "Simpan & Salin",

  // Overlay
  "Drag over text to copy it": "Seret di atas teks untuk menyalinnya",
  "text only, skip QR codes": "teks saja, abaikan kode QR",
  Record: "Rekam",
  "Scrolling capture": "Tangkapan gulir",
  "Select the part that scrolls": "Pilih bagian yang bergulir",
  "Drag to select": "Seret untuk memilih",
  "Click a window": "Klik sebuah jendela",
  "window mode": "mode jendela",
  "area mode": "mode area",
  "copy color": "salin warna",
  "Scroll works": "Bisa digulir",
  cancel: "batal",

  // Preview, toast, countdown, recording
  Screenshot: "Tangkapan layar",
  "Show in folder": "Tampilkan di folder",
  "Click to show in {app}": "Klik untuk menampilkan di {app}",
  "the file manager": "pengelola file",
  "On Wayland these shortcuts don't work. Add one in your system's keyboard settings that runs:":
    "Di Wayland pintasan ini tidak berfungsi. Tambahkan pintasan di pengaturan keyboard sistem yang menjalankan:",
  "(or window, screen, text, record).": "(atau window, screen, text, record).",
  "Recording needs ffmpeg and an X11 session; it doesn't work on Wayland yet.":
    "Merekam butuh ffmpeg dan sesi X11; belum berfungsi di Wayland.",
  "Click to cancel": "Klik untuk membatalkan",
  "Recording time": "Durasi rekaman",
  Stop: "Stop",

  // Scrolling capture
  "Maximum height reached": "Tinggi maksimum tercapai",
  "Too fast: scroll back up a little": "Terlalu cepat: gulir ke atas sedikit",
  "Scroll down slowly": "Gulir ke bawah pelan-pelan",
  "Scrolling…": "Menggulir…",
  "Scroll by hand, or allow Accessibility to auto-scroll": "Gulir manual, atau izinkan Aksesibilitas untuk gulir otomatis",
  "Allow KlikSnap in System Settings → Privacy & Security → Accessibility, then start the capture again.":
    "Izinkan KlikSnap di System Settings → Privacy & Security → Accessibility, lalu mulai tangkapan lagi.",

  // Pin
  "Pinned screenshot": "Tangkapan layar yang disematkan",
  "Opacity {n}%": "Transparansi {n}%",
  "Clicks pass through · undo from the menu bar icon": "Klik tembus · batalkan dari ikon di bar menu",
  "Clicks pass through · undo from the tray icon": "Klik tembus · batalkan dari ikon di tray",
  "Clickable again": "Bisa diklik lagi",

  // History
  "Search text in captures": "Cari teks di tangkapan",
  Search: "Cari",
  "{shown} of {total}": "{shown} dari {total}",
  "Clear All": "Hapus Semua",
  "Click again to delete all": "Klik lagi untuk menghapus semua",
  "No captures yet": "Belum ada tangkapan",
  "Screenshots you take show up here, newest first.": "Tangkapan layar Anda muncul di sini, yang terbaru di atas.",
  "History is off": "Riwayat dimatikan",
  "Turn it on in Settings → Capture.": "Nyalakan di Pengaturan → Tangkap.",
  "No capture contains “{query}”.": "Tidak ada tangkapan yang berisi “{query}”.",
  Captures: "Tangkapan",
  "Capture from {when}": "Tangkapan dari {when}",
  Today: "Hari ini",
  Yesterday: "Kemarin",
  "Saved to your folder": "Disimpan ke folder Anda",

  // Editor
  Arrow: "Panah",
  Line: "Garis",
  Rectangle: "Persegi",
  Ellipse: "Elips",
  Pen: "Pena",
  Text: "Teks",
  Number: "Nomor",
  Highlight: "Stabilo",
  Blur: "Buram",
  Pixelate: "Piksel",
  Crop: "Pangkas",
  Tools: "Alat",
  Color: "Warna",
  "Color {c}": "Warna {c}",
  Undo: "Urungkan",
  Redo: "Ulangi",
  "Clear all": "Hapus semua",
  Thickness: "Ketebalan",
  "Font size": "Ukuran huruf",
  Size: "Ukuran",
  "Block size": "Ukuran blok",
  Strength: "Kekuatan",
  "This tool has no size": "Alat ini tidak punya ukuran",
  "{label} (1 2 3, [ ]; double-click to reset)": "{label} (1 2 3, [ ]; klik ganda untuk mengatur ulang)",
  "Hide emails, numbers, keys and passwords (OCR)": "Sembunyikan email, nomor, kunci, dan kata sandi (OCR)",
  "Auto redact": "Sensor otomatis",
  "Nothing sensitive found": "Tidak ada data sensitif",
  "Hid {n} item(s). Check the result, OCR can miss things":
    "{n} item disembunyikan. Periksa hasilnya, OCR bisa melewatkan sesuatu",
  "Redact failed: {e}": "Sensor gagal: {e}",
  "Export failed: {e}": "Ekspor gagal: {e}",
  Background: "Latar",
  "No background": "Tanpa latar",
  "{label} background": "Latar {label}",
  Padding: "Jarak",
  Corners: "Sudut",
  None: "Tidak ada",
  Shadow: "Bayangan",
  Frame: "Bingkai",
  Window: "Jendela",
  Phone: "Ponsel",
  Font: "Huruf",
  Bold: "Tebal",
  Ocean: "Samudra",
  Sunset: "Senja",
  Violet: "Ungu",
  Mint: "Mint",
  Dusk: "Petang",
  Graphite: "Grafit",
  White: "Putih",
  Black: "Hitam",
  Transparent: "Transparan",
  Share: "Bagikan",
  "Share (AirDrop, Messages, Mail…)": "Bagikan (AirDrop, Pesan, Mail…)",
  "Save As…": "Simpan Sebagai…",

  // Update
  "KlikSnap {version} is available": "KlikSnap {version} sudah tersedia",
  "You have {version}.": "Versi Anda {version}.",
  "No release notes.": "Tidak ada catatan rilis.",
  "Read more": "Baca selengkapnya",
  "Show less": "Tampilkan lebih sedikit",
  "Full release notes on GitHub": "Catatan rilis lengkap di GitHub",
  "Downloading… {percent}": "Mengunduh… {percent}",
  "Installing and restarting…": "Memasang dan memulai ulang…",
  Later: "Nanti",
  "Install and Restart": "Pasang dan Mulai Ulang",

  // Settings
  Settings: "Pengaturan",
  General: "Umum",
  Capture: "Tangkap",
  Recording: "Rekaman",
  Shortcuts: "Pintasan",
  "Free, open-source screenshots. Lives in your menu bar.":
    "Tangkapan layar gratis dan open source. Ada di bar menu Anda.",
  "Free, open-source screenshots. Lives in your system tray.":
    "Tangkapan layar gratis dan open source. Ada di system tray Anda.",
  Language: "Bahasa",
  "System ({lang})": "Sistem ({lang})",
  "Launch at login": "Buka saat login",
  "Show tray icon": "Tampilkan ikon tray",
  "KlikSnap keeps running on its shortcuts. Open it again from the Start menu to get back here.":
    "KlikSnap tetap berjalan lewat pintasannya. Buka lagi dari menu Start untuk kembali ke sini.",
  "Stop KlikSnap until you open it again": "Hentikan KlikSnap sampai Anda membukanya lagi",
  Quit: "Keluar",
  "Check for updates automatically": "Periksa pembaruan otomatis",
  "Version {v}": "Versi {v}",
  "Checking…": "Memeriksa…",
  "Check Now": "Periksa Sekarang",
  "Save location": "Lokasi penyimpanan",
  "Change…": "Ubah…",
  Selection: "Pemilihan",
  "Live screen while selecting": "Layar tetap hidup saat memilih",
  "Videos keep playing; the shot is taken when you finish selecting. Open menus may close first.":
    "Video tetap berjalan; gambar diambil saat Anda selesai memilih. Menu yang terbuka bisa tertutup lebih dulu.",
  "The screen freezes when you press the shortcut, so open menus and tooltips stay in the shot.":
    "Layar membeku saat pintasan ditekan, jadi menu dan tooltip yang terbuka ikut tertangkap.",
  "After capture": "Setelah menangkap",
  "Copy to clipboard": "Salin ke clipboard",
  AI: "AI",
  "AI model": "Model AI",
  "Any OpenAI-compatible API, with your own key: OpenAI, OpenRouter, Groq, Ollama and others.":
    "API apa pun yang OpenAI-compatible, dengan key Anda sendiri: OpenAI, OpenRouter, Groq, Ollama, dan lainnya.",
  "Base URL": "Base URL",
  "API key": "API key",
  Model: "Model",
  "Make sure the model can read images (vision); KlikSnap sends it your screenshots.":
    "Pastikan model bisa membaca gambar (vision); KlikSnap mengirim screenshot Anda ke model ini.",
  "Check the model reads an image": "Cek model bisa membaca gambar",
  "Testing…": "Mengetes…",
  Test: "Tes",
  Explain: "Jelaskan",
  Translate: "Terjemahkan",
  Summarize: "Ringkas",
  "Table to CSV": "Tabel ke CSV",
  "Ask AI": "Tanya AI",
  "Ask a follow-up…": "Tanya lanjutan…",
  Send: "Kirim",
  into: "ke",
  "AI answers in": "AI menjawab dalam",
  "The app's language": "Bahasa aplikasi",
  "Also what Translate translates into, unless you pick another language in the AI window.":
    "Juga bahasa tujuan Terjemahkan, kecuali Anda memilih bahasa lain di jendela AI.",
  Actions: "Aksi",
  "Buttons in the AI window. Explain runs the first one. {language} becomes the language above, or one you pick in the AI window.":
    "Tombol di jendela AI. Jelaskan menjalankan yang pertama. {language} diganti bahasa di atas, atau yang Anda pilih di jendela AI.",
  "Delete action": "Hapus aksi",
  "What to ask about the screenshot": "Apa yang ditanyakan tentang screenshot",
  Prompt: "Prompt",
  "Reset to presets": "Kembalikan preset",
  "Add action": "Tambah aksi",
  "Copy Text (OCR)": "Salin Teks (OCR)",
  "Read text with": "Baca teks dengan",
  "This device": "Perangkat ini",
  "The AI profile in use isn't set up, so this device reads the text.":
    "Profil AI yang dipakai belum diatur, jadi teks dibaca oleh perangkat ini.",
  "Set up an AI model in the AI tab to read text with AI.": "Atur model AI di tab AI untuk membaca teks dengan AI.",
  "Open AI tab": "Buka tab AI",
  "Sent to {name}. Better with handwriting, tables and mixed languages; needs the internet. If it fails, this device reads the text.":
    "Dikirim ke {name}. Lebih bagus untuk tulisan tangan, tabel, dan campuran bahasa; butuh internet. Kalau gagal, teks dibaca oleh perangkat ini.",
  "Private and offline; QR codes are always read on this device.":
    "Privat dan offline; kode QR selalu dibaca di perangkat ini.",
  Profile: "Profil",
  Name: "Nama",
  "Profile {n}": "Profil {n}",
  "Add profile": "Tambah profil",
  "Delete profile": "Hapus profil",
  Untitled: "Tanpa nama",
  "Explain with AI": "Jelaskan dengan AI",
  "Thinking…": "Sedang berpikir…",
  "AI Settings": "Setelan AI",
  "Ask Again": "Tanya Lagi",
  "Sound effects": "Efek suara",
  "Save to folder": "Simpan ke folder",
  Resolution: "Resolusi",
  "Max (100%)": "Maks (100%)",
  "Medium (75%)": "Sedang (75%)",
  "Low (50%)": "Rendah (50%)",
  "Smaller files, less detail.": "File lebih kecil, detail lebih sedikit.",
  "On a Retina display, Low matches the size things appear on screen.":
    "Di layar Retina, Rendah sama dengan ukuran tampilan di layar.",
  "Copy Text (OCR) always reads the full resolution.": "Salin Teks (OCR) selalu membaca resolusi penuh.",
  "Hide preview after": "Sembunyikan pratinjau setelah",
  "{n} seconds": "{n} detik",
  Never: "Tidak pernah",
  History: "Riwayat",
  "Keep recent captures": "Simpan tangkapan terbaru",
  "Find, copy or edit past captures": "Cari, salin, atau edit tangkapan lama",
  "Open History": "Buka Riwayat",
  "Stored only on this {device}, and searchable by the text in them.":
    "Hanya disimpan di {device} ini, dan bisa dicari berdasarkan teks di dalamnya.",
  File: "File",
  Format: "Format",
  "PNG (lossless)": "PNG (tanpa kompresi)",
  "JPG (smaller)": "JPG (lebih kecil)",
  "JPG quality": "Kualitas JPG",
  "File name": "Nama file",
  "Date fields: %Y year, %m month, %d day, %H hour, %M minute, %S second.":
    "Kolom tanggal: %Y tahun, %m bulan, %d hari, %H jam, %M menit, %S detik.",
  "Countdown before recording": "Hitung mundur sebelum merekam",
  "Smaller videos, less detail.": "Video lebih kecil, detail lebih sedikit.",
  "On a Retina display, Low records at the size things appear on screen.":
    "Di layar Retina, Rendah merekam sesuai ukuran tampilan di layar.",
  "Full detail; the largest files.": "Detail penuh; file terbesar.",
  Sound: "Suara",
  "Record computer sound": "Rekam suara komputer",
  "Record microphone": "Rekam mikrofon",
  "Computer sound needs macOS 13 or later, the microphone macOS 15. Each goes on its own track. GIFs have no sound.":
    "Suara komputer butuh macOS 13 atau lebih baru, mikrofon macOS 15. Masing-masing di track sendiri. GIF tidak bersuara.",
  "Both are mixed into one track. GIFs have no sound.": "Keduanya digabung dalam satu track. GIF tidak bersuara.",
  "Capture area": "Tangkap area",
  "Capture window": "Tangkap jendela",
  "Capture screen": "Tangkap layar",
  "Copy text (OCR)": "Salin teks (OCR)",
  "Capture last area": "Tangkap area terakhir",
  "Record area (again to stop)": "Rekam area (tekan lagi untuk berhenti)",
  "Not set": "Belum diatur",
  "Press keys…": "Tekan tombol…",
  "Click a shortcut, then press the new keys. Backspace clears it, Esc cancels.":
    "Klik sebuah pintasan, lalu tekan tombol barunya. Backspace menghapusnya, Esc membatalkan.",
  "Print Screen captures an area": "Print Screen menangkap area",
  "Replaces the Snipping Tool on the Print Screen key.": "Menggantikan Snipping Tool di tombol Print Screen.",
  Saved: "Tersimpan",
  "Free & open source · MIT License": "Gratis & open source · Lisensi MIT",
};
