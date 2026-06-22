//! A small but fairly complete tour of the `iced_menu_bar` API.
//!
//! Run with: `cargo run --example menu`
//!
//! It shows:
//! - a [`MenuBar`] with several root items,
//! - the built-in [`Item::root`] / [`Item::leaf`] / [`Item::submenu`] / [`Item::action`] builders
//!   and [`separator`],
//! - nested submenus,
//! - the [`Item::keep_open`] per-item dismiss override,
//! - and the crate's built-in default styling (no custom `.style(..)` needed).

use iced::widget::{column, container, svg, text};
use iced::{Element, Fill, Task, Theme};

use iced_menu_bar::{Item, Menu, MenuBar, separator};

/// The widget types default to iced's built-in `Theme`/`Renderer`, so the common case only needs
/// the lifetime and `Message`.
type MenuItem = Item<'static, Message>;

pub fn main() -> iced::Result {
    iced::application(App::default, App::update, App::view)
        .title("iced_menu_bar example")
        .theme(dark_theme)
        .run()
}

fn dark_theme(_state: &App) -> Theme {
    Theme::Dark
}

#[derive(Default)]
struct App {
    /// The label of the most recently selected menu entry.
    last_action: Option<String>,
}

#[derive(Debug, Clone)]
enum Message {
    /// A leaf menu entry was selected.
    Selected(&'static str),
}

impl App {
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Selected(label) => self.last_action = Some(label.to_owned()),
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let status = text(match &self.last_action {
            Some(label) => format!("Last action: {label}"),
            None => "Open a menu and pick an entry…".to_owned(),
        });

        column![menu_bar(), container(status).padding(20).center_x(Fill),].into()
    }
}

/// Builds the menu bar, exercising most of the builder surface.
fn menu_bar() -> Element<'static, Message> {
    // Leaves, roots and submenu entries all come from the crate now — no hand-built buttons.
    // `Item::root(label, menu)` is the content-sized top-level bar button; `Item::submenu(label,
    // menu)` is a full-width in-menu entry that opens a nested flyout. Neither needs a message —
    // the `MenuBar` opens them. Both are builders: chain `.icon(..)` / `.style(..)` and `.build()`.
    // `Item::action(label, msg)` is the builder for action rows: chain `.icon(..)` (a fixed-width
    // column on the left, reserved on every row so labels align) and/or `.hotkey(..)` (a dimmed,
    // right-aligned shortcut hint), then `.build()`. Hotkeys are display-only and not available on
    // submenus, which keep their right-side chevron. "Exit" has neither, yet still lines up.
    let file = Item::root(
        "File",
        Menu::new(vec![
            Item::action("New", Message::Selected("New"))
                .icon(icon(NEW_ICON))
                .hotkey("⌘N")
                .build(),
            Item::action("Open", Message::Selected("Open"))
                .icon(icon(OPEN_ICON))
                .hotkey("⌘O")
                .build(),
            // A hotkey with no icon — still right-aligned, label still lines up via the icon column.
            Item::action("Save", Message::Selected("Save")).hotkey("⌘S").build(),
            // A disabled action: greyed out, ignores clicks, keeps the menu open.
            Item::action("Save As…", Message::Selected("Save As")).disabled().build(),
            separator(),
            Item::submenu(
                "Open Recent",
                Menu::new(vec![leaf("project.hex"), leaf("notes.txt")]),
            )
            .icon(icon(OPEN_ICON))
            .build(),
            separator(),
            leaf("Exit"),
        ]),
    )
    .build();

    let edit = Item::root(
        "Edit",
        Menu::new(vec![
            leaf("Cut"),
            // Keep the menu open after clicking "Copy".
            leaf("Copy").keep_open(),
            leaf("Paste"),
        ]),
    )
    .build();

    let help = Item::root("Help", Menu::new(vec![leaf("About")]).width(160)).build();

    MenuBar::new(vec![file, edit, help])
        .width(Fill)
        .open_on_hover(true)
        .into()
}

/// A leaf entry that publishes [`Message::Selected`] with its own label when clicked.
fn leaf(label: &'static str) -> MenuItem {
    Item::leaf(label, Message::Selected(label))
}

const NEW_ICON: &[u8] = include_bytes!("../svg/file-plus.svg");
const OPEN_ICON: &[u8] = include_bytes!("../svg/folder.svg");

/// Builds a 16×16 menu icon from raw SVG bytes, tinted to follow the theme's text color.
///
/// The crate hands the icon column to the caller untinted, so styling is done here.
fn icon(bytes: &'static [u8]) -> Element<'static, Message> {
    svg(svg::Handle::from_memory(bytes))
        .width(16)
        .height(16)
        .style(|theme: &Theme, _status| svg::Style {
            color: Some(theme.extended_palette().background.base.text),
        })
        .into()
}
