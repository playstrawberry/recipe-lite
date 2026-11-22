//! What `/recipe` reads from what was typed, and where a recipe's items go in the window that shows it.

/// The longest item id the command looks up.
pub const MAX_ID: usize = 128;
/// The highest recipe number taken; no item has nearly as many recipes.
pub const MAX_NUMBER: u32 = 10_000;
/// How many other items of a "one of" slot its lore names.
pub const MAX_ALTERNATIVES: usize = 4;

/// Which item, and which of its recipes (1 for the first).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    /// The item in the main hand.
    Hand { number: u32 },
    /// The item with this id (`minecraft:` added when no namespace is given).
    Item { id: String, number: u32 },
}

/// Why the words typed are no request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgError {
    /// More words than `[item] [number]`.
    Usage,
    /// The item is no item id.
    BadItem,
    /// The number is 0 or larger than [`MAX_NUMBER`].
    BadNumber,
    /// The game's own `/recipe give|take <targets> <recipe>`, which stays `/minecraft:recipe`.
    Vanilla,
}

fn number(word: &str) -> Option<Result<u32, ArgError>> {
    if !word.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(
        word.parse::<u32>()
            .ok()
            .filter(|n| (1..=MAX_NUMBER).contains(n))
            .ok_or(ArgError::BadNumber),
    )
}

/// `/recipe [item] [number]`: nothing is the held item's first recipe, a number alone is the held
/// item's recipe of that number.
pub fn parse(raw: &str) -> Result<Request, ArgError> {
    let words: Vec<&str> = raw.split_whitespace().collect();
    if words.len() > 1 && matches!(words[0], "give" | "take") {
        return Err(ArgError::Vanilla);
    }
    match words.as_slice() {
        [] => Ok(Request::Hand { number: 1 }),
        [only] => match number(only) {
            Some(number) => Ok(Request::Hand { number: number? }),
            None => Ok(Request::Item {
                id: item_id(only).ok_or(ArgError::BadItem)?,
                number: 1,
            }),
        },
        [item, count] => {
            let id = item_id(item).ok_or(ArgError::BadItem)?;
            let number = number(count).unwrap_or(Err(ArgError::BadNumber))?;
            Ok(Request::Item { id, number })
        }
        _ => Err(ArgError::Usage),
    }
}

/// An item id as the server writes it: lower case, `minecraft:` when no namespace is given. `None` for
/// anything that cannot be an id (a tag's `#`, a second `:`, a space, other characters, too long).
pub fn item_id(raw: &str) -> Option<String> {
    let id = raw.trim().to_ascii_lowercase();
    let (namespace, path) = id.split_once(':').unwrap_or(("minecraft", id.as_str()));
    let namespace_ok = !namespace.is_empty()
        && namespace
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'));
    let path_ok = !path.is_empty()
        && path
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '/'));
    let full = format!("{namespace}:{path}");
    (namespace_ok && path_ok && full.len() <= MAX_ID).then_some(full)
}

/// An id as a player types it: without `minecraft:`.
pub fn short(id: &str) -> &str {
    id.strip_prefix("minecraft:").unwrap_or(id)
}

/// What a recipe slot takes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    Item(String),
    /// A tag of items, without its `#`.
    Tag(String),
    /// Any of these items; none means the slot stays empty.
    OneOf(Vec<String>),
}

/// The kinds of recipe the server reads out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Shaped,
    Shapeless,
    Transmute,
    Smelting,
    Blasting,
    Smoking,
    Campfire,
    Stonecutting,
    Smithing,
}

impl Kind {
    /// The lang key of the kind's name.
    pub const fn key(self) -> &'static str {
        match self {
            Self::Shaped => "kind_shaped",
            Self::Shapeless => "kind_shapeless",
            Self::Transmute => "kind_transmute",
            Self::Smelting => "kind_smelting",
            Self::Blasting => "kind_blasting",
            Self::Smoking => "kind_smoking",
            Self::Campfire => "kind_campfire",
            Self::Stonecutting => "kind_stonecutting",
            Self::Smithing => "kind_smithing",
        }
    }

    /// The window that shows it, as the game draws that station. A campfire has no window; it is shown as a
    /// furnace, as EssentialsX does.
    pub const fn window(self) -> Window {
        match self {
            Self::Shaped | Self::Shapeless | Self::Transmute => Window::Crafting,
            Self::Smelting | Self::Campfire => Window::Furnace,
            Self::Blasting => Window::BlastFurnace,
            Self::Smoking => Window::Smoker,
            Self::Stonecutting => Window::Stonecutter,
            Self::Smithing => Window::Smithing,
        }
    }

    pub const fn is_cooking(self) -> bool {
        matches!(
            self,
            Self::Smelting | Self::Blasting | Self::Smoking | Self::Campfire
        )
    }
}

/// The windows a recipe is shown in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Window {
    Crafting,
    Furnace,
    BlastFurnace,
    Smoker,
    Stonecutter,
    Smithing,
}

impl Window {
    /// The result slot: crafting 0 (the grid is 1 to 9), furnaces 2 (input 0, fuel 1), stonecutter 1
    /// (input 0), smithing 3 (template 0, base 1, addition 2).
    pub const fn result_slot(self) -> u32 {
        match self {
            Self::Crafting => 0,
            Self::Furnace | Self::BlastFurnace | Self::Smoker => 2,
            Self::Stonecutter => 1,
            Self::Smithing => 3,
        }
    }
}

/// A recipe as the server read it out, without its result.
#[derive(Debug, Clone, PartialEq)]
pub struct Recipe {
    pub kind: Kind,
    /// Shaped: the rows of the pattern.
    pub pattern: Vec<String>,
    /// Shaped: what each character of the pattern stands for.
    pub key: Vec<(String, Input)>,
    /// The other kinds' inputs in their slots' order.
    pub ingredients: Vec<Input>,
    /// Cooking kinds: the experience and the time in ticks.
    pub cooking: Option<(f32, u32)>,
}

const GRID: usize = 3;

/// The window's input slots and what each takes; empty slots are left out.
pub fn slots(recipe: &Recipe) -> Vec<(u32, &Input)> {
    let mut slots = Vec::new();
    match recipe.kind.window() {
        Window::Crafting if recipe.kind == Kind::Shaped => {
            for (row, line) in recipe.pattern.iter().take(GRID).enumerate() {
                for (column, symbol) in line.chars().take(GRID).enumerate() {
                    let input = recipe
                        .key
                        .iter()
                        .find(|(key, _)| key.chars().eq(std::iter::once(symbol)))
                        .map(|(_, input)| input);
                    if let Some(input) = input {
                        slots.push((slot_number(1 + row * GRID + column), input));
                    }
                }
            }
        }
        Window::Crafting => {
            for (index, input) in recipe.ingredients.iter().take(GRID * GRID).enumerate() {
                slots.push((slot_number(1 + index), input));
            }
        }
        Window::Furnace | Window::BlastFurnace | Window::Smoker | Window::Stonecutter => {
            if let Some(input) = recipe.ingredients.first() {
                slots.push((0, input));
            }
        }
        Window::Smithing => {
            for (index, input) in recipe.ingredients.iter().take(3).enumerate() {
                slots.push((slot_number(index), input));
            }
        }
    }
    slots.retain(|(_, input)| !matches!(input, Input::OneOf(items) if items.is_empty()));
    slots
}

fn slot_number(index: usize) -> u32 {
    u32::try_from(index).unwrap_or(u32::MAX)
}

/// What a slot shows: an item, the item standing for several (with the others), or a tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Shown<'a> {
    Item(&'a str, &'a [String]),
    Tag(&'a str),
}

/// What the slot taking `input` shows: an item shows itself, "one of" its first item (the others named
/// in its lore), a tag a marker named after the tag (no contract reads a tag's items yet).
pub fn shown(input: &Input) -> Option<Shown<'_>> {
    match input {
        Input::Item(id) => Some(Shown::Item(id, &[])),
        Input::Tag(tag) => Some(Shown::Tag(tag)),
        Input::OneOf(items) => items.split_first().map(|(first, others)| {
            Shown::Item(first, &others[..others.len().min(MAX_ALTERNATIVES)])
        }),
    }
}

/// An input as text: an id, `#tag`, or the items of "one of" with `|` between them.
pub fn input_text(input: &Input) -> String {
    match input {
        Input::Item(id) => short(id).to_owned(),
        Input::Tag(tag) => format!("#{}", short(tag)),
        Input::OneOf(items) if items.is_empty() => "-".to_owned(),
        Input::OneOf(items) => items
            .iter()
            .map(|id| short(id))
            .collect::<Vec<_>>()
            .join(" | "),
    }
}

/// One text line: the lang key and its arguments.
pub type Line = (&'static str, Vec<String>);

/// The recipe as text lines, for the console: the pattern and what its characters stand for, or the
/// inputs; and a cooking recipe's time and experience. The header and the result are the caller's.
pub fn text_lines(recipe: &Recipe) -> Vec<Line> {
    let mut lines = Vec::new();
    if recipe.kind == Kind::Shaped {
        for row in recipe.pattern.iter().take(GRID) {
            lines.push(("grid_row", vec![row.replace(' ', "_")]));
        }
        for (symbol, input) in &recipe.key {
            lines.push(("legend", vec![symbol.clone(), input_text(input)]));
        }
    } else {
        let inputs: Vec<String> = recipe
            .ingredients
            .iter()
            .filter(|input| !matches!(input, Input::OneOf(items) if items.is_empty()))
            .map(input_text)
            .collect();
        lines.push(("inputs", vec![inputs.join(", ")]));
    }
    lines.extend(cooking_line(recipe));
    lines
}

/// A cooking recipe's time in seconds and its experience.
pub fn cooking_line(recipe: &Recipe) -> Option<Line> {
    let (experience, ticks) = recipe.cooking.filter(|_| recipe.kind.is_cooking())?;
    let seconds = f64::from(ticks) / 20.0;
    Some((
        "cooking",
        vec![trim_number(seconds), trim_number(f64::from(experience))],
    ))
}

/// A number with at most two decimals and no trailing zeros: 10, 0.35, 7.5.
fn trim_number(value: f64) -> String {
    let text = format!("{value:.2}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// The line under the window: which recipe this is and how to see the next.
pub fn foot(id: &str, number: u32, total: u32) -> Option<Line> {
    if total <= 1 {
        return None;
    }
    let (number_text, total_text) = (number.to_string(), total.to_string());
    Some(if number < total {
        (
            "next",
            vec![
                number_text,
                total_text,
                short(id).to_owned(),
                (number + 1).to_string(),
            ],
        )
    } else {
        ("last", vec![number_text, total_text])
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str) -> Input {
        Input::Item(id.into())
    }

    #[test]
    fn nothing_or_a_number_alone_is_the_held_item() {
        assert_eq!(parse(""), Ok(Request::Hand { number: 1 }));
        assert_eq!(parse("   "), Ok(Request::Hand { number: 1 }));
        assert_eq!(parse("3"), Ok(Request::Hand { number: 3 }));
        assert_eq!(parse("0"), Err(ArgError::BadNumber));
        assert_eq!(parse("99999999999"), Err(ArgError::BadNumber));
    }

    #[test]
    fn an_item_and_a_number_are_read_and_more_words_are_refused() {
        assert_eq!(
            parse("Stick"),
            Ok(Request::Item {
                id: "minecraft:stick".into(),
                number: 1
            })
        );
        assert_eq!(
            parse("minecraft:oak_planks 2"),
            Ok(Request::Item {
                id: "minecraft:oak_planks".into(),
                number: 2
            })
        );
        assert_eq!(parse("stick two"), Err(ArgError::BadNumber));
        assert_eq!(parse("stick 0"), Err(ArgError::BadNumber));
        assert_eq!(parse("stick 1 2"), Err(ArgError::Usage));
        assert_eq!(parse("#minecraft:planks"), Err(ArgError::BadItem));
    }

    #[test]
    fn the_games_own_give_and_take_are_pointed_to_minecraft_recipe() {
        assert_eq!(parse("give Steve *"), Err(ArgError::Vanilla));
        assert_eq!(parse("take @a minecraft:stick"), Err(ArgError::Vanilla));
        // `give` alone could only be an item.
        assert_eq!(
            parse("give"),
            Ok(Request::Item {
                id: "minecraft:give".into(),
                number: 1
            })
        );
    }

    #[test]
    fn an_item_id_gets_its_namespace_and_a_wrong_one_is_refused() {
        assert_eq!(item_id("stick").as_deref(), Some("minecraft:stick"));
        assert_eq!(item_id("MyPack:Ruby").as_deref(), Some("mypack:ruby"));
        assert_eq!(item_id("a:b:c"), None);
        assert_eq!(item_id(":stick"), None);
        assert_eq!(item_id("stick:"), None);
        assert_eq!(item_id("st ick"), None);
        assert_eq!(item_id(&"a".repeat(MAX_ID)), None);
        assert_eq!(short("minecraft:stick"), "stick");
        assert_eq!(short("mypack:ruby"), "mypack:ruby");
    }

    fn shaped(pattern: &[&str], key: &[(&str, Input)]) -> Recipe {
        Recipe {
            kind: Kind::Shaped,
            pattern: pattern.iter().map(|row| (*row).to_owned()).collect(),
            key: key
                .iter()
                .map(|(symbol, input)| ((*symbol).to_owned(), input.clone()))
                .collect(),
            ingredients: vec![],
            cooking: None,
        }
    }

    #[test]
    fn a_shaped_recipe_fills_the_grid_from_its_top_left_as_the_game_does() {
        let planks = Input::Tag("minecraft:planks".into());
        let stick = shaped(&["#", "#"], &[("#", planks.clone())]);
        assert_eq!(slots(&stick), vec![(1, &planks), (4, &planks)]);
        let iron = item("minecraft:iron_ingot");
        let rod = item("minecraft:stick");
        let pickaxe = shaped(
            &["XXX", " # ", " # "],
            &[("X", iron.clone()), ("#", rod.clone())],
        );
        assert_eq!(
            slots(&pickaxe),
            vec![(1, &iron), (2, &iron), (3, &iron), (5, &rod), (8, &rod)]
        );
        assert_eq!(Kind::Shaped.window().result_slot(), 0);
    }

    #[test]
    fn the_other_kinds_fill_their_stations_slots() {
        let input = |kind, ingredients: Vec<Input>| Recipe {
            kind,
            pattern: vec![],
            key: vec![],
            ingredients,
            cooking: None,
        };
        let dye = input(
            Kind::Shapeless,
            vec![item("minecraft:red_dye"), item("minecraft:white_wool")],
        );
        assert_eq!(slots(&dye).iter().map(|s| s.0).collect::<Vec<_>>(), [1, 2]);
        let ore = input(Kind::Smelting, vec![item("minecraft:iron_ore")]);
        assert_eq!(slots(&ore).iter().map(|s| s.0).collect::<Vec<_>>(), [0]);
        assert_eq!(Kind::Smelting.window(), Window::Furnace);
        assert_eq!(Kind::Campfire.window(), Window::Furnace);
        assert_eq!(Kind::Blasting.window().result_slot(), 2);
        let cut = input(Kind::Stonecutting, vec![item("minecraft:stone")]);
        assert_eq!(slots(&cut).iter().map(|s| s.0).collect::<Vec<_>>(), [0]);
        assert_eq!(Kind::Stonecutting.window().result_slot(), 1);
        // A smithing slot that must stay empty is an empty "one of".
        let smith = input(
            Kind::Smithing,
            vec![
                item("minecraft:netherite_upgrade_smithing_template"),
                item("minecraft:diamond_sword"),
                Input::OneOf(vec![]),
            ],
        );
        assert_eq!(
            slots(&smith).iter().map(|s| s.0).collect::<Vec<_>>(),
            [0, 1]
        );
        assert_eq!(Kind::Smithing.window().result_slot(), 3);
    }

    #[test]
    fn a_slot_shows_its_item_the_first_of_several_or_its_tag() {
        assert_eq!(
            shown(&item("minecraft:stick")),
            Some(Shown::Item("minecraft:stick", &[]))
        );
        let many: Vec<String> = (0..7).map(|i| format!("minecraft:i{i}")).collect();
        let one_of = Input::OneOf(many.clone());
        let Some(Shown::Item(first, others)) = shown(&one_of) else {
            panic!("one of shows an item");
        };
        assert_eq!(first, "minecraft:i0");
        assert_eq!(others, &many[1..=MAX_ALTERNATIVES]);
        assert_eq!(
            shown(&Input::Tag("minecraft:logs".into())),
            Some(Shown::Tag("minecraft:logs"))
        );
        assert_eq!(shown(&Input::OneOf(vec![])), None);
    }

    #[test]
    fn the_console_reads_the_pattern_or_the_inputs_and_the_cooking() {
        let stick = shaped(&["#", "#"], &[("#", Input::Tag("minecraft:planks".into()))]);
        assert_eq!(
            text_lines(&stick),
            vec![
                ("grid_row", vec!["#".to_owned()]),
                ("grid_row", vec!["#".to_owned()]),
                ("legend", vec!["#".to_owned(), "#planks".to_owned()]),
            ]
        );
        let ore = Recipe {
            kind: Kind::Smelting,
            pattern: vec![],
            key: vec![],
            ingredients: vec![Input::OneOf(vec![
                "minecraft:iron_ore".into(),
                "minecraft:deepslate_iron_ore".into(),
            ])],
            cooking: Some((0.7, 200)),
        };
        assert_eq!(
            text_lines(&ore),
            vec![
                ("inputs", vec!["iron_ore | deepslate_iron_ore".to_owned()]),
                ("cooking", vec!["10".to_owned(), "0.7".to_owned()]),
            ]
        );
    }

    #[test]
    fn the_foot_names_the_next_recipe_until_the_last() {
        assert_eq!(foot("minecraft:stick", 1, 1), None);
        assert_eq!(
            foot("minecraft:stick", 1, 3),
            Some((
                "next",
                vec!["1".into(), "3".into(), "stick".into(), "2".into()]
            ))
        );
        assert_eq!(
            foot("minecraft:stick", 3, 3),
            Some(("last", vec!["3".into(), "3".into()]))
        );
    }
}
