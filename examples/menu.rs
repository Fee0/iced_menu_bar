//! A small but fairly complete tour of the `iced_menu_bar` API.
//!
//! Run with: `cargo run --example menu`
//!
//! It shows:
//! - a [`MenuBar`] with several configured root items,
//! - element-based [`Item`]s wrapping ordinary `button`s,
//! - nested submenus via [`Item::with_menu`],
//! - per-item [tooltips](Item::tooltip) and a custom separator element,
//! - [`Item::close_on_click`] overrides,
//! - the fallible [`Menu::try_new`] constructor returning [`iced_menu_bar::Result`],
//! - and the crate's built-in default bar/flyout styling (no custom `.style(..)` needed).

use iced::widget::{button, column, container, text};
use iced::{Element, Fill, Renderer, Task, Theme};

use iced_menu_bar::{Item, Menu, MenuBar, menu_item_style, separator};

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
    let dropdown = Menu::new;

    let file = Item::with_menu(
        root_button("File"),
        dropdown(vec![
            leaf("New"),
            leaf("Open").tooltip(tooltip_text("Open an existing file")),
            separator(),
            Item::with_menu(
                root_button("Open Recent"),
                dropdown(vec![leaf("project.hex"), leaf("notes.txt")]),
            ),
            separator(),
            leaf("Exit"),
        ]),
    );

    let edit = Item::with_menu(
        root_button("Edit"),
        dropdown(vec![
            leaf("Cut"),
            // Keep the menu open after clicking "Copy".
            leaf("Copy").close_on_click(false),
            leaf("Paste").tooltip(tooltip_text("Insert clipboard contents")),
        ]),
    );

    // `try_new` rejects an empty item list — here it always succeeds.
    let help_menu = Menu::try_new(vec![leaf("About")])
        .expect("the help menu is non-empty")
        .width(160);
    let help = Item::with_menu(root_button("Help"), help_menu);

    MenuBar::new(vec![file, edit, help]).width(Fill).into()
}

/// A leaf entry that publishes [`Message::Selected`] when clicked.
fn leaf(label: &'static str) -> MenuItem {
    Item::new(
        button(text(label))
            .width(Fill)
            .padding([5, 12])
            .style(menu_item_style)
            .on_press(Message::Selected(label)),
    )
}

/// A top-level / submenu button. The `on_press` makes the button look active.
fn root_button(label: &'static str) -> iced::widget::Button<'static, Message> {
    button(text(label))
        .padding([5, 10])
        .style(menu_item_style)
        .on_press(Message::OpenMenu)
}

/// A styled tooltip body. The crate draws the tooltip background/border itself
/// (using the menu colors), so the body just needs padded, readable text.
fn tooltip_text(content: &'static str) -> Element<'static, Message> {
    container(text(content).size(13)).padding([4, 8]).into()
}
