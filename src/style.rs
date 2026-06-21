//! Theming for the menu bar and its menus.
//!
//! The widget is generic over any `Theme` that implements [`Catalog`]. A convenience
//! implementation is provided for the built-in [`iced::Theme`] via [`primary`].

use iced::{Background, Border, Color, Shadow, Theme, Vector};

/// The status of a menu bar / menu entry, passed to the style function.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Can be pressed.
    Active,
    /// Can be pressed and is being hovered.
    Hovered,
    /// Is being pressed.
    Pressed,
    /// Cannot be pressed.
    Disabled,
    /// Is focused.
    Focused,
    /// Is selected.
    Selected,
}

/// A boxed style function: maps a `Theme` and [`Status`] to a [`Style`].
pub type StyleFn<'a, Theme, Style> = Box<dyn Fn(&Theme, Status) -> Style + 'a>;

/// The appearance of a menu bar and its menus.
#[derive(Debug, Clone, Copy)]
pub struct Style {
    /// The background of the menu bar.
    pub bar_background: Background,
    /// The border of the menu bar.
    pub bar_border: Border,
    /// The shadow of the menu bar.
    pub bar_shadow: Shadow,

    /// The background of the menus.
    pub menu_background: Background,
    /// The border of the menus.
    pub menu_border: Border,
    /// The shadow of the menus.
    pub menu_shadow: Shadow,

    /// The background of the active-path highlight.
    pub path: Background,
    /// The border of the active-path highlight.
    pub path_border: Border,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            bar_background: Color::from([0.85; 3]).into(),
            bar_border: Border {
                radius: 8.0.into(),
                ..Default::default()
            },
            bar_shadow: Shadow::default(),

            menu_background: Color::from([0.85; 3]).into(),
            menu_border: Border {
                radius: 8.0.into(),
                ..Default::default()
            },
            menu_shadow: Shadow {
                color: Color::from([0.0, 0.0, 0.0, 0.5]),
                offset: Vector::ZERO,
                blur_radius: 10.0,
            },
            path: Color::from([0.3; 3]).into(),
            path_border: Border {
                radius: 6.0.into(),
                ..Default::default()
            },
        }
    }
}

/// The theme catalog of a menu bar.
///
/// Implement this for your own theme type to use it with [`MenuBar`](crate::MenuBar).
pub trait Catalog {
    /// The style class used by this catalog.
    type Class<'a>;

    /// The default class produced by the [`Catalog`].
    fn default<'a>() -> Self::Class<'a>;

    /// Resolves the [`Style`] of a class with the given [`Status`].
    fn style(&self, class: &Self::Class<'_>, status: Status) -> Style;
}

impl Catalog for Theme {
    type Class<'a> = StyleFn<'a, Self, Style>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(primary)
    }

    fn style(&self, class: &Self::Class<'_>, status: Status) -> Style {
        class(self, status)
    }
}

/// The default style of a menu bar for the built-in [`iced::Theme`].
#[must_use]
pub fn primary(theme: &Theme, _status: Status) -> Style {
    let palette = theme.extended_palette();

    Style {
        bar_background: palette.background.base.color.into(),
        menu_background: palette.background.base.color.into(),
        path: palette.primary.weak.color.into(),
        ..Default::default()
    }
}
