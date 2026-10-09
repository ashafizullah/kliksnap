//! The language of the menus and notices: English or Indonesian, from
//! Settings or the system's language. Text is looked up by its English
//! wording, which is also the fallback.

use std::sync::atomic::{AtomicBool, Ordering};

static INDONESIAN: AtomicBool = AtomicBool::new(false);

/// Applies the Settings value: "en", "id", or "auto" for the system's language.
pub fn set(setting: &str) {
    let id = match setting {
        "id" => true,
        "en" => false,
        _ => sys_locale::get_locales()
            .next()
            .is_some_and(|l| is_indonesian(&l)),
    };
    INDONESIAN.store(id, Ordering::Relaxed);
}

fn is_indonesian(locale: &str) -> bool {
    let lang = locale.split(['-', '_']).next().unwrap_or("");
    lang.eq_ignore_ascii_case("id") || lang.eq_ignore_ascii_case("in")
}

/// "en" or "id", for the webviews.
pub fn code() -> &'static str {
    if INDONESIAN.load(Ordering::Relaxed) {
        "id"
    } else {
        "en"
    }
}

/// `en` in the current language.
pub fn tr(en: &'static str) -> &'static str {
    if !INDONESIAN.load(Ordering::Relaxed) {
        return en;
    }
    ID.iter().find(|(e, _)| *e == en).map_or(en, |(_, id)| id)
}

const ID: &[(&str, &str)] = &[
    // Tray
    ("Capture Area", "Tangkap Area"),
    ("Capture Window", "Tangkap Jendela"),
    ("Capture Screen", "Tangkap Layar"),
    ("Copy Text (OCR)", "Salin Teks (OCR)"),
    ("Capture Last Area", "Tangkap Area Terakhir"),
    ("Scrolling Capture", "Tangkapan Gulir"),
    ("Capture After Delay", "Tangkap Setelah Jeda"),
    ("Area", "Area"),
    ("Screen", "Layar"),
    ("{name} in {secs} Seconds", "{name} dalam {secs} Detik"),
    ("Record Area", "Rekam Area"),
    ("Record Screen", "Rekam Layar"),
    ("Record GIF", "Rekam GIF"),
    ("Stop Recording", "Hentikan Rekaman"),
    ("Pin Clipboard Image", "Sematkan Gambar dari Clipboard"),
    ("Make Pins Clickable Again", "Buat Pin Bisa Diklik Lagi"),
    ("History…", "Riwayat…"),
    ("Settings…", "Pengaturan…"),
    ("Check for Updates…", "Periksa Pembaruan…"),
    ("Quit KlikSnap", "Keluar dari KlikSnap"),
    // Pin menu
    ("Copy", "Salin"),
    ("Save", "Simpan"),
    ("Annotate", "Anotasi"),
    ("Opacity", "Transparansi"),
    ("Click Through", "Tembus Klik"),
    ("Close", "Tutup"),
    // Notices
    ("Text copied", "Teks disalin"),
    ("QR code copied", "Kode QR disalin"),
    ("No text found", "Tidak ada teks"),
    ("Text recognition failed", "Pengenalan teks gagal"),
    ("Color copied", "Warna disalin"),
    ("Recording failed", "Rekaman gagal"),
    ("Recording saved", "Rekaman disimpan"),
    ("GIF saved", "GIF disimpan"),
    ("Scrolling capture failed", "Tangkapan gulir gagal"),
    ("No image on the clipboard", "Tidak ada gambar di clipboard"),
    ("Recording without the microphone", "Merekam tanpa mikrofon"),
    (
        "Allow KlikSnap in System Settings → Privacy & Security → Microphone.",
        "Izinkan KlikSnap di Pengaturan Sistem → Privasi & Keamanan → Mikrofon.",
    ),
    ("Recording without sound", "Merekam tanpa suara"),
    (
        "The sound couldn't be captured; the video is still recorded.",
        "Suara tidak bisa direkam; videonya tetap direkam.",
    ),
    ("Screen Recording isn't allowed", "Perekaman Layar tidak diizinkan"),
    (
        "Turn KlikSnap on in Privacy & Security → Screen Recording, or remove it (−) and add it again.",
        "Nyalakan KlikSnap di Privasi & Keamanan → Perekaman Layar, atau hapus (−) lalu tambahkan lagi.",
    ),
    // Save dialog
    ("PNG image", "Gambar PNG"),
    ("JPEG image", "Gambar JPEG"),
    // Updates
    ("Update available", "Pembaruan tersedia"),
    ("Install and Restart", "Pasang dan Mulai Ulang"),
    ("Later", "Nanti"),
    (
        "KlikSnap {new} is available. You have {old}.",
        "KlikSnap {new} sudah tersedia. Versi Anda {old}.",
    ),
    (
        "The update could not be installed.",
        "Pembaruan tidak bisa dipasang.",
    ),
    (
        "You're up to date. KlikSnap {version} is the latest version.",
        "KlikSnap Anda sudah terbaru (versi {version}).",
    ),
    (
        "Couldn't check for updates.",
        "Tidak bisa memeriksa pembaruan.",
    ),
    (
        "You're up to date: {version} is the latest version.",
        "Sudah terbaru: {version} adalah versi terakhir.",
    ),
    ("Already checking…", "Sedang memeriksa…"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_indonesian_locales() {
        assert!(is_indonesian("id-ID"));
        assert!(is_indonesian("id_ID"));
        assert!(is_indonesian("in"));
        assert!(!is_indonesian("en-ID"));
        assert!(!is_indonesian("ms-MY"));
    }

    #[test]
    fn translations_are_unique_and_keep_placeholders() {
        for (i, (en, id)) in ID.iter().enumerate() {
            assert!(!ID[..i].iter().any(|(e, _)| e == en), "duplicate: {en}");
            for part in en.split('{').skip(1) {
                let name = part.split('}').next().unwrap();
                assert!(id.contains(&format!("{{{name}}}")), "{id} lacks {{{name}}}");
            }
        }
    }
}
