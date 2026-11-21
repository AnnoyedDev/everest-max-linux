use std::collections::HashMap;
use std::fmt::Display;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::OnceLock;

pub const LANGS: [(&str, &str); 3] = [("fr", "Français"), ("en", "English"), ("ja", "日本語")];

static LANG: AtomicU8 = AtomicU8::new(0);

fn table() -> &'static HashMap<&'static str, (&'static str, &'static str)> {
    static T: OnceLock<HashMap<&'static str, (&'static str, &'static str)>> = OnceLock::new();
    T.get_or_init(|| {
        include_str!("../data/i18n.tsv").lines()
            .filter(|l| !l.trim().is_empty())
            .filter_map(|l| {
                let mut it = l.split('\t');
                Some((it.next()?, (it.next()?, it.next()?)))
            })
            .collect()
    })
}

pub fn detect() -> &'static str {
    let env = std::env::var("LC_ALL").or_else(|_| std::env::var("LC_MESSAGES")).or_else(|_| std::env::var("LANG")).unwrap_or_default();
    if env.starts_with("fr") { "fr" } else if env.starts_with("ja") { "ja" } else { "en" }
}

pub fn set(code: &str) {
    let code = if code.is_empty() { detect() } else { code };
    LANG.store(LANGS.iter().position(|l| l.0 == code).unwrap_or(0) as u8, Ordering::Relaxed);
}

pub fn current() -> &'static str {
    LANGS[LANG.load(Ordering::Relaxed) as usize].0
}

pub fn t(s: &'static str) -> &'static str {
    match LANG.load(Ordering::Relaxed) {
        0 => s,
        n => table().get(s).map(|(en, ja)| if n == 1 { *en } else { *ja }).filter(|v| !v.is_empty()).unwrap_or(s),
    }
}

pub fn fill(template: &str, args: &[&dyn Display]) -> String {
    let mut out = String::with_capacity(template.len() + 16);
    let mut rest = template;
    let mut i = 0;
    while let Some(p) = rest.find("{}") {
        out.push_str(&rest[..p]);
        if let Some(a) = args.get(i) {
            out.push_str(&a.to_string());
        }
        i += 1;
        rest = &rest[p + 2..];
    }
    out.push_str(rest);
    out
}

#[macro_export]
macro_rules! tf {
    ($s:literal $(, $a:expr)* $(,)?) => {
        $crate::i18n::fill($crate::i18n::t($s), &[$(&$a as &dyn std::fmt::Display),*])
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_key_has_both_translations_with_same_placeholders() {
        for (fr, (en, ja)) in table() {
            assert!(!en.is_empty() && !ja.is_empty(), "{fr}");
            let n = fr.matches("{}").count();
            assert_eq!(en.matches("{}").count(), n, "en: {fr}");
            assert_eq!(ja.matches("{}").count(), n, "ja: {fr}");
        }
    }

    #[test]
    fn every_source_key_is_translated() {
        let sources = [include_str!("gui.rs"), include_str!("setup.rs"), include_str!("device.rs"), include_str!("apply.rs"), include_str!("uinput.rs"), include_str!("images.rs"), include_str!("fx.rs"), include_str!("protocol.rs"), include_str!("icons.rs"), include_str!("pad.rs"), include_str!("obs.rs"), include_str!("actions.rs")];
        let re_keys = |src: &'static str| -> Vec<&'static str> {
            let mut keys = vec![];
            for marker in ["t(\"", "tf!(\""] {
                let mut from = 0;
                while let Some(p) = src[from..].find(marker) {
                    let at = from + p;
                    from = at + marker.len();
                    let prev = src[..at].chars().last().unwrap_or(' ');
                    if prev.is_alphanumeric() || prev == '_' || prev == '.' {
                        continue;
                    }
                    if let Some(end) = src[from..].find('"') {
                        keys.push(&src[from..from + end]);
                    }
                }
            }
            keys
        };
        for src in sources {
            for k in re_keys(src) {
                assert!(table().contains_key(k), "missing translation: {k}");
            }
        }
        let consts = crate::protocol::EFFECTS.iter().map(|e| e.0)
            .chain(crate::fx::SOFT.iter().map(|e| e.0))
            .chain(crate::icons::LIBRARY.iter().map(|e| e.1))
            .chain(crate::obs::REQUESTS.iter().map(|e| e.1));
        for k in consts {
            assert!(table().contains_key(k), "missing translation: {k}");
        }
    }

    #[test]
    fn fill_replaces_in_order() {
        assert_eq!(fill("a {} b {}", &[&1, &"x"]), "a 1 b x");
    }
}
