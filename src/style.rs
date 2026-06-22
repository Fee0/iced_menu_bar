//! Theming for the menu bar and its menus.
//!
//! The widget is generic over any `Theme` that implements [`Catalog`]. A convenience
//! implementation is provided for the built-in [`iced::Theme`] via [`default_style`].

use iced::widget::button;
use iced::{Background, Border, Color, Shadow, Theme, Vector};

/// A boxed style function: maps a `Theme` to a [`Style`].
///
/// The bar and its menus are *chrome* with a single visual state, so the function takes only the
/// theme. Per-entry interactive states (hover/press) are handled separately by the entries' own
/// button styles (see [`menu_item_style`]).
pub type StyleFn<'a, Theme, Style> = Box<dyn Fn(&Theme) -> Style + 'a>;

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

    /// The background of the highlight drawn behind the active/focused entry (for both mouse and
    /// keyboard).
    pub path: Background,
    /// The border of the highlight.
    pub path_border: Border,

    /// The color of the hairline drawn by [`separator`](crate::separator).
    pub separator: Color,
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
            separator: Color::from([0.5; 3]),
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

    /// Resolves the [`Style`] of a class.
    fn style(&self, class: &Self::Class<'_>) -> Style;
}

impl Catalog for Theme {
    type Class<'a> = StyleFn<'a, Self, Style>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(default_style)
    }

    fn style(&self, class: &Self::Class<'_>) -> Style {
        class(self)
    }
}

/// The default style of a menu bar for the built-in [`iced::Theme`].
///
/// A transparent bar over a slightly elevated flyout with a subtle hairline border and a
/// soft drop shadow — derived from the theme palette so it adapts to light and dark themes.
#[must_use]
pub fn default_style(theme: &Theme) -> Style {
    let palette = theme.extended_palette();

    Style {
        bar_background: Background::Color(Color::TRANSPARENT),
        bar_border: Border::default(),
        bar_shadow: Shadow::default(),
        menu_background: palette.background.weak.color.into(),
        menu_border: Border {
            color: palette.background.strong.color,
            width: 1.0,
            radius: 4.0.into(),
        },
        menu_shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.28),
            offset: Vector::new(0.0, 4.0),
            blur_radius: 8.0,
        },
        path: Background::Color(palette.primary.weak.color),
        path_border: Border::default(),
        separator: palette.background.strong.color,
    }
}

/// The default styling for a menu row or root button on the built-in [`iced::Theme`].
///
/// Transparent by default with an accent fill on hover/press, so the active path reads
/// clearly against the flyout drawn by [`default_style`]. Apply it to the [`button`]s wrapped by
/// your [`Item`](crate::Item)s via [`button::Button::style`] to get the crate's baseline look.
#[must_use]
pub fn menu_item_style(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.extended_palette();

    let base = button::Style {
        background: Some(Background::Color(Color::TRANSPARENT)),
        text_color: palette.background.base.text,
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 0.0.into(),
        },
        shadow: Shadow::default(),
        ..button::Style::default()
    };

    match status {
        // `Disabled` renders like `Active`: root and submenu rows carry no `on_press` (the
        // menu bar drives opening), so iced reports them as disabled even though they are
        // fully interactive — they must not look greyed out.
        button::Status::Active | button::Status::Disabled => base,
        button::Status::Hovered => button::Style {
            background: Some(palette.primary.base.color.into()),
            text_color: palette.primary.base.text,
            ..base
        },
        button::Status::Pressed => button::Style {
            background: Some(palette.primary.strong.color.into()),
            text_color: palette.primary.strong.text,
            ..base
        },
    }
}

/// Default opacity applied to a disabled entry's text, so it reads clearly greyed out.
pub const DISABLED_ALPHA: f32 = 0.4;

/// The styling for a disabled menu row on the built-in [`iced::Theme`].
///
/// Transparent and dimmed in every state — no hover or press reaction — so the row reads as
/// non-interactive. Used by the [`action`](crate::Item::action) builder when
/// [`disabled`](crate::ActionBuilder::disabled) is set; unlike [`menu_item_style`] it must not
/// rely on `button::Status::Disabled` (which that style deliberately draws like `Active`).
#[must_use]
pub fn menu_item_disabled_style(theme: &Theme, status: button::Status) -> button::Style {
    menu_item_disabled_style_with(theme, status, DISABLED_ALPHA)
}

/// Like [`menu_item_disabled_style`], but with a caller-chosen text opacity.
///
/// Used by [`ActionBuilder::disabled_alpha`](crate::ActionBuilder::disabled_alpha) to tune how
/// strongly a disabled row is greyed out.
#[must_use]
pub fn menu_item_disabled_style_with(
    theme: &Theme,
    _status: button::Status,
    alpha: f32,
) -> button::Style {
    let palette = theme.extended_palette();

    button::Style {
        background: Some(Background::Color(Color::TRANSPARENT)),
        text_color: Color {
            a: alpha,
            ..palette.background.base.text
        },
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 0.0.into(),
        },
        shadow: Shadow::default(),
        ..button::Style::default()
    }
}
