//! Text and QR code recognition. Text uses the OCR engine built into the OS
//! (Apple Vision on macOS, Windows.Media.Ocr on Windows), so it adds nothing to
//! the app size. QR codes use Vision on macOS and `rqrr` on Windows, which has
//! no built-in decoder.

use xcap::image::RgbaImage;

pub fn recognize(img: &RgbaImage) -> Result<String, String> {
    imp::recognize(img)
}

/// The payloads of the QR codes (and on macOS, other barcodes) in the image.
pub fn scan_codes(img: &RgbaImage) -> Result<Vec<String>, String> {
    imp::scan_codes(img)
}

#[cfg(target_os = "macos")]
mod imp {
    use std::ffi::{c_void, CStr};

    use objc2::rc::{autoreleasepool, Allocated, Retained};
    use objc2::runtime::{AnyClass, AnyObject, Bool};
    use objc2::{msg_send, sel};
    use xcap::image::RgbaImage;

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGColorSpaceCreateDeviceRGB() -> *mut c_void;
        fn CGDataProviderCreateWithData(
            info: *mut c_void,
            data: *const c_void,
            size: usize,
            release: *const c_void,
        ) -> *mut c_void;
        fn CGImageCreate(
            width: usize,
            height: usize,
            bits_per_component: usize,
            bits_per_pixel: usize,
            bytes_per_row: usize,
            space: *mut c_void,
            bitmap_info: u32,
            provider: *mut c_void,
            decode: *const f64,
            should_interpolate: bool,
            intent: i32,
        ) -> *mut c_void;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFRelease(cf: *const c_void);
    }

    #[link(name = "Vision", kind = "framework")]
    extern "C" {}

    const ALPHA_PREMULTIPLIED_LAST: u32 = 1;
    const REQUEST_ACCURATE: isize = 0;
    /// On macOS 27 the default revision 3 accurate recognizer works only for the
    /// first request in a process (later ones, at any revision, fail with
    /// CRImageReaderError 1), and the app's warm-up is always that first one.
    const REVISION: usize = 2;

    fn class(name: &CStr) -> Result<&'static AnyClass, String> {
        AnyClass::get(name).ok_or_else(|| format!("{name:?} is unavailable"))
    }

    pub fn recognize(img: &RgbaImage) -> Result<String, String> {
        with_cg_image(img, |cg_image| unsafe { run_vision(cg_image) })
    }

    pub fn scan_codes(img: &RgbaImage) -> Result<Vec<String>, String> {
        with_cg_image(img, |cg_image| unsafe { run_barcodes(cg_image) })
    }

    fn with_cg_image<T>(
        img: &RgbaImage,
        f: impl FnOnce(*mut c_void) -> Result<T, String>,
    ) -> Result<T, String> {
        let (w, h) = (img.width() as usize, img.height() as usize);
        let raw = img.as_raw();
        unsafe {
            // `raw` outlives the CGImage: both are released before this returns.
            let space = CGColorSpaceCreateDeviceRGB();
            let provider = CGDataProviderCreateWithData(
                std::ptr::null_mut(),
                raw.as_ptr().cast(),
                raw.len(),
                std::ptr::null(),
            );
            let cg_image = CGImageCreate(
                w,
                h,
                8,
                32,
                w * 4,
                space,
                ALPHA_PREMULTIPLIED_LAST,
                provider,
                std::ptr::null(),
                false,
                0,
            );
            let result = if cg_image.is_null() {
                Err("could not create image".into())
            } else {
                autoreleasepool(|_| f(cg_image))
            };
            for cf in [cg_image, provider, space] {
                if !cf.is_null() {
                    CFRelease(cf);
                }
            }
            result
        }
    }

    /// The message of an `NSError`, or a generic one if there is none.
    unsafe fn describe(error: *mut AnyObject) -> String {
        if error.is_null() {
            return "text recognition failed".into();
        }
        let message: *mut AnyObject = msg_send![error, localizedDescription];
        to_string(message).unwrap_or_else(|| "text recognition failed".into())
    }

    unsafe fn run_vision(cg_image: *mut c_void) -> Result<String, String> {
        let request: Retained<AnyObject> = msg_send![class(c"VNRecognizeTextRequest")?, new];
        let _: () = msg_send![&*request, setRevision: REVISION];
        let _: () = msg_send![&*request, setRecognitionLevel: REQUEST_ACCURATE];
        let _: () = msg_send![&*request, setUsesLanguageCorrection: true];
        let auto_language: Bool =
            msg_send![&*request, respondsToSelector: sel!(setAutomaticallyDetectsLanguage:)];
        if auto_language.as_bool() {
            let _: () = msg_send![&*request, setAutomaticallyDetectsLanguage: true];
        }

        let mut lines = Vec::new();
        for observation in perform(cg_image, &request)? {
            let candidates: *mut AnyObject = msg_send![observation, topCandidates: 1usize];
            let best: *mut AnyObject = msg_send![candidates, firstObject];
            if best.is_null() {
                continue;
            }
            let string: *mut AnyObject = msg_send![best, string];
            lines.extend(to_string(string));
        }
        Ok(lines.join("\n"))
    }

    unsafe fn run_barcodes(cg_image: *mut c_void) -> Result<Vec<String>, String> {
        let request: Retained<AnyObject> = msg_send![class(c"VNDetectBarcodesRequest")?, new];
        let mut codes = Vec::new();
        for observation in perform(cg_image, &request)? {
            let payload: *mut AnyObject = msg_send![observation, payloadStringValue];
            codes.extend(to_string(payload));
        }
        Ok(codes)
    }

    /// Runs a Vision request on the image and returns its result observations.
    unsafe fn perform(
        cg_image: *mut c_void,
        request: &AnyObject,
    ) -> Result<Vec<*mut AnyObject>, String> {
        let options: *mut AnyObject = msg_send![class(c"NSDictionary")?, dictionary];
        let handler: Allocated<AnyObject> = msg_send![class(c"VNImageRequestHandler")?, alloc];
        let handler: Retained<AnyObject> =
            msg_send![handler, initWithCGImage: cg_image, options: options];
        let requests: *mut AnyObject = msg_send![class(c"NSArray")?, arrayWithObject: request];
        let mut error: *mut AnyObject = std::ptr::null_mut();
        let ok: Bool = msg_send![&*handler, performRequests: requests, error: &mut error];
        if !ok.as_bool() {
            return Err(describe(error));
        }
        let observations: *mut AnyObject = msg_send![request, results];
        if observations.is_null() {
            return Ok(Vec::new());
        }
        let count: usize = msg_send![observations, count];
        Ok((0..count)
            .map(|i| msg_send![observations, objectAtIndex: i])
            .collect())
    }

    /// Copies an `NSString`, which may be nil.
    unsafe fn to_string(string: *mut AnyObject) -> Option<String> {
        if string.is_null() {
            return None;
        }
        let utf8: *const std::ffi::c_char = msg_send![string, UTF8String];
        (!utf8.is_null()).then(|| CStr::from_ptr(utf8).to_string_lossy().into_owned())
    }
}

#[cfg(target_os = "windows")]
mod imp {
    use windows::Graphics::Imaging::{BitmapAlphaMode, BitmapPixelFormat, SoftwareBitmap};
    use windows::Media::Ocr::OcrEngine;
    use windows::Storage::Streams::DataWriter;
    use xcap::image::{imageops, RgbaImage};

    pub fn recognize(img: &RgbaImage) -> Result<String, String> {
        run(img).map_err(|e| e.message().to_string())
    }

    fn run(img: &RgbaImage) -> windows::core::Result<String> {
        // The engine rejects images larger than MaxImageDimension.
        let max = OcrEngine::MaxImageDimension()?;
        let scaled;
        let img = if img.width() > max || img.height() > max {
            let k = max as f64 / img.width().max(img.height()) as f64;
            let (w, h) = (
                (img.width() as f64 * k) as u32,
                (img.height() as f64 * k) as u32,
            );
            scaled = imageops::resize(img, w, h, imageops::FilterType::Triangle);
            &scaled
        } else {
            img
        };

        let bgra: Vec<u8> = img
            .as_raw()
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|&[r, g, b, _]| [b, g, r, 255])
            .collect();
        let writer = DataWriter::new()?;
        writer.WriteBytes(&bgra)?;
        let buffer = writer.DetachBuffer()?;
        let bitmap = SoftwareBitmap::CreateCopyWithAlphaFromBuffer(
            &buffer,
            BitmapPixelFormat::Bgra8,
            img.width() as i32,
            img.height() as i32,
            BitmapAlphaMode::Premultiplied,
        )?;
        let engine = OcrEngine::TryCreateFromUserProfileLanguages()?;
        let result = engine.RecognizeAsync(&bitmap)?.join()?;
        let mut lines = Vec::new();
        for line in result.Lines()? {
            lines.push(line.Text()?.to_string());
        }
        Ok(lines.join("\n"))
    }

    pub fn scan_codes(img: &RgbaImage) -> Result<Vec<String>, String> {
        let mut prepared = rqrr::PreparedImage::prepare_from_greyscale(
            img.width() as usize,
            img.height() as usize,
            |x, y| {
                let [r, g, b, _] = img.get_pixel(x as u32, y as u32).0;
                ((r as u32 * 299 + g as u32 * 587 + b as u32 * 114) / 1000) as u8
            },
        );
        Ok(prepared
            .detect_grids()
            .into_iter()
            .filter_map(|grid| grid.decode().ok())
            .map(|(_, content)| content)
            .collect())
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod imp {
    use xcap::image::RgbaImage;

    pub fn recognize(_img: &RgbaImage) -> Result<String, String> {
        Err("text recognition is not supported on this platform".into())
    }

    pub fn scan_codes(_img: &RgbaImage) -> Result<Vec<String>, String> {
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "needs OCR_IMAGE pointing at a PNG with text"]
    fn recognizes_text() {
        let path = std::env::var("OCR_IMAGE").unwrap();
        let img = xcap::image::open(path).unwrap().to_rgba8();
        // The app warms the engine up first, so the real request is never the first one.
        let _ = super::recognize(&xcap::image::RgbaImage::new(64, 32));
        for _ in 0..2 {
            let text = super::recognize(&img).unwrap();
            println!("{text}");
            assert!(!text.trim().is_empty());
        }
    }
}

#[cfg(test)]
mod qr_tests {
    #[test]
    #[ignore = "needs QR_IMAGE pointing at a PNG with a QR code"]
    fn scans_qr_code() {
        let path = std::env::var("QR_IMAGE").unwrap();
        let img = xcap::image::open(path).unwrap().to_rgba8();
        let codes = super::scan_codes(&img).unwrap();
        println!("{codes:?}");
        assert!(!codes.is_empty());
    }
}
