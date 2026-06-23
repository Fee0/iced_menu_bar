//! The [`MenuBar`] widget.
#![allow(clippy::unwrap_used)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::wildcard_imports)]
#![allow(clippy::enum_glob_use)]

use iced::advanced::layout::{Limits, Node};
use iced::advanced::widget::{Id as WidgetId, Operation, Tree, tree};
use iced::advanced::{Clipboard, Layout, Shell, Widget, overlay, renderer};
use iced::{
    Alignment, Element, Event, Length, Padding, Pixels, Rectangle, Size, Vector, keyboard, mouse,
    window,
};

use crate::common::*;
use crate::flex;
use crate::menu::*;
use crate::overlay::MenuBarOverlay;
use crate::style::*;

#[derive(Debug, Clone, Copy)]
pub(crate) enum MenuBarTask {
    OpenOnClick,
    CloseOnClick,
}

#[derive(Default, Debug)]
pub(crate) struct GlobalState {
    pub(crate) open: bool,
    pub(crate) pressed: bool,
    /// While `true`, the menu tree is being driven by the keyboard and ignores cursor-based
    /// closing (a keyboard-opened submenu must not be torn down just because the cursor is not
    /// over it). Reset as soon as the mouse moves.
    pub(crate) keyboard_nav: bool,
    /// `true` while Alt/F10 has activated the bar but no submenu is open yet. Arrow Left/Right
    /// moves the focus across roots; Arrow Down / Enter / Space opens the focused root.
    pub(crate) bar_focused: bool,
    /// Set by a widget operation ([`open_root`]) and consumed in `update()`.
    pub(crate) pending_open: Option<usize>,
    /// Set by a widget operation ([`close_menu`]) and consumed in `update()`.
    pub(crate) pending_close: bool,
    task: Option<MenuBarTask>,
}
impl GlobalState {
    pub(crate) fn schedule(&mut self, task: MenuBarTask) {
        self.task = Some(task);
    }

    pub(crate) fn task(&self) -> Option<MenuBarTask> {
        self.task
    }

    pub(crate) fn clear_task(&mut self) {
        self.task = None;
    }
}

#[derive(Default)]
pub(crate) struct MenuBarState {
    pub(crate) global_state: GlobalState,
    pub(crate) menu_state: MenuState,
}
impl MenuBarState {
    pub(crate) fn close<Message>(
        &mut self,
        item_trees: &mut [Tree],
        shell: &mut Shell<'_, Message>,
    ) {
        if self.global_state.pressed {
            return;
        }

        for item_tree in item_trees.iter_mut() {
            if item_tree.children.len() == 2 {
                let _ = item_tree.children.pop();
                shell.invalidate_layout();
            }
        }
        self.global_state.pressed = false;
        self.global_state.clear_task();
        self.global_state.open = false;
        self.global_state.keyboard_nav = false;
        self.global_state.bar_focused = false;
        self.menu_state.active = None;
        self.menu_state.keyboard_highlight = None;
        shell.request_redraw();
    }
}

/// A horizontal menu bar.
///
/// Construct it from a list of root [`Item`]s; on the built-in [`iced::Theme`] each root is
/// typically built with [`Item::root`] to attach a dropdown [`Menu`] (or [`Item::with_menu`] for a
/// hand-assembled element on a custom theme).
#[must_use]
pub struct MenuBar<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer>
where
    Theme: Catalog,
    Renderer: renderer::Renderer,
{
    pub(crate) roots: Vec<Item<'a, Message, Theme, Renderer>>,
    id: Option<WidgetId>,
    spacing: Pixels,
    padding: Padding,
    width: Length,
    height: Length,
    pub(crate) global_parameters: GlobalParameters<'a, Theme>,
}
impl<'a, Message, Theme, Renderer> MenuBar<'a, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: renderer::Renderer,
{
    /// Creates a [`MenuBar`] with the given root items.
    pub fn new(mut roots: Vec<Item<'a, Message, Theme, Renderer>>) -> Self {
        for i in &mut roots {
            if let Some(m) = i.menu.as_mut() {
                m.axis = Axis::Vertical;
            }
        }

        Self {
            roots,
            id: None,
            spacing: Pixels(4.0),
            padding: Padding {
                top: 0.0,
                right: 8.0,
                bottom: 0.0,
                left: 8.0,
            },
            width: Length::Shrink,
            height: Length::Shrink,
            global_parameters: GlobalParameters {
                safe_bounds_margin: 40.0,
                // `Fill` draws a backdrop behind the active path regardless of widget state. The
                // built-in `root`/`submenu` buttons carry no `on_press` (iced renders them as
                // `Disabled`, which never reports `Hovered`), so `Hover` would not highlight them.
                path_highlight: PathHighlight::Fill,
                close_on_item_click: true,
                close_on_background_click: false,
                open_on_hover: false,
                class: <Theme as Catalog>::default(),
            },
        }
    }

    /// Sets the unique [`MenuBarId`] of the [`MenuBar`], enabling programmatic control via
    /// [`open_root`] and [`close_menu`].
    pub fn id(mut self, id: MenuBarId) -> Self {
        self.id = Some(id.0);
        self
    }

    /// Sets the width of the [`MenuBar`].
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    /// Sets the height of the [`MenuBar`].
    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }

    /// Sets the spacing of the [`MenuBar`].
    pub fn spacing(mut self, spacing: impl Into<Pixels>) -> Self {
        self.spacing = spacing.into();
        self
    }

    /// Sets how forgiving the menus are about the cursor briefly leaving them.
    ///
    /// Each open menu keeps a rectangular safe area that extends its background by `margin`
    /// pixels; the menu stays open while the cursor is inside it and closes once the cursor
    /// leaves. Larger values are more forgiving of imprecise mouse movement.
    pub fn hover_grace(mut self, margin: f32) -> Self {
        self.global_parameters.safe_bounds_margin = margin;
        self
    }

    /// Sets how the active path — the trail of open entries — is highlighted.
    pub fn path_highlight(mut self, path_highlight: PathHighlight) -> Self {
        self.global_parameters.path_highlight = path_highlight;
        self
    }

    /// Sets when an open menu tree is dismissed (the default is [`Dismiss::OnItemClick`]).
    ///
    /// This is the bar-wide policy; individual entries can override it with
    /// [`Item::keep_open`](crate::Item::keep_open) or [`Item::close_on_click`](crate::Item::close_on_click).
    /// A click outside the menus always dismisses them regardless of this setting.
    pub fn dismiss(mut self, dismiss: Dismiss) -> Self {
        self.global_parameters.close_on_item_click = matches!(dismiss, Dismiss::OnItemClick);
        self
    }

    /// Also dismisses the menu tree when a click lands on a menu's own background (the padding
    /// around the entries) rather than on an entry. Off by default.
    ///
    /// A click fully outside the menus always dismisses them regardless of this setting; this only
    /// governs clicks inside an open menu but between its entries.
    pub fn close_on_background_click(mut self, close: bool) -> Self {
        self.global_parameters.close_on_background_click = close;
        self
    }

    /// Opens the menu tree on hover instead of requiring an initial click. Off by default.
    ///
    /// Once a menu is open, moving the cursor across root entries already switches between them
    /// regardless of this setting; this only governs the *first* open.
    pub fn open_on_hover(mut self, open_on_hover: bool) -> Self {
        self.global_parameters.open_on_hover = open_on_hover;
        self
    }

    /// Sets the padding of the [`MenuBar`].
    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.padding = padding.into();
        self
    }

    /// Sets the style of the [`MenuBar`].
    pub fn style(mut self, style: impl Fn(&Theme, Status) -> Style + 'a) -> Self
    where
        <Theme as Catalog>::Class<'a>: From<StyleFn<'a, Theme, Style>>,
    {
        self.global_parameters.class = (Box::new(style) as StyleFn<'a, Theme, Style>).into();
        self
    }

    /// Sets the class of the input of the [`MenuBar`].
    pub fn class(mut self, class: impl Into<<Theme as Catalog>::Class<'a>>) -> Self {
        self.global_parameters.class = class.into();
        self
    }
}
impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for MenuBar<'_, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: renderer::Renderer,
{
    fn size(&self) -> Size<Length> {
        Size::new(self.width, self.height)
    }

    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<MenuBarState>()
    }

    fn state(&self) -> tree::State {
        tree::State::Some(Box::<MenuBarState>::default())
    }

    fn children(&self) -> Vec<Tree> {
        self.roots.iter().map(Item::tree).collect()
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children_custom(
            &self.roots,
            |tree, item| item.diff(tree),
            |item| item.tree(),
        );
    }

    /// tree: Tree{bar, \[item_tree...]}
    ///
    /// out: Node{bar bounds , \[widget_layout, widget_layout, ...]}
    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) -> Node {
        let bar_state = tree.state.downcast_mut::<MenuBarState>();
        let bar_menu_state = &mut bar_state.menu_state;

        let items_node = flex::resolve(
            flex::Axis::Horizontal,
            renderer,
            &Limits::new(
                Size {
                    width: 0.0,
                    height: limits.min().height,
                },
                Size {
                    width: f32::INFINITY,
                    height: limits.max().height,
                },
            ),
            Length::Shrink,
            self.height,
            self.padding,
            self.spacing,
            Alignment::Center,
            &mut self
                .roots
                .iter_mut()
                .map(|item| &mut item.item)
                .collect::<Vec<_>>(),
            &mut tree
                .children
                .iter_mut()
                .map(|tree| &mut tree.children[0])
                .collect::<Vec<_>>(),
        );

        let items_node_bounds = items_node.bounds();

        let resolved_width = match self.width {
            Length::Fill | Length::FillPortion(_) => items_node_bounds
                .width
                .min(limits.max().width)
                .max(limits.min().width),
            Length::Fixed(amount) => amount.min(limits.max().width).max(limits.min().width),
            Length::Shrink => items_node_bounds.width,
        };

        let lower_bound_rel = self.padding.left - bar_menu_state.scroll_offset;
        let upper_bound_rel = lower_bound_rel + resolved_width - self.padding.x();
        let slice_width = resolved_width - self.padding.x();

        let slice =
            MenuSlice::from_bounds_rel(lower_bound_rel, upper_bound_rel, &items_node, |n| {
                n.bounds().x
            });
        bar_menu_state.slice = slice;

        let slice_node = if self.roots.is_empty() {
            // No root entries: nothing to slice, just an empty placeholder child.
            Node::new(Size::ZERO)
        } else if slice.start_index == slice.end_index {
            let node = &items_node.children()[slice.start_index];
            let bounds = node.bounds();
            let start_offset = slice.lower_bound_rel - bounds.x;
            let width = (slice.upper_bound_rel - slice.lower_bound_rel).min(slice_width);

            Node::with_children(
                Size::new(width, items_node.bounds().height),
                std::iter::once(clip_node_x(node, width, start_offset)).collect(),
            )
        } else {
            let start_node = {
                let node = &items_node.children()[slice.start_index];
                let bounds = node.bounds();
                let start_offset = slice.lower_bound_rel - bounds.x;
                let width = bounds.width - start_offset;
                clip_node_x(node, width, start_offset)
            };

            let end_node = {
                let node = &items_node.children()[slice.end_index];
                let bounds = node.bounds();
                let width = (slice.upper_bound_rel - bounds.x).min(slice_width);
                clip_node_x(node, width, 0.0)
            };

            Node::with_children(
                items_node_bounds.size(),
                std::iter::once(start_node)
                    .chain(
                        items_node.children()[slice.start_index + 1..slice.end_index]
                            .iter()
                            .map(Clone::clone),
                    )
                    .chain(std::iter::once(end_node))
                    .collect(),
            )
        };

        let children = vec![slice_node.translate([bar_menu_state.scroll_offset, 0.0])];
        Node::with_children(
            Size {
                width: resolved_width,
                height: items_node_bounds.height,
            },
            children,
        )
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let slice_layout = layout.children().next().unwrap();

        let Tree {
            state,
            children: item_trees,
            ..
        } = tree;
        let bar_state = state.downcast_mut::<MenuBarState>();
        let global_state = &mut bar_state.global_state;
        let bar_menu_state = &mut bar_state.menu_state;

        // Consume any pending operations set by operate() (open_root / close_menu).
        if let Some(idx) = global_state.pending_open.take() {
            if idx < self.roots.len() && self.roots[idx].menu.is_some() {
                global_state.open = true;
                global_state.keyboard_nav = false;
                global_state.bar_focused = false;
                bar_menu_state.active = None;
                let item = &self.roots[idx];
                bar_menu_state.open_new_menu(idx, item, &mut item_trees[idx]);
                shell.invalidate_layout();
                shell.request_redraw();
            }
        } else if global_state.pending_close {
            global_state.pending_close = false;
            for item_tree in item_trees.iter_mut() {
                if item_tree.children.len() == 2 {
                    let _ = item_tree.children.pop();
                    shell.invalidate_layout();
                }
            }
            global_state.open = false;
            global_state.keyboard_nav = false;
            global_state.bar_focused = false;
            bar_menu_state.active = None;
            bar_menu_state.keyboard_highlight = None;
            shell.request_redraw();
        }

        let slice = bar_menu_state.slice;
        let bar_is_open = global_state.open;
        let bar_active = bar_menu_state.active;
        let bar_focused = global_state.bar_focused;
        let keyboard_highlight = bar_menu_state.keyboard_highlight;
        itl_iter_slice_enum!(
            slice,
            self.roots;iter_mut,
            item_trees;iter_mut,
            slice_layout.children()
        )
        .for_each(|(i, ((item, tree), layout))| {
            let item_cursor =
                if matches!(&self.global_parameters.path_highlight, PathHighlight::Fill)
                    && ((bar_is_open && bar_active == Some(i))
                        || (bar_focused && keyboard_highlight == Some(i)))
                {
                    mouse::Cursor::Available(layout.bounds().center())
                } else {
                    cursor
                };
            item.update(
                tree,
                event,
                layout,
                item_cursor,
                renderer,
                clipboard,
                shell,
                viewport,
            );
        });

        let bar_bounds = layout.bounds();

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
                if cursor.is_over(bar_bounds) =>
            {
                global_state.pressed = true;
                if global_state.open {
                    schedule_close_on_click(
                        global_state,
                        &self.global_parameters,
                        slice,
                        &mut self.roots,
                        slice_layout.children(),
                        cursor,
                    );
                } else {
                    global_state.schedule(MenuBarTask::OpenOnClick);
                }
                shell.capture_event();
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                global_state.pressed = false;

                if let Some(task) = global_state.task() {
                    match task {
                        MenuBarTask::OpenOnClick => {
                            if !global_state.open {
                                global_state.open = true;
                                bar_menu_state.active = None;
                            }
                            try_open_menu(
                                &mut self.roots,
                                bar_menu_state,
                                item_trees,
                                slice_layout.children(),
                                cursor,
                                shell,
                            );
                            global_state.clear_task();
                        }
                        MenuBarTask::CloseOnClick => {
                            if !global_state.pressed {
                                for item_tree in item_trees.iter_mut() {
                                    if item_tree.children.len() == 2 {
                                        let _ = item_tree.children.pop();
                                        shell.invalidate_layout();
                                    }
                                }
                                global_state.clear_task();
                                global_state.open = false;
                                global_state.keyboard_nav = false;
                                global_state.bar_focused = false;
                                bar_menu_state.active = None;
                                bar_menu_state.keyboard_highlight = None;
                                shell.request_redraw();
                            }
                        }
                    }
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) if global_state.open => {
                if cursor.is_over(bar_bounds) {
                    try_open_menu(
                        &mut self.roots,
                        bar_menu_state,
                        item_trees,
                        slice_layout.children(),
                        cursor,
                        shell,
                    );
                    shell.capture_event();
                } else if !global_state.pressed {
                    for item_tree in item_trees.iter_mut() {
                        if item_tree.children.len() == 2 {
                            let _ = item_tree.children.pop();
                            shell.invalidate_layout();
                        }
                    }
                    global_state.clear_task();
                    global_state.open = false;
                    global_state.keyboard_nav = false;
                    global_state.bar_focused = false;
                    bar_menu_state.active = None;
                    bar_menu_state.keyboard_highlight = None;
                    shell.request_redraw();
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { .. })
                if self.global_parameters.open_on_hover && cursor.is_over(bar_bounds) =>
            {
                if !global_state.open {
                    global_state.open = true;
                    bar_menu_state.active = None;
                }
                try_open_menu(
                    &mut self.roots,
                    bar_menu_state,
                    item_trees,
                    slice_layout.children(),
                    cursor,
                    shell,
                );
                global_state.clear_task();
                shell.capture_event();
            }
            // Request a redraw so the hover highlight updates even when the menu is closed and
            // open_on_hover is off (iced skips re-rendering if no widget captures the event).
            Event::Mouse(mouse::Event::CursorMoved { .. }) if !global_state.open => {
                shell.request_redraw();
            }
            Event::Mouse(mouse::Event::WheelScrolled { delta })
                if cursor.is_over(bar_bounds)
                    && slice_layout.bounds().width > layout.bounds().width =>
            {
                let delta_x = match delta {
                    mouse::ScrollDelta::Lines { x, .. } => x * SCROLL_SPEED_LINE,
                    mouse::ScrollDelta::Pixels { x, .. } => x * SCROLL_SPEED_PIXEL,
                };

                let min_offset = -(slice_layout.bounds().width - layout.bounds().width);

                bar_menu_state.scroll_offset =
                    (bar_menu_state.scroll_offset + delta_x).clamp(min_offset, 0.0);
                shell.invalidate_layout();
                shell.request_redraw();
                shell.capture_event();
            }
            Event::Window(window::Event::Resized { .. }) => {
                if slice_layout.bounds().width > layout.bounds().width {
                    let min_offset = -(slice_layout.bounds().width - layout.bounds().width);

                    bar_menu_state.scroll_offset =
                        bar_menu_state.scroll_offset.clamp(min_offset, 0.0);
                }
                shell.invalidate_layout();
                shell.request_redraw();
            }
            // Alt / F10 activates the bar so it can be navigated without a mouse.
            Event::Keyboard(keyboard::Event::KeyPressed { key, .. }) if !global_state.open => {
                use keyboard::key::Named;
                match key {
                    keyboard::Key::Named(Named::Alt | Named::F10) => {
                        if global_state.bar_focused {
                            global_state.bar_focused = false;
                            global_state.keyboard_nav = false;
                            bar_menu_state.keyboard_highlight = None;
                        } else {
                            global_state.bar_focused = true;
                            global_state.keyboard_nav = true;
                            bar_menu_state.keyboard_highlight =
                                self.roots.iter().position(|r| r.navigable);
                        }
                        shell.capture_event();
                        shell.request_redraw();
                    }
                    keyboard::Key::Named(Named::Escape) if global_state.bar_focused => {
                        global_state.bar_focused = false;
                        global_state.keyboard_nav = false;
                        bar_menu_state.keyboard_highlight = None;
                        shell.capture_event();
                        shell.request_redraw();
                    }
                    keyboard::Key::Named(Named::ArrowLeft) if global_state.bar_focused => {
                        let current = bar_menu_state.keyboard_highlight.unwrap_or(0);
                        bar_menu_state.keyboard_highlight =
                            prev_navigable_root(&self.roots, current);
                        shell.capture_event();
                        shell.request_redraw();
                    }
                    keyboard::Key::Named(Named::ArrowRight) if global_state.bar_focused => {
                        let n = self.roots.len();
                        let current = bar_menu_state.keyboard_highlight.unwrap_or(n - 1);
                        bar_menu_state.keyboard_highlight =
                            next_navigable_root(&self.roots, current);
                        shell.capture_event();
                        shell.request_redraw();
                    }
                    keyboard::Key::Named(Named::ArrowDown | Named::Enter | Named::Space)
                        if global_state.bar_focused =>
                    {
                        if let Some(idx) = bar_menu_state.keyboard_highlight {
                            let item = &self.roots[idx];
                            if item.menu.is_some() {
                                global_state.open = true;
                                global_state.keyboard_nav = true;
                                global_state.bar_focused = false;
                                bar_menu_state.open_new_menu(idx, item, &mut item_trees[idx]);
                                shell.invalidate_layout();
                                shell.request_redraw();
                            }
                        }
                        shell.capture_event();
                    }
                    _ => {}
                }
            }
            // Mouse movement while bar_focused exits bar-focus mode so the user can switch back
            // to pointer-driven navigation without pressing Escape first.
            Event::Mouse(mouse::Event::CursorMoved { .. }) if global_state.bar_focused => {
                global_state.bar_focused = false;
                global_state.keyboard_nav = false;
                bar_menu_state.keyboard_highlight = None;
                shell.request_redraw();
            }
            _ => {}
        }
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation<()>,
    ) {
        let slice_layout = layout.children().next().unwrap();

        let bar_state = tree.state.downcast_mut::<MenuBarState>();

        // Let custom operations (open_root / close_menu) find and modify our state by id.
        operation.custom(self.id.as_ref(), layout.bounds(), bar_state);

        let MenuBarState {
            menu_state: bar_menu_state,
            ..
        } = tree.state.downcast_ref::<MenuBarState>();

        let slice = bar_menu_state.slice;

        operation.container(None, layout.bounds());
        operation.traverse(&mut |operation| {
            itl_iter_slice!(slice, self.roots;iter_mut, tree.children;iter_mut, slice_layout.children())
                .for_each(|((child, state), layout)| {
                    child.operate(state, layout, renderer, operation);
                });
        });
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let slice_layout = layout.children().next().unwrap();

        let MenuBarState {
            menu_state: bar_menu_state,
            ..
        } = tree.state.downcast_ref::<MenuBarState>();

        itl_iter_slice!(bar_menu_state.slice, self.roots;iter, tree.children;iter, slice_layout.children())
            .map(|((item, tree), layout)| item.mouse_interaction(tree, layout, cursor, renderer))
            .max()
            .unwrap_or_default()
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let slice_layout = layout.children().next().unwrap();

        let MenuBarState {
            global_state,
            menu_state: bar_menu_state,
            ..
        } = tree.state.downcast_ref::<MenuBarState>();

        let slice = bar_menu_state.slice;

        let status = if global_state.keyboard_nav {
            Status::Focused
        } else if global_state.open {
            Status::Selected
        } else if global_state.pressed {
            Status::Pressed
        } else if cursor.is_over(layout.bounds()) {
            Status::Hovered
        } else {
            Status::Active
        };

        let styling = <Theme as Catalog>::style(theme, &self.global_parameters.class, status);
        renderer.fill_quad(
            renderer::Quad {
                bounds: layout.bounds(),
                border: styling.bar_border,
                shadow: styling.bar_shadow,
                ..Default::default()
            },
            styling.bar_background,
        );

        if matches!(&self.global_parameters.path_highlight, PathHighlight::Fill) {
            // When open, highlight the active (open) root. When bar_focused (Alt mode), highlight
            // the keyboard-focused root. Otherwise highlight whichever root the cursor is over.
            let highlight_bounds = if global_state.open {
                bar_menu_state.active.and_then(|active| {
                    let active_in_slice = active - slice.start_index;
                    slice_layout
                        .children()
                        .nth(active_in_slice)
                        .map(|l| l.bounds())
                })
            } else if global_state.bar_focused {
                bar_menu_state.keyboard_highlight.and_then(|highlighted| {
                    let in_slice = highlighted.saturating_sub(slice.start_index);
                    slice_layout.children().nth(in_slice).map(|l| l.bounds())
                })
            } else {
                slice_layout
                    .children()
                    .find(|l| cursor.is_over(l.bounds()))
                    .map(|l| l.bounds())
            };

            if let Some(bounds) = highlight_bounds {
                renderer.fill_quad(
                    renderer::Quad {
                        bounds,
                        border: styling.path_border,
                        ..Default::default()
                    },
                    styling.path,
                );
            }
        }

        let draw_bar_is_open = global_state.open;
        let draw_bar_focused = global_state.bar_focused;
        let draw_bar_active = bar_menu_state.active;
        let draw_keyboard_highlight = bar_menu_state.keyboard_highlight;
        renderer.with_layer(
            Rectangle {
                x: layout.bounds().x + self.padding.left,
                y: layout.bounds().y + self.padding.top,
                width: layout.bounds().width - self.padding.x(),
                height: layout.bounds().height - self.padding.y(),
            },
            |r| {
                itl_iter_slice_enum!(slice, self.roots;iter, tree.children;iter, slice_layout.children())
                .for_each(|(i, ((item, child_tree), layout))| {
                    let item_cursor = if (draw_bar_focused && draw_keyboard_highlight == Some(i))
                        || (draw_bar_is_open && draw_bar_active == Some(i))
                    {
                        mouse::Cursor::Available(layout.bounds().center())
                    } else {
                        cursor
                    };
                    item.draw(child_tree, r, theme, style, layout, item_cursor, viewport);
                });
            },
        );
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        let is_open = tree.state.downcast_ref::<MenuBarState>().global_state.open;

        if is_open {
            Some(
                MenuBarOverlay {
                    menu_bar: self,
                    layout,
                    translation,
                    tree,
                }
                .overlay_element(),
            )
        } else {
            let slice_layout = layout.children().next()?;

            let Tree {
                state,
                children: item_trees,
                ..
            } = tree;
            let bar = state.downcast_mut::<MenuBarState>();
            let MenuBarState {
                menu_state: bar_menu_state,
                ..
            } = bar;

            let slice = bar_menu_state.slice;

            let overlays = itl_iter_slice!(slice, self.roots;iter_mut, item_trees;iter_mut, slice_layout.children())
                .filter_map(|((item, item_tree), item_layout)| {
                    item.item.as_widget_mut().overlay(
                        &mut item_tree.children[0],
                        item_layout,
                        renderer,
                        viewport,
                        translation,
                    )
                })
                .collect::<Vec<_>>();

            if overlays.is_empty() {
                None
            } else {
                Some(overlay::Group::with_children(overlays).overlay())
            }
        }
    }
}

/// A unique identifier for a [`MenuBar`], used with [`open_root`] and [`close_menu`].
///
/// ```ignore
/// let id = MenuBarId::unique();
/// let bar = MenuBar::new(roots).id(id.clone());
/// // In your update:
/// Task::batch([iced_menu_bar::open_root(id, 0)])
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MenuBarId(WidgetId);

impl MenuBarId {
    /// Creates a named [`MenuBarId`].
    pub fn new(s: &'static str) -> Self {
        Self(WidgetId::new(s))
    }

    /// Creates a unique [`MenuBarId`].
    pub fn unique() -> Self {
        Self(WidgetId::unique())
    }
}

/// Opens the root at `index` in the [`MenuBar`] with the given `id`.
///
/// Returns a [`Task`](iced::Task) that can be returned from your `update` function.
/// The menu opens on the next event cycle.
pub fn open_root<Message>(id: MenuBarId, index: usize) -> iced::Task<Message>
where
    Message: Send + 'static,
{
    use std::any::Any;

    struct OpenRoot {
        target: WidgetId,
        index: usize,
    }
    impl<T> Operation<T> for OpenRoot {
        fn custom(&mut self, id: Option<&WidgetId>, _bounds: Rectangle, state: &mut dyn Any) {
            if id == Some(&self.target)
                && let Some(bar) = state.downcast_mut::<MenuBarState>()
            {
                bar.global_state.pending_open = Some(self.index);
            }
        }
        fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<T>)) {
            operate(self);
        }
    }

    iced::advanced::widget::operate(OpenRoot {
        target: id.0,
        index,
    })
}

/// Closes the open menu tree in the [`MenuBar`] with the given `id`, if any.
///
/// Returns a [`Task`](iced::Task) that can be returned from your `update` function.
pub fn close_menu<Message>(id: MenuBarId) -> iced::Task<Message>
where
    Message: Send + 'static,
{
    use std::any::Any;

    struct CloseMenu {
        target: WidgetId,
    }
    impl<T> Operation<T> for CloseMenu {
        fn custom(&mut self, id: Option<&WidgetId>, _bounds: Rectangle, state: &mut dyn Any) {
            if id == Some(&self.target)
                && let Some(bar) = state.downcast_mut::<MenuBarState>()
            {
                bar.global_state.pending_close = true;
            }
        }
        fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<T>)) {
            operate(self);
        }
    }

    iced::advanced::widget::operate(CloseMenu { target: id.0 })
}

/// Finds the previous navigable root index, wrapping around.
fn prev_navigable_root<'a, Message, Theme: Catalog, Renderer: renderer::Renderer>(
    roots: &[Item<'a, Message, Theme, Renderer>],
    current: usize,
) -> Option<usize> {
    let n = roots.len();
    for delta in 1..=n {
        let idx = (current + n - delta) % n;
        if roots[idx].navigable {
            return Some(idx);
        }
    }
    None
}

/// Finds the next navigable root index, wrapping around.
fn next_navigable_root<'a, Message, Theme: Catalog, Renderer: renderer::Renderer>(
    roots: &[Item<'a, Message, Theme, Renderer>],
    current: usize,
) -> Option<usize> {
    let n = roots.len();
    for delta in 1..=n {
        let idx = (current + delta) % n;
        if roots[idx].navigable {
            return Some(idx);
        }
    }
    None
}

impl<'a, Message, Theme, Renderer> From<MenuBar<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a + Catalog,
    Renderer: 'a + renderer::Renderer,
{
    fn from(value: MenuBar<'a, Message, Theme, Renderer>) -> Self {
        Self::new(value)
    }
}
