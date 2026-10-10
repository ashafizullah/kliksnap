//! An OpenAI-compatible chat model, set up by the user with their own key.

use std::time::Duration;

use base64::Engine;
use image::codecs::png::PngEncoder;
use image::{imageops, ExtendedColorType, ImageEncoder, RgbaImage};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::i18n::tr;

const TIMEOUT: Duration = Duration::from_secs(60);

/// A 16×16 red PNG: the test asks for its color, so a model that can't see
/// images fails it.
/// Longer sides are scaled down to this: models see no more detail than
/// about this anyway, and it keeps uploads small.
const MAX_SIDE: u32 = 2048;

const TEST_IMAGE: &str = "iVBORw0KGgoAAAANSUhEUgAAABAAAAAQCAIAAACQkWg2AAAAFklEQVR42mN4oKBAEmIY1TCqYfhqAADc5yAQocSb7AAAAABJRU5ErkJggg==";

/// One model the user set up; Settings keeps several and uses one at a time.
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Profile {
    pub name: String,
    /// An OpenAI-compatible endpoint, such as https://api.openai.com/v1.
    pub base_url: String,
    pub api_key: String,
    /// Must read images: the AI explains screenshots.
    pub model: String,
}

impl Profile {
    pub fn openai() -> Self {
        Self {
            name: "OpenAI".into(),
            base_url: "https://api.openai.com/v1".into(),
            ..Default::default()
        }
    }

    pub fn is_set_up(&self) -> bool {
        !self.base_url.trim().is_empty() && !self.model.trim().is_empty()
    }

    fn endpoint(&self) -> String {
        format!(
            "{}/chat/completions",
            self.base_url.trim().trim_end_matches('/')
        )
    }

    /// Asks `prompt` about a PNG (base64) and returns the reply's text.
    pub async fn ask_about_image(&self, prompt: &str, png_base64: &str) -> Result<String, String> {
        if !self.is_set_up() {
            return Err(tr("Fill in the base URL and the model first.").into());
        }
        let body = json!({
            "model": self.model.trim(),
            "messages": [{
                "role": "user",
                "content": [
                    { "type": "text", "text": prompt },
                    { "type": "image_url", "image_url": { "url": format!("data:image/png;base64,{png_base64}") } },
                ],
            }],
        });
        // reqwest is built without a TLS crypto provider, as for the updater.
        if rustls::crypto::CryptoProvider::get_default().is_none() {
            let _ = rustls::crypto::ring::default_provider().install_default();
        }
        let mut req = reqwest::Client::new()
            .post(self.endpoint())
            .timeout(TIMEOUT)
            .json(&body);
        // Local servers such as Ollama need no key.
        if !self.api_key.trim().is_empty() {
            req = req.bearer_auth(self.api_key.trim());
        }
        let res = req.send().await.map_err(describe)?;
        let status = res.status();
        let reply: Value = res
            .json()
            .await
            .map_err(|_| format!("{} ({status})", tr("Not an OpenAI-compatible reply")))?;
        if !status.is_success() {
            let msg = reply["error"]["message"].as_str().unwrap_or("");
            return Err(format!("{status} {msg}").trim().to_string());
        }
        reply["choices"][0]["message"]["content"]
            .as_str()
            .map(|s| s.trim().to_string())
            .ok_or_else(|| tr("Not an OpenAI-compatible reply").into())
    }

    /// Checks the model answers and can read an image; returns its answer.
    pub async fn test(&self) -> Result<String, String> {
        self.ask_about_image(
            "What color is this image? Answer with one word in English.",
            TEST_IMAGE,
        )
        .await
    }
}

/// A screenshot as base64 PNG, scaled down to `MAX_SIDE`.
pub fn encode(img: &RgbaImage) -> Result<String, String> {
    let (w, h) = img.dimensions();
    let k = MAX_SIDE as f64 / w.max(h) as f64;
    let scaled;
    let img = if k < 1.0 {
        let (sw, sh) = ((w as f64 * k).round() as u32, (h as f64 * k).round() as u32);
        scaled = imageops::resize(img, sw.max(1), sh.max(1), imageops::FilterType::Triangle);
        &scaled
    } else {
        img
    };
    let mut png = Vec::new();
    PngEncoder::new(&mut png)
        .write_image(
            img.as_raw(),
            img.width(),
            img.height(),
            ExtendedColorType::Rgba8,
        )
        .map_err(|e| e.to_string())?;
    Ok(base64::engine::general_purpose::STANDARD.encode(png))
}

/// What the model is asked about a screenshot, answering in `lang` ("en"/"id").
pub fn explain_prompt(lang: &str) -> String {
    let language = if lang == "id" {
        "Indonesian"
    } else {
        "English"
    };
    format!(
        "Explain this screenshot to the person who took it. Say briefly what it shows, \
         then what matters: if it has an error, what it means and how to fix it; if it \
         has code, a chart, a document or an interface, what it says or does. Be concise \
         and use short Markdown (lists, `code`). Answer in {language}."
    )
}

/// The innermost cause, e.g. "Connection refused" rather than reqwest's
/// "error sending request"; with a timeout said plainly.
fn describe(e: reqwest::Error) -> String {
    if e.is_timeout() {
        return "timed out".into();
    }
    let mut cause: &dyn std::error::Error = &e;
    while let Some(next) = cause.source() {
        cause = next;
    }
    cause.to_string()
}

/// Whether a test answer shows the model saw the red image.
pub fn saw_red(answer: &str) -> bool {
    answer.to_lowercase().contains("red")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_tolerates_a_trailing_slash() {
        let m = Profile {
            base_url: " https://api.openai.com/v1/ ".into(),
            ..Default::default()
        };
        assert_eq!(m.endpoint(), "https://api.openai.com/v1/chat/completions");
    }

    #[test]
    fn encode_scales_down_large_shots() {
        use base64::Engine;
        let png = encode(&RgbaImage::new(4096, 1024)).unwrap();
        let png = base64::engine::general_purpose::STANDARD
            .decode(png)
            .unwrap();
        let img = image::load_from_memory(&png).unwrap();
        assert_eq!((img.width(), img.height()), (2048, 512));
    }

    #[test]
    fn saw_red_ignores_case_and_punctuation() {
        assert!(saw_red("Red."));
        assert!(!saw_red("Blue"));
    }
}
