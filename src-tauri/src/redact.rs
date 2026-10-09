//! Finds sensitive text in recognized words: email addresses, IP addresses,
//! phone and card numbers, API keys and tokens, and values labelled as
//! passwords or secrets. Errs on the side of hiding too much.

use crate::ocr::Word;

/// Indexes of the words to hide. Words are in reading order; `line` groups them.
pub fn find(words: &[Word]) -> Vec<usize> {
    let mut hide = vec![false; words.len()];
    for (i, w) in words.iter().enumerate() {
        let t = trim(&w.text);
        if is_email(t) || is_ipv4(t) || is_secret(t) || is_labelled_value(t) {
            hide[i] = true;
        }
        // "password: hunter2", "token = abc": the word after the label.
        if is_label(t) || (is_label(t.trim_end_matches([':', '='])) && t.ends_with([':', '='])) {
            let mut j = i + 1;
            // Skip a lone ":" or "=" between the label and the value.
            while j < words.len()
                && words[j].line == w.line
                && matches!(trim(&words[j].text), ":" | "=" | "")
            {
                j += 1;
            }
            if j < words.len() && words[j].line == w.line {
                hide[j] = true;
            }
        }
    }
    // Runs of digit groups on one line, such as "+62 812-3456-7890" or a card number.
    let mut i = 0;
    while i < words.len() {
        let mut j = i;
        let mut digits = 0;
        while j < words.len() && words[j].line == words[i].line && is_number_part(&words[j].text) {
            digits += words[j].text.chars().filter(char::is_ascii_digit).count();
            j += 1;
        }
        if digits >= 9 {
            hide[i..j].iter_mut().for_each(|h| *h = true);
        }
        i = j.max(i + 1);
    }
    hide.iter()
        .enumerate()
        .filter_map(|(i, &h)| h.then_some(i))
        .collect()
}

/// The word without the punctuation around it.
fn trim(s: &str) -> &str {
    s.trim_matches(|c: char| {
        matches!(
            c,
            ',' | ';' | '(' | ')' | '[' | ']' | '{' | '}' | '"' | '\'' | '<' | '>' | '`'
        )
    })
    .trim_end_matches('.')
}

fn is_email(t: &str) -> bool {
    let Some((user, domain)) = t.split_once('@') else {
        return false;
    };
    !user.is_empty()
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && domain
            .chars()
            .all(|c| c.is_alphanumeric() || c == '.' || c == '-')
}

fn is_ipv4(t: &str) -> bool {
    // An optional port or CIDR suffix.
    let t = t.split([':', '/']).next().unwrap_or(t);
    let parts: Vec<&str> = t.split('.').collect();
    parts.len() == 4
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.len() <= 3 && p.parse::<u16>().is_ok_and(|n| n <= 255))
}

const KEY_PREFIXES: &[&str] = &[
    "sk-",
    "sk_live_",
    "pk_live_",
    "rk_live_",
    "ghp_",
    "gho_",
    "ghu_",
    "ghs_",
    "github_pat_",
    "glpat-",
    "xoxb-",
    "xoxp-",
    "xoxa-",
    "AKIA",
    "ASIA",
    "AIza",
    "ya29.",
    "eyJ",
    "npm_",
    "hf_",
];

/// API keys, tokens and other long random-looking strings.
fn is_secret(t: &str) -> bool {
    if KEY_PREFIXES.iter().any(|p| t.starts_with(p)) && t.len() >= 16 {
        return true;
    }
    if t.len() < 24 || t.contains("://") {
        return false;
    }
    let allowed = t
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '+' | '/' | '=' | '.'));
    let digits = t.chars().filter(char::is_ascii_digit).count();
    let upper = t.chars().filter(char::is_ascii_uppercase).count();
    let lower = t.chars().filter(char::is_ascii_lowercase).count();
    // Random strings mix cases and digits; long words and paths don't.
    allowed && digits >= 3 && ((upper >= 2 && lower >= 2) || digits * 3 >= t.len())
}

const LABELS: &[&str] = &[
    "password", "passwd", "pwd", "pass", "passcode", "pin", "secret", "token", "apikey", "api_key",
    "api-key", "key", "otp", "sandi",
];

fn is_label(t: &str) -> bool {
    let t = t.to_lowercase();
    LABELS.contains(&t.as_str())
}

/// `password=hunter2` or `API_KEY:abc` in a single word.
fn is_labelled_value(t: &str) -> bool {
    let Some((name, value)) = t.split_once(['=', ':']) else {
        return false;
    };
    let name = name.to_lowercase();
    !value.is_empty()
        && !value.starts_with("//")
        && ["pass", "secret", "token", "key", "pwd"]
            .iter()
            .any(|k| name.contains(k))
}

/// A word that can be part of a phone or card number: digits and separators only.
fn is_number_part(s: &str) -> bool {
    let t = s.trim_end_matches([',', '.']);
    !t.is_empty()
        && t.chars()
            .all(|c| c.is_ascii_digit() || matches!(c, '+' | '-' | '(' | ')' | '.'))
        && (t.chars().any(|c| c.is_ascii_digit()) || t == "-")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(lines: &[&str]) -> Vec<Word> {
        lines
            .iter()
            .enumerate()
            .flat_map(|(line, text)| {
                text.split_whitespace().map(move |w| Word {
                    text: w.into(),
                    rect: [0.0; 4],
                    line,
                })
            })
            .collect()
    }

    fn hidden(lines: &[&str]) -> Vec<String> {
        let w = words(lines);
        find(&w).into_iter().map(|i| w[i].text.clone()).collect()
    }

    #[test]
    fn finds_emails_and_ips() {
        assert_eq!(
            hidden(&["mail nihao@example.co.id now"]),
            ["nihao@example.co.id"]
        );
        assert_eq!(
            hidden(&["server 192.168.1.20:8080 up"]),
            ["192.168.1.20:8080"]
        );
        assert!(hidden(&["version 1.2.3 and 300.1.1.1"]).is_empty());
    }

    #[test]
    fn finds_phone_and_card_numbers() {
        assert_eq!(
            hidden(&["call +62 812-3456-7890 today"]),
            ["+62", "812-3456-7890"]
        );
        assert_eq!(hidden(&["4111 1111 1111 1111"]).len(), 4);
        assert!(hidden(&["on 2026-10-09 at 10:30"]).is_empty());
        assert!(hidden(&["total 1250"]).is_empty());
    }

    #[test]
    fn finds_keys_and_labelled_values() {
        assert_eq!(
            hidden(&["key sk-proj-abc123def456ghi789"]),
            ["sk-proj-abc123def456ghi789"]
        );
        assert_eq!(
            hidden(&["AWS_SECRET=wJalrXUtnFEMIK7MDENGbPxRfiCY"]).len(),
            1
        );
        assert_eq!(hidden(&["Password: hunter2"]), ["hunter2"]);
        assert_eq!(hidden(&["token = abc"]), ["abc"]);
        assert_eq!(hidden(&["Kata sandi"]).len(), 0);
    }

    #[test]
    fn leaves_ordinary_text_alone() {
        assert!(hidden(&["The quick brown fox jumps over the lazy dog"]).is_empty());
        assert!(hidden(&["https://github.com/ashafizullah/kliksnap/releases"]).is_empty());
        assert!(hidden(&["internationalization"]).is_empty());
    }
}

#[cfg(test)]
mod ocr_tests {
    #[test]
    #[ignore = "needs REDACT_IMAGE pointing at a PNG with sensitive text"]
    fn finds_sensitive_words_in_an_image() {
        let path = std::env::var("REDACT_IMAGE").unwrap();
        let img = xcap::image::open(path).unwrap().to_rgba8();
        let _ = crate::ocr::recognize(&xcap::image::RgbaImage::new(64, 32));
        let words = crate::ocr::words(&img).unwrap();
        let hidden: Vec<_> = super::find(&words).into_iter().map(|i| &words[i]).collect();
        for w in &hidden {
            println!("{} {:?}", w.text, w.rect);
        }
        assert!(!hidden.is_empty());
    }
}
