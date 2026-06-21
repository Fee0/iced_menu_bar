//! A custom menu-bar widget for [iced](https://github.com/iced-rs/iced).
//!
//! The menu bar is implemented as a real [`Widget`](iced::advanced::Widget): the bar lays
//! out its root [`Item`]s horizontally, and the dropdown [`Menu`]s are rendered through
//! iced's [`Overlay`](iced::advanced::overlay::Overlay) system so they float above the rest
//! of the interface and can flyout into nested submenus.
//!
//! Items are **element based** — every [`Item`] wraps an arbitrary [`Element`](iced::Element)
//! (typically a styled button containing a row of text, an icon, and an accelerator hint), so
//! the bar composes with the rest of an iced application without a bespoke data model. Items
//! may additionally carry a [tooltip](Item::tooltip) shown while hovered.
//!
//! The widget is generic over the theme: any `Theme` that implements [`Catalog`] can be used.
//! A convenience implementation is provided for the built-in [`iced::Theme`].
//!
//! ```ignore
//! use iced::widget::button;
//! use iced_menu_bar::{Item, Menu, MenuBar};
//!
//! let menu_bar = MenuBar::new(vec![
//!     Item::with_menu(
//!         button("File"),
//!         Menu::new(vec![
//!             Item::new(button("New")),
//!             Item::new(button("Open")).tooltip(button("Open a file")),
//!         ]),
//!     ),
//! ]);
//! ```

mod common;
mod error;
mod flex;
mod menu;
mod menu_bar;
mod overlay;
mod style;

pub use common::{DrawPath, ScrollSpeed};
pub use error::{Error, Result};
pub use menu::{Item, Menu};
pub use menu_bar::MenuBar;
pub use style::{Catalog, Status, Style, StyleFn, primary};
