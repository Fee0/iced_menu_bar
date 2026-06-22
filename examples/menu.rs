//! A small but fairly complete tour of the `iced_menu_bar` API.
//!
//! Run with: `cargo run --example menu`
//!
//! It shows:
//! - a [`MenuBar`] with several root items,
//! - the built-in [`Item::root`] / [`Item::leaf`] / [`Item::submenu`] constructors and [`separator`],
//! - nested submenus,
//! - per-item text tooltips via [`Item::tooltip_text`],
//! - [`Item::close_on_click`] overrides,
//! - the fallible [`Menu::try_new`] constructor returning [`iced_menu_bar::Result`],
//! - and the crate's built-in default styling (no custom `.style(..)` needed).

use iced::widget::{column, container, text};
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
    // Leaves, roots, submenu entries and tooltips all come from the crate now — no hand-built
    // buttons. `Item::root` is the content-sized top-level bar button; `Item::submenu` is a
    // full-width in-menu entry that opens a nested flyout; the `*_styled` variants would let us
    // swap in a custom button style per item.
    let file = Item::root(
        "File",
        Message::OpenMenu,
        Menu::new(vec![
            leaf("New"),
            leaf("Open").tooltip_text("Open an existing file"),
            separator(),
            Item::submenu(
                "Open Recent",
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
            leaf("Paste").tooltip_text("Insert clipboard contents"),
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
