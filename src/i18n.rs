//! Runtime UI localization. Hardware names and native OS errors remain verbatim.
use serde::{Deserialize, Serialize};
use std::sync::{
    OnceLock,
    atomic::{AtomicU8, Ordering},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum Language {
    Russian,
    English,
    German,
    French,
    Spanish,
    Portuguese,
    Italian,
    Chinese,
    Japanese,
    Korean,
    Ukrainian,
}
impl Language {
    pub const ALL: [Self; 11] = [
        Self::Russian,
        Self::English,
        Self::German,
        Self::French,
        Self::Spanish,
        Self::Portuguese,
        Self::Italian,
        Self::Chinese,
        Self::Japanese,
        Self::Korean,
        Self::Ukrainian,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Russian => "Русский",
            Self::English => "English",
            Self::German => "Deutsch",
            Self::French => "Français",
            Self::Spanish => "Español",
            Self::Portuguese => "Português (Brasil)",
            Self::Italian => "Italiano",
            Self::Chinese => "简体中文",
            Self::Japanese => "日本語",
            Self::Korean => "한국어",
            Self::Ukrainian => "Українська",
        }
    }
    pub fn from_tag(tag: &str) -> Self {
        match tag
            .split(['-', '_', '.', '@'])
            .next()
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str()
        {
            "ru" => Self::Russian,
            "uk" => Self::Ukrainian,
            "de" => Self::German,
            "fr" => Self::French,
            "es" => Self::Spanish,
            "pt" => Self::Portuguese,
            "it" => Self::Italian,
            "zh" => Self::Chinese,
            "ja" => Self::Japanese,
            "ko" => Self::Korean,
            _ => Self::English,
        }
    }
    pub fn font_index(self) -> u32 {
        match self {
            Self::Japanese => 0,
            Self::Korean => 1,
            _ => 2,
        }
    }
    pub fn translate(self, input: &str) -> String {
        translate(self, input, 0)
    }
}
impl Default for Language {
    fn default() -> Self {
        sys_locale::get_locale()
            .as_deref()
            .map(Self::from_tag)
            .unwrap_or(Self::English)
    }
}
static CURRENT: AtomicU8 = AtomicU8::new(Language::English as u8);
pub fn set_language(language: Language) {
    CURRENT.store(language as u8, Ordering::Relaxed);
}
pub fn tr(input: impl AsRef<str>) -> String {
    Language::ALL[usize::from(CURRENT.load(Ordering::Relaxed))].translate(input.as_ref())
}
#[derive(Deserialize)]
struct Entry {
    source: String,
    text: [String; 10],
}
struct Message {
    entry: Entry,
    parts: Vec<String>,
    weight: usize,
}
fn catalog() -> &'static [Message] {
    static CATALOG: OnceLock<Vec<Message>> = OnceLock::new();
    CATALOG.get_or_init(|| {
        let entries: Vec<Entry> = serde_json::from_str(include_str!("../locales/messages.json"))
            .expect("valid embedded translations");
        let mut messages: Vec<_> = entries
            .into_iter()
            .map(|entry| {
                let parts = template_parts(&entry.source);
                let weight = parts.iter().map(String::len).sum();
                Message {
                    entry,
                    parts,
                    weight,
                }
            })
            .collect();
        // Prefer the most specific template, so nested error wrappers match first.
        messages.sort_by_key(|m| std::cmp::Reverse(m.weight));
        messages
    })
}
fn template_parts(source: &str) -> Vec<String> {
    let mut rest = source;
    let mut parts = Vec::new();
    while let Some(start) = rest.find('{') {
        let Some(end) = rest[start..].find('}') else {
            break;
        };
        parts.push(rest[..start].to_owned());
        rest = &rest[start + end + 1..];
    }
    parts.push(rest.to_owned());
    parts
}
fn capture<'a>(input: &'a str, parts: &[String]) -> Option<Vec<&'a str>> {
    let mut rest = input.strip_prefix(&parts[0])?;
    let mut values = Vec::new();
    for (i, separator) in parts.iter().enumerate().skip(1) {
        if i == parts.len() - 1 {
            values.push(rest.strip_suffix(separator)?);
        } else {
            let end = rest.find(separator)?;
            values.push(&rest[..end]);
            rest = &rest[end + separator.len()..];
        }
    }
    Some(values)
}
fn translate(language: Language, input: &str, depth: usize) -> String {
    if language == Language::Russian || depth > 4 {
        return input.to_owned();
    }
    let column = language as usize - 1;
    for message in catalog() {
        if message.entry.source == input {
            return message.entry.text[column].clone();
        }
    }
    for message in catalog().iter().filter(|m| m.parts.len() > 1) {
        if let Some(values) = capture(input, &message.parts) {
            let values: Vec<_> = values
                .into_iter()
                .map(|v| translate(language, v, depth + 1))
                .collect();
            return interpolate(&message.entry.text[column], &values);
        }
    }
    input.to_owned()
}
fn interpolate(template: &str, values: &[String]) -> String {
    let mut result = String::new();
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        result.push_str(&rest[..start]);
        let Some(end) = rest[start..].find('}') else {
            result.push_str(&rest[start..]);
            return result;
        };
        let key = &rest[start + 1..start + end];
        if let Some(value) = key.parse::<usize>().ok().and_then(|i| values.get(i)) {
            result.push_str(value);
        }
        rest = &rest[start + end + 1..];
    }
    result.push_str(rest);
    result
}

pub fn configure_fonts(ctx: &eframe::egui::Context, language: Language) {
    use eframe::egui::{FontData, FontDefinitions, FontFamily};
    let mut fonts = FontDefinitions::default();
    let mut data = FontData::from_static(include_bytes!("../assets/fonts/NotoSansCJK-Regular.ttc"));
    data.index = language.font_index();
    fonts.font_data.insert("noto-cjk".into(), data.into());
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        fonts
            .families
            .entry(family)
            .or_default()
            .push("noto-cjk".into());
    }
    ctx.set_fonts(fonts);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn locale_mapping_and_fallback() {
        for (tag, expected) in [
            ("ru_RU.UTF-8", Language::Russian),
            ("pt-BR", Language::Portuguese),
            ("zh-Hans-CN", Language::Chinese),
            ("ja_JP", Language::Japanese),
            ("ko-KR", Language::Korean),
            ("sv-SE", Language::English),
        ] {
            assert_eq!(Language::from_tag(tag), expected);
        }
    }
    #[test]
    fn all_translations_preserve_format_arguments() {
        let mut sources = std::collections::HashSet::new();
        for message in catalog() {
            assert!(sources.insert(&message.entry.source), "duplicate source");
            let count = message.parts.len() - 1;
            for text in &message.entry.text {
                assert!(
                    !text.trim().is_empty(),
                    "missing translation: {}",
                    message.entry.source
                );
                let mut found = Vec::new();
                let mut rest = text.as_str();
                while let Some(start) = rest.find('{') {
                    let end = rest[start..].find('}').expect("closed placeholder");
                    found.push(
                        rest[start + 1..start + end]
                            .parse::<usize>()
                            .expect("numeric placeholder"),
                    );
                    rest = &rest[start + end + 1..];
                }
                found.sort();
                assert_eq!(
                    found,
                    (0..count).collect::<Vec<_>>(),
                    "placeholder mismatch: {text}"
                );
            }
        }
    }
    #[test]
    fn formatted_messages_and_nested_errors_change_language() {
        assert_eq!(Language::English.translate("Уровень: 25%"), "Level: 25%");
        assert_eq!(
            Language::German.translate("Версия 0.4.0 готова к установке"),
            "Version 0.4.0 ist installationsbereit"
        );
        assert_eq!(
            Language::English
                .translate("Ошибка поиска: USB-поток недоступен. Перезапустите приложение."),
            "Search failed: USB service unavailable. Restart the app."
        );
        assert_eq!(Language::Japanese.translate("10 мин"), "10 分");
        assert_eq!(
            Language::English.translate("GTX JetSeat 231D:2002"),
            "GTX JetSeat 231D:2002"
        );
    }
}
