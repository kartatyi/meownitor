// The few words Rust shows itself — the tray menu and a round window's title — in the language the
// card was set to (config.json `lang`, written by the web side; English unless it says "uk").
// Everything else is worded by the web side (src/i18n.js).

#[derive(Clone, Copy, PartialEq)]
pub enum Lang {
    En,
    Uk,
}

pub fn lang() -> Lang {
    match crate::config::read().get("lang").and_then(|v| v.as_str()) {
        Some("uk") => Lang::Uk,
        _ => Lang::En,
    }
}

impl Lang {
    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Uk => "uk",
        }
    }

    pub fn tr(self, key: &str) -> &'static str {
        let (en, uk) = match key {
            "open" => ("Open the card", "Відкрити картку"),
            "settings" => ("Settings", "Налаштування"),
            "hide" => ("Hide for an hour", "Сховати на годину"),
            "unhide" => ("Show the widget", "Показати віджет"),
            "quiet" => ("Do not disturb for an hour", "Не турбувати годину"),
            "data" => ("Data and logs folder", "Тека з даними й логами"),
            "mood" => ("Mood", "Настрій"),
            "mood:auto" => ("Auto — follows the sessions", "Авто — за сесіями"),
            "mood:idle" => ("Resting", "Відпочиває"),
            "mood:work" => ("Working", "Працює"),
            "mood:ask" => ("Asking you", "Питає тебе"),
            "mood:done" => ("Done", "Готово"),
            "mood:sleep" => ("Sleeping", "Спить"),
            "mood:tired" => ("Near the limit", "Ліміт під межею"),
            "character" => ("Character", "Персонаж"),
            "kind:cat" => ("Cat", "Котик"),
            "kind:blob" => ("Blob", "Краплинка"),
            "kind:ghost" => ("Ghost", "Привидок"),
            "quit" => ("Quit", "Вийти"),
            "round" => ("Question from «{title}»", "Питання від «{title}»"),
            _ => ("", ""),
        };
        match self {
            Lang::En => en,
            Lang::Uk => uk,
        }
    }
}
