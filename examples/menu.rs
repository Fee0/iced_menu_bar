//! A small but fairly complete tour of the `iced_menu_bar` API.
//!
//! Run with: `cargo run --example menu`
//!
//! It shows:
//! - a [`MenuBar`] with several root items,
//! - the built-in [`Item::root`] / [`Item::leaf`] / [`Item::submenu`] constructors and [`separator`],
//! - nested submenus,
//! - [`Item::close_on_click`] overrides,
//! - the fallible [`Menu::try_new`] constructor returning [`iced_menu_bar::Result`],
//! - and the crate's built-in default styling (no custom `.style(..)` needed).

use iced::widget::{column, container, svg, text};
use iced::{Element, Fill, Renderer, Task, Theme};

use iced_menu_bar::{Item, Menu, MenuBar, separator};

/// The widget types are generic over the theme, so the example spells out the concrete
/// `Theme`/`Renderer` it uses (there are no default type parameters to lean on).
type MenuItem = Item<'static, Message, Theme, Renderer>;

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
    /// A root / submenu button was pressed (needed so the button renders as active).
    OpenMenu,
}

impl App {
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Selected(label) => self.last_action = Some(label.to_owned()),
            Message::OpenMenu => {}
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
    // Leaves, roots and submenu entries all come from the crate now — no hand-built
    // buttons. `Item::root` is the content-sized top-level bar button; `Item::submenu` is a
    // full-width in-menu entry that opens a nested flyout; the `*_styled` variants would let us
    // swap in a custom button style per item.
    // `leaf_with_icon` / `submenu_with_icon` put an icon in a fixed-width column on the left. Every
    // leaf/submenu row reserves that column, so "Exit" (no icon) still lines up with the iconned
    // entries above it.
    let file = Item::root(
        "File",
        Message::OpenMenu,
        Menu::new(vec![
            Item::leaf_with_icon("New", icon(NEW_ICON), Message::Selected("New")),
            Item::leaf_with_icon("Open", icon(OPEN_ICON), Message::Selected("Open")),
            separator(),
            Item::submenu_with_icon(
                "Open Recent",
                icon(OPEN_ICON),
                Message::OpenMenu,
                Menu::new(vec![leaf("project.hex"), leaf("notes.txt")]),
            ),
            separator(),
            leaf("Exit"),
        ]),
    );

    let edit = Item::root(
        "Edit",
        Message::OpenMenu,
        Menu::new(vec![
            leaf("Cut"),
            // Keep the menu open after clicking "Copy".
            leaf("Copy").close_on_click(false),
            leaf("Paste"),
        ]),
    );

    // `try_new` rejects an empty item list — here it always succeeds.
    let help_menu = Menu::try_new(vec![leaf("About")])
        .expect("the help menu is non-empty")
        .width(160);
    let help = Item::root("Help", Message::OpenMenu, help_menu);

    MenuBar::new(vec![file, edit, help]).width(Fill).into()
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
