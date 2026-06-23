//! A fairly complete tour of the `iced_menu_bar` API.
//!
//! Run with: `cargo run --example menu`
//!
//! Features demonstrated:
//! - Root items, nested submenus, icons, hotkey hints, disabled entries.
//! - [`separator`] and [`group_header`] for visual grouping within a menu.
//! - Checkable toggle items (`.checked`) and exclusive radio groups (`.radio`).
//! - Alt / F10 activates the bar for full keyboard navigation without a mouse.

use iced::widget::{column, container, svg, text};
use iced::{Element, Fill, Task, Theme};

use iced_menu_bar::{Item, Menu, MenuBar, Status, Style, default_style, group_header, separator};

/// The widget types default to iced's built-in `Theme`/`Renderer`, so the common case only needs
/// the lifetime and `Message`.
type MenuItem = Item<'static, Message>;

pub fn main() -> iced::Result {
    iced::application(App::default, App::update, App::view)
        .title("iced_menu_bar example")
        .window_size((640.0, 300.0))
        .theme(|app: &App| app.theme())
        .run()
}

#[derive(Default)]
struct App {
    last_action: Option<String>,
    // Feature 1: checkbox state — toggled by their own messages
    show_toolbar: bool,
    show_statusbar: bool,
    // Feature 1: radio state — which theme is selected
    dark_theme: bool,
}

impl App {
    fn theme(&self) -> Theme {
        if self.dark_theme {
            Theme::Dark
        } else {
            Theme::Light
        }
    }
}

#[derive(Debug, Clone)]
enum Message {
    Selected(&'static str),
    ToggleToolbar,
    ToggleStatusBar,
    SetDarkTheme(bool),
}

impl App {
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Selected(label) => self.last_action = Some(label.to_owned()),
            Message::ToggleToolbar => self.show_toolbar = !self.show_toolbar,
            Message::ToggleStatusBar => self.show_statusbar = !self.show_statusbar,
            Message::SetDarkTheme(dark) => self.dark_theme = dark,
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let hint = match &self.last_action {
            Some(label) => format!("Last action: {label}"),
            None => "Open a menu and pick an entry…  (Alt or F10 activates the bar for keyboard navigation)".to_owned(),
        };

        column![
            menu_bar(self),
            container(text(hint)).padding(20).center_x(Fill),
        ]
        .into()
    }
}

/// Builds the menu bar.  State from the app is needed for checkable / radio items (Feature 1).
fn menu_bar(app: &App) -> Element<'static, Message> {
    // Copy primitive state before the 'static boundary — no app references inside items.
    let show_toolbar = app.show_toolbar;
    let show_statusbar = app.show_statusbar;
    let dark_theme = app.dark_theme;

    let file = file_menu();
    let edit = edit_menu();
    let view = view_menu(show_toolbar, show_statusbar, dark_theme);
    let format = format_menu();
    let tools = tools_menu();
    let window = window_menu();
    let help = help_menu();

    MenuBar::new(vec![file, edit, view, format, tools, window, help])
        .width(Fill)
        .open_on_hover(true)
        .style(|theme, status| {
            let base = default_style(theme, status);
            match status {
                Status::Hovered | Status::Selected | Status::Focused => Style {
                    path: theme.extended_palette().primary.base.color.into(),
                    ..base
                },
                _ => base,
            }
        })
        .into()
}

// ---------------------------------------------------------------------------
// Individual menus
// ---------------------------------------------------------------------------

fn file_menu() -> MenuItem {
    Item::root(
        "File",
        Menu::new(vec![
            // Feature 2: group_header labels a section without being interactive.
            group_header("Document"),
            Item::action("New", Message::Selected("New"))
                .icon(icon(NEW_ICON))
                .hotkey("⌘N")
                .build(),
            Item::action("Open", Message::Selected("Open"))
                .icon(icon(OPEN_ICON))
                .hotkey("⌘O")
                .build(),
            Item::action("Save", Message::Selected("Save"))
                .hotkey("⌘S")
                .build(),
            // Disabled: greyed out, ignores clicks.
            Item::action("Save As…", Message::Selected("Save As"))
                .disabled()
                .build(),
            separator(),
            // Feature 2: a second header within the same menu.
            group_header("Recent"),
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
    .build()
}

fn edit_menu() -> MenuItem {
    Item::root(
        "Edit",
        Menu::new(vec![
            group_header("Clipboard"),
            Item::action("Cut", Message::Selected("Cut"))
                .hotkey("⌘X")
                .build(),
            // keep_open: the menu stays open after clicking "Copy".
            Item::action("Copy", Message::Selected("Copy"))
                .hotkey("⌘C")
                .build()
                .keep_open(),
            Item::action("Paste", Message::Selected("Paste"))
                .hotkey("⌘V")
                .build(),
            separator(),
            group_header("History"),
            Item::action("Undo", Message::Selected("Undo"))
                .hotkey("⌘Z")
                .build(),
            Item::action("Redo", Message::Selected("Redo"))
                .hotkey("⌘⇧Z")
                .build(),
        ]),
    )
    .build()
}

/// Feature 1: checkable toggles and a radio group for theme selection.
fn view_menu(show_toolbar: bool, show_statusbar: bool, dark_theme: bool) -> MenuItem {
    Item::root(
        "View",
        Menu::new(vec![
            group_header("Panels"),
            // .checked(bool) renders a ✓ glyph in the icon slot when true.
            Item::action("Toolbar", Message::ToggleToolbar)
                .checked(show_toolbar)
                .build(),
            Item::action("Status Bar", Message::ToggleStatusBar)
                .checked(show_statusbar)
                .build(),
            separator(),
            group_header("Appearance"),
            // .radio(bool) renders a • glyph when true — use it for exclusive options.
            Item::action("Light", Message::SetDarkTheme(false))
                .radio(!dark_theme)
                .build(),
            Item::action("Dark", Message::SetDarkTheme(true))
                .radio(dark_theme)
                .build(),
        ]),
    )
    .build()
}

fn format_menu() -> MenuItem {
    Item::root(
        "Format",
        Menu::new(vec![
            Item::action("Bold", Message::Selected("Bold"))
                .hotkey("⌘B")
                .build(),
            Item::action("Italic", Message::Selected("Italic"))
                .hotkey("⌘I")
                .build(),
            Item::action("Underline", Message::Selected("Underline"))
                .hotkey("⌘U")
                .build(),
        ]),
    )
    .build()
}

fn tools_menu() -> MenuItem {
    Item::root(
        "Tools",
        Menu::new(vec![leaf("Settings"), leaf("Extensions")]),
    )
    .build()
}

fn window_menu() -> MenuItem {
    Item::root(
        "Window",
        Menu::new(vec![leaf("New Window"), leaf("Minimize"), leaf("Maximize")]),
    )
    .build()
}

fn help_menu() -> MenuItem {
    Item::root("Help", Menu::new(vec![leaf("About")]).width(160)).build()
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// A leaf entry that publishes [`Message::Selected`] with its own label when clicked.
fn leaf(label: &'static str) -> MenuItem {
    Item::leaf(label, Message::Selected(label))
}

const NEW_ICON: &[u8] = include_bytes!("../svg/file-plus.svg");
const OPEN_ICON: &[u8] = include_bytes!("../svg/folder.svg");

/// Builds a 16×16 menu icon from raw SVG bytes, tinted to follow the theme's text color.
fn icon(bytes: &'static [u8]) -> Element<'static, Message> {
    svg(svg::Handle::from_memory(bytes))
        .width(16)
        .height(16)
        .style(|theme: &Theme, _status| svg::Style {
            color: Some(theme.extended_palette().background.base.text),
        })
        .into()
}
