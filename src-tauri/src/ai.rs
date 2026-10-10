//! An OpenAI-compatible chat model, set up by the user with their own key.

use std::time::Duration;

use base64::Engine;
use image::codecs::png::PngEncoder;
use image::{imageops, ExtendedColorType, ImageEncoder, RgbaImage};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::i18n::tr;

/// Reasoning models can take a while over a large screenshot.
const TIMEOUT: Duration = Duration::from_secs(120);

/// Longer sides are scaled down to this: models see no more detail than
/// about this anyway, and it keeps uploads small.
const MAX_SIDE: u32 = 2048;

/// A 16×16 red PNG: the test asks for its color, so a model that can't see
/// images fails it.
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
        let turn = Turn {
            role: "user".into(),
            content: prompt.into(),
        };
        self.chat(None, png_base64, std::slice::from_ref(&turn))
            .await
    }

    /// Continues a conversation about a PNG (base64), which goes with the
    /// first message; returns the reply's text.
    pub async fn chat(
        &self,
        system: Option<&str>,
        png_base64: &str,
        turns: &[Turn],
    ) -> Result<String, String> {
        if !self.is_set_up() {
            return Err(tr("Fill in the base URL and the model first.").into());
        }
        let mut messages = Vec::new();
        if let Some(system) = system {
            messages.push(json!({ "role": "system", "content": system }));
        }
        for (i, turn) in turns.iter().enumerate() {
            // Only the webview's two roles: nothing can pose as the system.
            let role = if turn.role == "assistant" {
                "assistant"
            } else {
                "user"
            };
            messages.push(if i == 0 {
                json!({
                    "role": role,
                    "content": [
                        { "type": "text", "text": turn.content },
                        { "type": "image_url", "image_url": { "url": format!("data:image/png;base64,{png_base64}") } },
                    ],
                })
            } else {
                json!({ "role": role, "content": turn.content })
            });
        }
        let body = json!({ "model": self.model.trim(), "messages": messages });
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

    /// The text in an image, as plain text in reading order.
    pub async fn read_text(&self, png_base64: &str) -> Result<String, String> {
        let text = self
            .ask_about_image(
                "Transcribe all the text in this image exactly as written, in reading \
                 order, keeping line breaks. Output only the text: no commentary, no \
                 Markdown, no code fences. If there is no text, output nothing.",
                png_base64,
            )
            .await?;
        Ok(strip_fence(&text).to_string())
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

/// One message of a conversation about a screenshot.
#[derive(Clone, Deserialize)]
pub struct Turn {
    /// "user" or "assistant".
    pub role: String,
    pub content: String,
}

/// Something to ask about a screenshot, from a button in the AI window.
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Action {
    pub name: String,
    /// `{language}` becomes the app's language.
    pub prompt: String,
}

impl Action {
    fn new(name: &str, prompt: &str) -> Self {
        Self {
            name: name.into(),
            prompt: prompt.into(),
        }
    }

    /// The presets; the first runs from the Explain buttons.
    pub fn defaults() -> Vec<Self> {
        vec![
            Self::new(
                "Explain",
                "Explain this screenshot to me. Say briefly what it shows, then what \
                 matters: if it has an error, what it means and how to fix it; if it has \
                 code, a chart, a document or an interface, what it says or does.",
            ),
            Self::new(
                "Translate",
                "Translate all the text in this screenshot into {language}, in reading \
                 order. Output only the translation.",
            ),
            Self::new(
                "Summarize",
                "Summarize this screenshot in a few short bullet points.",
            ),
            Self::new(
                "Table to CSV",
                "Extract the table in this screenshot as CSV in one code block, with its \
                 headers. No commentary.",
            ),
        ]
    }
}

/// The app's language for prompts, from `code` ("en"/"id").
pub fn language(code: &str) -> &'static str {
    if code == "id" {
        "Indonesian"
    } else {
        "English"
    }
}

/// Sets the scene for a conversation about a screenshot.
pub fn system_prompt(language: &str) -> String {
    format!(
        "You help the user with a screenshot they just took, attached to their first \
         message. Answer in {language} unless they ask otherwise. Be concise and use \
         short Markdown (lists, `code`, code blocks)."
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

/// The inside of a ``` block, for models that wrap their answer in one anyway.
fn strip_fence(text: &str) -> &str {
    let t = text.trim();
    let Some(inner) = t.strip_prefix("```").and_then(|t| t.strip_suffix("```")) else {
        return t;
    };
    // Drop a language tag on the opening line.
    inner
        .split_once('\n')
        .map_or(inner, |(_, body)| body)
        .trim()
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
    fn strip_fence_unwraps_code_blocks() {
        assert_eq!(strip_fence("```text\nHello\nWorld\n```"), "Hello\nWorld");
        assert_eq!(strip_fence("  Plain\ntext "), "Plain\ntext");
    }

    #[test]
    fn saw_red_ignores_case_and_punctuation() {
        assert!(saw_red("Red."));
        assert!(!saw_red("Blue"));
    }
}
