//! Languages, with rules only (no tokens): which language an idea is written in, and whether a
//! piece of text is already in the reader's language. The progress feed is translated by
//! dictionary (narrate.rs) and foreign free text is replaced by a short phrase, never by an AI call.

/// Codes the app can narrate in (the order is the column order of the phrase tables in narrate.rs).
pub const SUPPORTED: [&str; 16] = ["th", "en", "zh", "ja", "ko", "vi", "id", "es", "fr", "de", "pt", "ru", "ar", "hi", "it", "tr"];

/// What the owner gets until they choose.
pub const DEFAULT: &str = "en";

/// Main language the owner chose in settings (kept in sync like workspace's report language).
static UI: parking_lot::Mutex<String> = parking_lot::const_mutex(String::new());

pub fn set_ui(code: &str) {
    *UI.lock() = normalize(code);
}

pub fn ui() -> String {
    let c = UI.lock().clone();
    if c.is_empty() { DEFAULT.into() } else { c }
}

pub fn normalize(code: &str) -> String {
    let c = code.trim().to_lowercase();
    SUPPORTED.iter().find(|s| **s == c).map(|s| s.to_string()).unwrap_or_else(|| DEFAULT.into())
}

/// English name, used inside prompts to the AI ("write in Thai").
pub fn name(code: &str) -> &'static str {
    match code {
        "en" => "English",
        "zh" => "Chinese (Simplified)",
        "ja" => "Japanese",
        "ko" => "Korean",
        "vi" => "Vietnamese",
        "id" => "Indonesian",
        "es" => "Spanish",
        "fr" => "French",
        "de" => "German",
        "pt" => "Portuguese",
        "ru" => "Russian",
        "ar" => "Arabic",
        "hi" => "Hindi",
        "it" => "Italian",
        "tr" => "Turkish",
        _ => "Thai",
    }
}

#[derive(Default)]
struct Count {
    letters: usize,
    thai: usize,
    hangul: usize,
    kana: usize,
    han: usize,
    viet: usize,
    cyrillic: usize,
    arabic: usize,
    devanagari: usize,
    turkish: usize,
}

fn count(text: &str) -> Count {
    let mut c = Count::default();
    for ch in text.chars().filter(|c| c.is_alphabetic()) {
        c.letters += 1;
        match ch as u32 {
            0x0E00..=0x0E7F => c.thai += 1,
            0xAC00..=0xD7AF | 0x1100..=0x11FF | 0x3130..=0x318F => c.hangul += 1,
            0x3040..=0x30FF => c.kana += 1,
            0x4E00..=0x9FFF => c.han += 1,
            0x0400..=0x04FF => c.cyrillic += 1,
            0x0600..=0x06FF | 0x0750..=0x077F => c.arabic += 1,
            0x0900..=0x097F => c.devanagari += 1,
            0x011E | 0x011F | 0x0130 | 0x0131 | 0x015E | 0x015F => c.turkish += 1,
            _ => {
                if "ăâđêôơưạảấầẩẫậắằẳẵặẹẻẽếềểễệỉịọỏốồổỗộớờởỡợụủứừửữựỳỵỷỹ".contains(ch.to_lowercase().next().unwrap_or(ch)) {
                    c.viet += 1;
                }
            }
        }
    }
    c
}

const EN: &[&str] = &["the", "and", "for", "with", "that", "this", "is", "are", "to", "of", "in", "a", "an", "it", "on", "my", "i", "we", "can", "how", "make", "app", "use"];
const ES: &[&str] = &["el", "la", "los", "las", "de", "que", "y", "en", "un", "una", "para", "con", "por", "es", "como", "quiero", "mi", "se", "del", "al", "más"];
const ID: &[&str] = &["yang", "dan", "untuk", "dengan", "ini", "itu", "di", "ke", "dari", "saya", "aku", "bisa", "tidak", "ada", "aplikasi", "buat", "membuat", "agar", "atau"];

const FR: &[&str] = &["le", "la", "les", "des", "une", "un", "et", "pour", "avec", "est", "dans", "que", "je", "veux", "mon", "pas", "sur", "du", "au"];
const DE: &[&str] = &["der", "die", "das", "und", "ist", "ein", "eine", "für", "mit", "ich", "nicht", "auf", "zu", "den", "von", "möchte", "will", "mein"];
const PT: &[&str] = &["o", "os", "as", "uma", "um", "para", "com", "não", "que", "de", "em", "quero", "meu", "minha", "um", "é", "se", "por", "você"];
const IT: &[&str] = &["il", "lo", "gli", "le", "una", "un", "per", "con", "che", "non", "di", "sono", "voglio", "mio", "della", "del", "e", "è"];
const TR: &[&str] = &["bir", "ve", "için", "ile", "bu", "ne", "da", "de", "ben", "istiyorum", "bunu", "gibi", "çok"];

fn score(words: &[&str], list: &[&str]) -> usize {
    words.iter().filter(|w| list.contains(w)).count()
}

/// The language `text` is written in, when that is clear; None for short or ambiguous text.
pub fn detect(text: &str) -> Option<&'static str> {
    let c = count(text);
    if c.letters < 6 {
        return None;
    }
    let share = |n: usize| n as f64 / c.letters as f64;
    if c.kana > 0 && share(c.kana + c.han) > 0.25 {
        return Some("ja");
    }
    if share(c.hangul) > 0.25 {
        return Some("ko");
    }
    if share(c.thai) > 0.25 {
        return Some("th");
    }
    if share(c.han) > 0.25 {
        return Some("zh");
    }
    if share(c.cyrillic) > 0.25 {
        return Some("ru");
    }
    if share(c.arabic) > 0.25 {
        return Some("ar");
    }
    if share(c.devanagari) > 0.25 {
        return Some("hi");
    }
    if c.turkish >= 1 {
        return Some("tr");
    }
    if c.viet >= 2 {
        return Some("vi");
    }
    let lower = text.to_lowercase();
    let words: Vec<&str> = lower.split(|ch: char| !ch.is_alphabetic()).filter(|w| !w.is_empty()).collect();
    let langs: [(&'static str, usize); 9] = [
        ("en", score(&words, EN)),
        ("es", score(&words, ES)),
        ("id", score(&words, ID)),
        ("fr", score(&words, FR)),
        ("de", score(&words, DE)),
        ("pt", score(&words, PT)),
        ("it", score(&words, IT)),
        ("tr", score(&words, TR)),
        ("", 0),
    ];
    let best = langs.iter().map(|l| l.1).max().unwrap_or(0);
    if best < 2 {
        return None;
    }
    // a tie is ambiguous, so the setting decides
    if langs.iter().filter(|l| l.1 == best).count() > 1 {
        return None;
    }
    langs.iter().find(|l| l.1 == best).map(|l| l.0)
}

/// Language to answer in: what the owner typed when that is clear (and following is on),
/// otherwise the main language from settings.
pub fn resolve(setting: &str, follow_input: bool, text: &str) -> String {
    let main = normalize(setting);
    if follow_input {
        if let Some(d) = detect(text) {
            return d.to_string();
        }
    }
    main
}

/// Is `text` already in `lang` (or too unclear to tell)? Used to decide whether the AI's own
/// words can be shown as they are, or must be replaced by a short phrase in the reader's language.
pub fn is_in(text: &str, lang: &str) -> bool {
    match detect(text) {
        Some(d) => d == lang,
        None => {
            // short or unclear text: for a non-Latin reader, Latin-only text is foreign
            let c = count(text);
            let other_script = c.cyrillic + c.arabic + c.devanagari;
            let latin_reader = matches!(lang, "en" | "vi" | "id" | "es" | "fr" | "de" | "pt" | "it" | "tr");
            latin_reader || c.letters < 6 || c.thai + c.hangul + c.kana + c.han + other_script > 0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_scripts() {
        assert_eq!(detect("อยากทำแอปจดค่าใช้จ่ายสำหรับครอบครัว"), Some("th"));
        assert_eq!(detect("I want to make an app for tracking coffee"), Some("en"));
        assert_eq!(detect("家族の支出を記録するアプリを作りたい"), Some("ja"));
        assert_eq!(detect("我想做一个记录家庭开支的应用"), Some("zh"));
        assert_eq!(detect("가족 지출을 기록하는 앱을 만들고 싶어요"), Some("ko"));
        assert_eq!(detect("tôi muốn làm một ứng dụng ghi chi tiêu"), Some("vi"));
        assert_eq!(detect("saya ingin membuat aplikasi untuk mencatat uang"), Some("id"));
        assert_eq!(detect("quiero hacer una app para el control de gastos"), Some("es"));
        assert_eq!(detect("je veux faire une application pour suivre mes dépenses"), Some("fr"));
        assert_eq!(detect("ich möchte eine App für meine Ausgaben bauen"), Some("de"));
        assert_eq!(detect("eu quero fazer um aplicativo para controlar os gastos"), Some("pt"));
        assert_eq!(detect("я хочу сделать приложение для учёта расходов"), Some("ru"));
        assert_eq!(detect("أريد إنشاء تطبيق لتتبع المصروفات"), Some("ar"));
        assert_eq!(detect("मैं खर्चों को ट्रैक करने के लिए एक ऐप बनाना चाहता हूँ"), Some("hi"));
        assert_eq!(detect("voglio fare un'app per tenere traccia delle spese"), Some("it"));
        assert_eq!(detect("harcamalarımı takip etmek için bir uygulama yapmak istiyorum"), Some("tr"));
        assert_eq!(detect("ok"), None);
    }

    #[test]
    fn follows_input_or_setting() {
        assert_eq!(resolve("th", true, "I want to make an app for tracking coffee"), "en");
        assert_eq!(resolve("th", false, "I want to make an app for tracking coffee"), "th");
        assert_eq!(resolve("en", true, "ok"), "en");
    }

    #[test]
    fn foreign_text_is_flagged() {
        assert!(!is_in("The user wants a coffee tracker, let me search for competitors", "th"));
        assert!(is_in("กำลังหาคู่แข่งของแอปนี้อยู่", "th"));
        assert!(is_in("Looking for competitors of this app", "en"));
    }
}
