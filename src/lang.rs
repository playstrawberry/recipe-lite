//! The script's texts: `lang/{en,fr,es,tr}.lang`, embedded and checked for the same keys while it compiles.
use sp_lang::{Catalog, Language};

static LANG: Catalog = sp_lang::catalog!("recipe-lite");

/// The text of `key` in the language of `locale` (English for any other), with `{0}`, `{1}` filled from `args`.
pub fn text(locale: &str, key: &str, args: &[&str]) -> String {
    let raw = LANG
        .get_or_en(Language::of_locale(locale), key)
        .unwrap_or_default();
    sp_lang::render(raw, args, "")
}

/// A model line in the language of `locale`.
pub fn line(locale: &str, (key, args): &crate::model::Line) -> String {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    text(locale, key, &args)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Kind;

    const KINDS: [Kind; 9] = [
        Kind::Shaped,
        Kind::Shapeless,
        Kind::Transmute,
        Kind::Smelting,
        Kind::Blasting,
        Kind::Smoking,
        Kind::Campfire,
        Kind::Stonecutting,
        Kind::Smithing,
    ];

    #[test]
    fn every_key_the_script_names_has_a_text_in_every_language() {
        let own = [
            "description",
            "permission_use",
            "command_recipe",
            "no_permission",
            "usage",
            "bad_item",
            "bad_number",
            "vanilla",
            "players_only",
            "empty_hand",
            "no_recipe",
            "no_such",
            "title",
            "head",
            "result",
            "grid_row",
            "legend",
            "inputs",
            "cooking",
            "next",
            "last",
            "tag_slot",
            "others",
        ];
        let kinds = KINDS.map(Kind::key);
        for key in own.iter().chain(kinds.iter()) {
            for language in Language::ALL {
                assert!(
                    LANG.get(language, key).is_some_and(|t| !t.is_empty()),
                    "{key} in {}",
                    language.code()
                );
            }
        }
        assert_eq!(LANG.len(), own.len() + kinds.len(), "a key nobody names");
    }

    #[test]
    fn the_next_recipe_is_named_as_a_command_in_every_language() {
        for locale in ["en_us", "fr_fr", "es_es", "tr_tr"] {
            let foot = text(locale, "next", &["1", "3", "stick", "2"]);
            assert!(foot.contains("/recipe stick 2"), "{foot}");
        }
    }
}
