//! Error and [`Result`] types for the public API of this crate.

/// Errors that can occur while constructing a menu bar or one of its menus.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A [`Menu`](crate::Menu) or [`MenuBar`](crate::MenuBar) was created without any items.
    #[error("a menu must contain at least one item")]
    EmptyMenu,
}

/// The result type returned by the fallible constructors in this crate.
pub type Result<T> = std::result::Result<T, Error>;
