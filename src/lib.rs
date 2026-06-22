//! A custom menu-bar widget for [iced](https://github.com/iced-rs/iced).
//!
//! The menu bar is implemented as a real [`Widget`](iced::advanced::Widget): the bar lays
//! out its root [`Item`]s horizontally, and the dropdown [`Menu`]s are rendered through
//! iced's [`Overlay`](iced::advanced::overlay::Overlay) system so they float above the rest
//! of the interface and can flyout into nested submenus.
//!
//! Items are **element based** — every [`Item`] wraps an arbitrary [`Element`](iced::Element)
//! (typically a styled button containing a row of text, an icon, and an accelerator hint), so
//! the bar composes with the rest of an iced application without a bespoke data model.
//!
//! The widget is generic over the theme: any `Theme` that implements [`Catalog`] can be used.
//! A convenience implementation is provided for the built-in [`iced::Theme`].
//!
//! ```ignore
//! use iced_menu_bar::{Item, Menu, MenuBar};
//!
//! let menu_bar = MenuBar::new(vec![
//!     Item::root(
//!         "File",
//!         Menu::new(vec![
//!             Item::leaf("New", Message::New),
//!             Item::leaf("Open", Message::Open),
//!         ]),
//!     )
//!     .build(),
//! ]);
//! ```

mod common;
mod flex;
mod menu;
mod menu_bar;
mod overlay;
mod style;

pub use common::{Dismiss, PathHighlight, ScrollSpeed};
pub use menu::{ActionBuilder, Item, Menu, RootBuilder, SubmenuBuilder, separator};
pub use menu_bar::MenuBar;
pub use style::{
    Catalog, Style, StyleFn, default_style, menu_item_disabled_style, menu_item_style,
};
