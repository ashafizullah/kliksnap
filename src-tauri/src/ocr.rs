//! Text recognition with the OCR engine built into the OS (Apple Vision on
//! macOS, Windows.Media.Ocr on Windows), so it adds nothing to the app size.

use xcap::image::RgbaImage;

pub fn recognize(img: &RgbaImage) -> Result<String, String> {
    imp::recognize(img)
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

    fn class(name: &CStr) -> Result<&'static AnyClass, String> {
        AnyClass::get(name).ok_or_else(|| format!("{name:?} is unavailable"))
    }

    pub fn recognize(img: &RgbaImage) -> Result<String, String> {
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
                autoreleasepool(|_| run_vision(cg_image))
            };
            for cf in [cg_image, provider, space] {
                if !cf.is_null() {
                    CFRelease(cf);
                }
            }
            result
        }
    }

    unsafe fn run_vision(cg_image: *mut c_void) -> Result<String, String> {
        let request: Retained<AnyObject> = msg_send![class(c"VNRecognizeTextRequest")?, new];
        let _: () = msg_send![&*request, setRecognitionLevel: REQUEST_ACCURATE];
        let _: () = msg_send![&*request, setUsesLanguageCorrection: true];
        let auto_language: Bool =
            msg_send![&*request, respondsToSelector: sel!(setAutomaticallyDetectsLanguage:)];
        if auto_language.as_bool() {
            let _: () = msg_send![&*request, setAutomaticallyDetectsLanguage: true];
        }

        let options: *mut AnyObject = msg_send![class(c"NSDictionary")?, dictionary];
        let handler: Allocated<AnyObject> = msg_send![class(c"VNImageRequestHandler")?, alloc];
        let handler: Retained<AnyObject> =
            msg_send![handler, initWithCGImage: cg_image, options: options];
        let requests: *mut AnyObject = msg_send![class(c"NSArray")?, arrayWithObject: &*request];
        let mut error: *mut AnyObject = std::ptr::null_mut();
        let ok: Bool = msg_send![&*handler, performRequests: requests, error: &mut error];
        if !ok.as_bool() {
            return Err("text recognition failed".into());
        }

        let observations: *mut AnyObject = msg_send![&*request, results];
        if observations.is_null() {
            return Ok(String::new());
        }
        let count: usize = msg_send![observations, count];
        let mut lines = Vec::with_capacity(count);
        for i in 0..count {
            let observation: *mut AnyObject = msg_send![observations, objectAtIndex: i];
            let candidates: *mut AnyObject = msg_send![observation, topCandidates: 1usize];
            let best: *mut AnyObject = msg_send![candidates, firstObject];
            if best.is_null() {
                continue;
            }
            let string: *mut AnyObject = msg_send![best, string];
            let utf8: *const std::ffi::c_char = msg_send![string, UTF8String];
            if !utf8.is_null() {
                lines.push(CStr::from_ptr(utf8).to_string_lossy().into_owned());
            }
        }
        Ok(lines.join("\n"))
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
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod imp {
    use xcap::image::RgbaImage;

    pub fn recognize(_img: &RgbaImage) -> Result<String, String> {
        Err("text recognition is not supported on this platform".into())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "needs OCR_IMAGE pointing at a PNG with text"]
    fn recognizes_text() {
        let path = std::env::var("OCR_IMAGE").unwrap();
        let img = xcap::image::open(path).unwrap().to_rgba8();
        let text = super::recognize(&img).unwrap();
        println!("{text}");
        assert!(!text.trim().is_empty());
    }
}
