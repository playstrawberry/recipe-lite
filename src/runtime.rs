use crate::{
    lang,
    model::{self, ArgError, Input, Kind, Recipe, Request, Shown, Window},
};
use strawberry_plugin_api::{
    Context, ItemStack, Player, Plugin, PluginMetadata, Result, Screen, Server,
    command::{
        Arg, ArgumentType, Command, CommandError, CommandNode, CommandSender, ConsumedArgs,
        StringType,
    },
    commands::CommandHandler,
    common::{Hand, NamedColor},
    gui::Gui,
    permission::{Permission, PermissionDefault},
    recipe::{RecipeInfo, RecipeKind, WitIngredient},
    register_plugin,
    text::TextComponent,
};

const USE: &str = "recipe-lite:use";
/// The item a tag's slot shows, named after the tag: no contract reads a tag's items yet.
const TAG_MARKER: &str = "minecraft:name_tag";

fn kind(kind: RecipeKind) -> Kind {
    match kind {
        RecipeKind::Shaped => Kind::Shaped,
        RecipeKind::Shapeless => Kind::Shapeless,
        RecipeKind::Transmute => Kind::Transmute,
        RecipeKind::Smelting => Kind::Smelting,
        RecipeKind::Blasting => Kind::Blasting,
        RecipeKind::Smoking => Kind::Smoking,
        RecipeKind::Campfire => Kind::Campfire,
        RecipeKind::Stonecutting => Kind::Stonecutting,
        RecipeKind::SmithingTransform => Kind::Smithing,
    }
}

fn input(ingredient: WitIngredient) -> Input {
    match ingredient {
        WitIngredient::Item(id) => Input::Item(id),
        WitIngredient::Tag(tag) => Input::Tag(tag),
        WitIngredient::OneOf(items) => Input::OneOf(items),
    }
}

/// The recipe without its result, and the result.
fn split(info: RecipeInfo) -> (Recipe, ItemStack) {
    let recipe = Recipe {
        kind: kind(info.kind),
        pattern: info.pattern,
        key: info
            .key
            .into_iter()
            .map(|(symbol, ingredient)| (symbol, input(ingredient)))
            .collect(),
        ingredients: info.ingredients.into_iter().map(input).collect(),
        cooking: info.cooking.map(|c| (c.experience, c.cooking_time)),
    };
    (recipe, info.output)
}

const fn screen(window: Window) -> Screen {
    match window {
        Window::Crafting => Screen::Crafting,
        Window::Furnace => Screen::Furnace,
        Window::BlastFurnace => Screen::BlastFurnace,
        Window::Smoker => Screen::Smoker,
        Window::Stonecutter => Screen::Stonecutter,
        Window::Smithing => Screen::Smithing,
    }
}

fn plain(text: &str) -> TextComponent {
    TextComponent::text(text)
        .italic(false)
        .color_named(NamedColor::White)
}

/// The stack a slot shows: the item itself, or a marker named after a tag.
fn stack(locale: &str, shown: &Shown<'_>) -> ItemStack {
    match shown {
        Shown::Item(id, others) => {
            let stack = ItemStack::new(id, 1);
            if !others.is_empty() {
                let names: Vec<&str> = others.iter().map(|id| model::short(id)).collect();
                stack.set_lore(vec![
                    plain(&lang::text(locale, "others", &[&names.join(", ")]))
                        .color_named(NamedColor::Gray),
                ]);
            }
            stack
        }
        Shown::Tag(tag) => {
            let stack = ItemStack::new(TAG_MARKER, 1);
            stack.set_custom_name(Some(plain(&lang::text(
                locale,
                "tag_slot",
                &[model::short(tag)],
            ))));
            stack
        }
    }
}

/// The recipe in its station's window, which nothing can be taken from or put into.
fn open(player: &Player, locale: &str, recipe: &Recipe, result: ItemStack, title: &str) {
    let window = recipe.kind.window();
    let gui = Gui::new(screen(window), TextComponent::text(title));
    for (slot, input) in model::slots(recipe) {
        if let Some(shown) = model::shown(input) {
            gui.set_item(slot, stack(locale, &shown));
        }
    }
    gui.set_item(window.result_slot(), result);
    gui.set_allow_grab_items(false);
    gui.set_allow_put_items(false);
    player.open_gui(gui);
}

struct Run;

impl CommandHandler for Run {
    fn handle(
        &self,
        sender: CommandSender,
        server: Server,
        args: ConsumedArgs,
    ) -> std::result::Result<i32, CommandError> {
        let player = sender.as_player();
        let locale = match &player {
            Some(player) => player.get_locale(),
            None => format!("{:?}", sender.get_locale()),
        };
        let refuse = |key: &str, args: &[&str]| {
            CommandError::CommandFailed(
                TextComponent::text(&lang::text(&locale, key, args)).color_named(NamedColor::Red),
            )
        };
        if player.as_ref().is_some_and(|p| !p.has_permission(USE)) {
            return Err(refuse("no_permission", &[]));
        }
        let raw = match args.get_value("item") {
            Arg::Simple(raw) => raw,
            _ => String::new(),
        };
        let (id, number) = match model::parse(&raw) {
            Ok(Request::Item { id, number }) => (id, number),
            Ok(Request::Hand { number }) => {
                let Some(player) = &player else {
                    return Err(refuse("players_only", &[]));
                };
                // The main hand, as every first-party script reads it.
                let id = player
                    .get_item_in_hand(Hand::Right)
                    .filter(|stack| stack.get_count() > 0)
                    .map(|stack| stack.get_registry_key())
                    .filter(|id| !id.is_empty() && id != "minecraft:air")
                    .ok_or_else(|| refuse("empty_hand", &[]))?;
                (id, number)
            }
            Err(ArgError::Usage) => return Err(refuse("usage", &[])),
            Err(ArgError::BadItem) => return Err(refuse("bad_item", &[])),
            Err(ArgError::BadNumber) => return Err(refuse("bad_number", &[])),
            Err(ArgError::Vanilla) => return Err(refuse("vanilla", &[])),
        };
        // One question to the server: its recipes for the item, from the one asked for on.
        let page = server
            .get_recipe_manager()
            .recipes_for(&id, number.saturating_sub(1));
        let name = model::short(&id);
        if page.total == 0 {
            return Err(refuse("no_recipe", &[name]));
        }
        let Some(info) = page.recipes.into_iter().next() else {
            return Err(refuse("no_such", &[name, &page.total.to_string()]));
        };
        let (recipe, result) = split(info);
        let total = page.total;
        let say = |line: &model::Line, color: NamedColor| {
            sender.send_message(TextComponent::text(&lang::line(&locale, line)).color_named(color));
        };
        match &player {
            Some(player) => {
                let title = lang::text(
                    &locale,
                    "title",
                    &[name, &number.to_string(), &total.to_string()],
                );
                open(player, &locale, &recipe, result, &title);
                if let Some(line) = model::cooking_line(&recipe) {
                    say(&line, NamedColor::Gray);
                }
            }
            None => {
                let kind = lang::text(&locale, recipe.kind.key(), &[]);
                let head = (
                    "head",
                    vec![number.to_string(), total.to_string(), name.to_owned(), kind],
                );
                say(&head, NamedColor::Gold);
                for line in model::text_lines(&recipe) {
                    say(&line, NamedColor::White);
                }
                let made = (
                    "result",
                    vec![
                        result.get_count().to_string(),
                        model::short(&result.get_registry_key()).to_owned(),
                    ],
                );
                say(&made, NamedColor::White);
            }
        }
        if let Some(line) = model::foot(&id, number, total) {
            say(&line, NamedColor::Gray);
        }
        Ok(1)
    }
}

struct RecipeLite;

impl Plugin for RecipeLite {
    fn new() -> Self {
        Self
    }

    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            name: "recipe-lite".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            authors: vec!["Strawberry Interactive LLC".into()],
            description: lang::text("en", "description", &[]),
            dependencies: vec![],
            permissions: vec![],
        }
    }

    fn on_load(&self, context: Context) -> Result<()> {
        context.register_permission(&Permission {
            node: USE.into(),
            description: lang::text("en", "permission_use", &[]),
            default: PermissionDefault::Allow,
            children: vec![],
        })?;
        // `/recipe [item] [number]`: one greedy argument, read by the script, so that an id with its
        // namespace (`minecraft:stick`) is taken as typed.
        context.register_command(
            Command::new(&["recipe".into()], &lang::text("en", "command_recipe", &[]))
                .execute(Run)
                .then(
                    CommandNode::argument("item", &ArgumentType::String(StringType::Greedy))
                        .execute(Run),
                ),
            USE,
        );
        Ok(())
    }

    fn on_unload(&self, _context: Context) -> Result<()> {
        Ok(())
    }
}

register_plugin!(RecipeLite);
