use crate::win32::GetUserDefaultUILanguage;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    Auto,
    ZhCn,
    EnUs,
}

impl Language {
    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "zh" | "zh-cn" | "zh_cn" | "chinese" | "zh-hans" => Language::ZhCn,
            "en" | "en-us" | "en_us" | "english" => Language::EnUs,
            _ => Language::Auto,
        }
    }

    #[allow(dead_code)]
    pub fn as_str(self) -> &'static str {
        match self {
            Language::Auto => "auto",
            Language::ZhCn => "zh-CN",
            Language::EnUs => "en-US",
        }
    }

    pub fn is_chinese(self) -> bool {
        match self {
            Language::ZhCn => true,
            Language::EnUs => false,
            Language::Auto => is_system_chinese(),
        }
    }
}

pub fn is_system_chinese() -> bool {
    unsafe {
        let lang = GetUserDefaultUILanguage();
        (lang & 0x03FF) == 0x04
    }
}
