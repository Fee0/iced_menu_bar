//! [`Item`] and [`Menu`] — the element-based building blocks of the menu tree.
#![allow(clippy::unwrap_used)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::wildcard_imports)]
#![allow(clippy::enum_glob_use)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::unused_self)]
#![allow(clippy::return_self_not_must_use)]
#![allow(clippy::pedantic)]
#![allow(clippy::similar_names)]
#![allow(clippy::if_same_then_else)]
#![allow(clippy::collapsible_if)]

use iced::advanced::layout::{Layout, Limits, Node};
use iced::advanced::widget::Operation;
use iced::advanced::widget::tree::{self, Tree};
use iced::advanced::{Clipboard, Shell, renderer};
use iced::time::Instant;
use iced::{
    Alignment, Element, Event, Length, Padding, Pixels, Point, Rectangle, Size, Vector, mouse,
    window,
};
use std::iter::once;

use crate::common::*;
use crate::flex;
use crate::menu_bar::*;
use crate::style::*;

/*
menu tree:
Item{
    widget
    Menu [
        Item{...}
        Item{...}
        Item{...}
        ...
    ]
}

state tree:
Tree{
    item state
    [
        Tree{widget state}
        Tree{
            menu state
            [
                Tree{item state [...]}
                Tree{item state [...]}
                Tree{item state [...]}
                ...
            ]
        }
    ]
}

*/

#[derive(Debug)]
pub(crate) struct MenuState {
    pub(crate) scroll_offset: f32,
    pub(crate) active: Index,
    pub(crate) slice: MenuSlice,
    pub(crate) safe_triangle: Option<SafeTriangle>,
    pub(crate) last_cursor_on_parent: Option<Point>,
}
impl MenuState {
    /// item_tree: Tree{item state, [Tree{widget state}, Tree{menu state, [...]}]}
    pub(crate) fn open_new_menu<'a, Message, Theme: Catalog, Renderer: renderer::Renderer>(
        &mut self,
        active_index: usize,
        item: &Item<'a, Message, Theme, Renderer>,
        item_tree: &mut Tree,
    ) {
        let Some(menu) = item.menu.as_ref() else {
            return;
        };

        // An empty menu has nothing to show and its slice/index logic would panic, so leave the
        // path closed: the entry simply opens to nothing.
        if menu.items.is_empty() {
            return;
        }

        self.active = Some(active_index);

        // build the state tree for the new menu
        let menu_tree = menu.tree();

        if item_tree.children.len() == 1 {
            item_tree.children.push(menu_tree);
        } else {
            item_tree.children[1] = menu_tree;
        }
    }
}
impl Default for MenuState {
    fn default() -> Self {
        Self {
            scroll_offset: 0.0,
            active: None,
            slice: MenuSlice {
                start_index: 0,
                end_index: usize::MAX - 1,
                lower_bound_rel: 0.0,
                upper_bound_rel: f32::MAX,
            },
            safe_triangle: None,
            last_cursor_on_parent: None,
        }
    }
}

/// A menu — a vertical list of [`Item`]s shown in an overlay.
#[must_use]
pub struct Menu<'a, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: renderer::Renderer,
{
    pub(crate) items: Vec<Item<'a, Message, Theme, Renderer>>,
    pub(crate) spacing: Pixels,
    pub(crate) max_width: f32,
    pub(crate) width: Length,
    pub(crate) height: Length,
    pub(crate) axis: Axis,
    pub(crate) offset: f32,
    pub(crate) padding: Padding,
}
impl<'a, Message, Theme, Renderer> Menu<'a, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: renderer::Renderer,
{
    /// Creates a [`Menu`] with the given items.
    pub fn new(items: Vec<Item<'a, Message, Theme, Renderer>>) -> Self {
        Self {
            items,
            spacing: Pixels::ZERO,
            max_width: 280.0,
            width: Length::Fill,
            height: Length::Shrink,
            axis: Axis::Horizontal,
            offset: 0.0,
            padding: Padding::new(4.0),
        }
    }

    /// Sets the maximum width of the [`Menu`].
    pub fn max_width(mut self, max_width: f32) -> Self {
        self.max_width = max_width;
        self
    }

    /// Sets the width of the [`Menu`].
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    /// Sets the spacing of the [`Menu`].
    pub fn spacing(mut self, spacing: impl Into<Pixels>) -> Self {
        self.spacing = spacing.into();
        self
    }

    /// The offset from the menu's parent item.
    pub fn offset(mut self, offset: f32) -> Self {
        self.offset = offset;
        self
    }

    /// Sets the padding of the [`Menu`].
    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.padding = padding.into();
        self
    }

    /// Rebuild state tree
    pub(crate) fn tree(&self) -> Tree {
        Tree {
            tag: self.tag(),
            state: self.state(),
            children: self.children(),
        }
    }
}
impl<Message, Theme, Renderer> Menu<'_, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: renderer::Renderer,
{
    pub(crate) fn tag(&self) -> tree::Tag {
        tree::Tag::of::<MenuState>()
    }

    pub(crate) fn state(&self) -> tree::State {
        tree::State::Some(Box::<MenuState>::default())
    }

    /// out: \[item_tree...]
    pub(crate) fn children(&self) -> Vec<Tree> {
        self.items.iter().map(Item::tree).collect()
    }

    /// tree: Tree{menu_state, \[item_tree...]}
    pub(crate) fn diff(&self, tree: &mut Tree) {
        tree.diff_children_custom(&self.items, |tree, item| item.diff(tree), Item::tree);
    }

    /// tree: Tree{ menu_state, \[item_tree...] }
    ///
    /// out: Node{inf, \[ slice_node, items_bounds, offset_bounds]}
    pub(crate) fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &Limits,
        parent_bounds: Rectangle,
        parent_direction: (Direction, Direction),
        viewport: &Rectangle,
    ) -> (Node, (Direction, Direction)) {
        let limits = limits
            .max_width(self.max_width)
            .max_width(self.compute_max_available_width(parent_bounds, viewport));

        let items_node = flex::resolve(
            flex::Axis::Vertical,
            renderer,
            &limits,
            self.width,
            self.height,
            Padding::ZERO,
            self.spacing,
            // Left-align rows: full-width leaves fill the row, while content-sized rows (e.g.
            // submenu triggers) align to the start instead of being centered.
            Alignment::Start,
            &mut self
                .items
                .iter_mut()
                .map(|i| &mut i.item)
                .collect::<Vec<_>>(),
            &mut tree
                .children
                .iter_mut()
                .map(|t| &mut t.children[0])
                .collect::<Vec<_>>(),
        );

        let aod = Aod::new(
            self.axis,
            viewport.size(),
            parent_bounds,
            parent_direction,
            self.offset,
        );

        let children_size = items_node.bounds().size();
        let (children_position, offset_position, child_direction) =
            aod.resolve(parent_bounds, children_size, viewport.size());

        // calc auxiliary bounds
        let delta = children_position - offset_position;
        let offset_size = if delta.x.abs() > delta.y.abs() {
            Size::new(self.offset, children_size.height)
        } else {
            Size::new(children_size.width, self.offset)
        };

        let offset_bounds = Rectangle::new(offset_position, offset_size);

        let menu_state = tree.state.downcast_mut::<MenuState>();

        // calc slice
        let (lower_bound_rel, upper_bound_rel) = cal_bounds_rel_menu(
            &items_node,
            children_position - Point::ORIGIN,
            viewport.size(),
            menu_state.scroll_offset,
        );
        let slice =
            MenuSlice::from_bounds_rel(lower_bound_rel, upper_bound_rel, &items_node, |n| {
                n.bounds().y
            });
        menu_state.slice = slice;

        let slice_node = if slice.start_index == slice.end_index {
            let node = &items_node.children()[slice.start_index];
            let bounds = node.bounds();
            let start_offset = slice.lower_bound_rel - bounds.y;
            let height = slice.upper_bound_rel - slice.lower_bound_rel;

            Node::with_children(
                Size::new(items_node.bounds().width, height),
                once(clip_node_y(node, height, start_offset)).collect(),
            )
        } else {
            let start_node = {
                let node = &items_node.children()[slice.start_index];
                let bounds = node.bounds();
                let start_offset = slice.lower_bound_rel - bounds.y;
                let height = bounds.height - start_offset;
                clip_node_y(node, height, start_offset)
            };

            let end_node = {
                let node = &items_node.children()[slice.end_index];
                let bounds = node.bounds();
                let height = slice.upper_bound_rel - bounds.y;
                clip_node_y(node, height, 0.0)
            };

            Node::with_children(
                Size::new(
                    items_node.bounds().width,
                    slice.upper_bound_rel - slice.lower_bound_rel,
                ),
                once(start_node)
                    .chain(
                        items_node.children()[slice.start_index + 1..slice.end_index]
                            .iter()
                            .map(Clone::clone),
                    )
                    .chain(once(end_node))
                    .collect(),
            )
        };

        (
            Node::with_children(
                Size::INFINITE,
                [
                    slice_node
                        .move_to(children_position)
                        .translate([0.0, menu_state.scroll_offset]), // slice_layout
                    Node::new(children_size).move_to(children_position), // items_bounds
                    Node::new(offset_bounds.size()).move_to(offset_bounds.position()), // offset_bounds
                ]
                .into(),
            ),
            child_direction,
        )
    }

    fn compute_max_available_width(&self, parent_bounds: Rectangle, viewport: &Rectangle) -> f32 {
        match self.axis {
            Axis::Horizontal => {
                let left = parent_bounds.x - (viewport.x + self.offset);
                let right = viewport.x + viewport.width
                    - (parent_bounds.x + parent_bounds.width + self.offset);
                left.max(right)
            }
            Axis::Vertical => {
                let left = parent_bounds.x + parent_bounds.width - viewport.x;
                let right = viewport.x + viewport.width - parent_bounds.x;
                left.max(right)
            }
        }
        .max(0.0)
    }

    /// tree: Tree{ menu_state, \[item_tree...] }
    ///
    /// layout: Node{inf, \[ slice_node, items_bounds, offset_bounds]}
    pub(crate) fn update(
        &mut self,
        global_state: &mut GlobalState,
        global_parameters: &GlobalParameters<'_, Theme>,
        rec_event: RecEvent,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
        parent_bounds: Rectangle,
        prev_bounds_list: &[Rectangle],
        prev_active: &mut Index,
    ) -> RecEvent {
        let mut lc = layout.children();
        let slice_layout = lc.next().unwrap();
        let items_bounds = lc.next().unwrap().bounds();
        let offset_bounds = lc.next().unwrap().bounds();
        let background_bounds = pad_rectangle(items_bounds, self.padding);
        let safe_bounds = pad_rectangle(
            background_bounds,
            Padding::new(global_parameters.safe_bounds_margin),
        );

        {
            let menu_state = tree.state.downcast_mut::<MenuState>();
            let parent_direction = {
                let hcenter = viewport.width / 2.0;
                let vcenter = viewport.height / 2.0;
                let phcenter = parent_bounds.x + parent_bounds.width / 2.0;
                let pvcenter = parent_bounds.y + parent_bounds.height / 2.0;
                (
                    if phcenter < hcenter {
                        Direction::Positive
                    } else {
                        Direction::Negative
                    },
                    if pvcenter < vcenter {
                        Direction::Positive
                    } else {
                        Direction::Negative
                    },
                )
            };

            if cursor.is_over(parent_bounds) {
                if let Some(pos) = cursor.position() {
                    menu_state.last_cursor_on_parent = Some(pos);
                }
            }

            let p1 = menu_state
                .last_cursor_on_parent
                .unwrap_or_else(|| parent_bounds.center());

            let triangle = SafeTriangle::new(p1, background_bounds, parent_direction);

            menu_state.safe_triangle = Some(triangle);
        }

        enum Op {
            UpdateItems,
            OpenEvent,
            LeftPress,
            ScrollEvent,
            RedrawUpdate,
        }

        let mut run_op = |global_state: &mut GlobalState, tree: &mut Tree, op: &Op| {
            let Tree {
                state,
                children: item_trees,
                ..
            } = tree;
            let menu_state = state.downcast_mut::<MenuState>();

            match op {
                Op::UpdateItems => {
                    itl_iter_slice!(
                        menu_state.slice,
                        self.items;iter_mut,
                        item_trees;iter_mut,
                        slice_layout.children()
                    )
                    .for_each(|((item, tree), layout)| {
                        item.update(
                            tree, event, layout, cursor, renderer, clipboard, shell, viewport,
                        );
                    });
                }
                Op::RedrawUpdate => {
                    let cursor = if let Some(active) = menu_state.active {
                        match &global_parameters.path_highlight {
                            PathHighlight::Hover => {
                                let active_in_slice = active - menu_state.slice.start_index;
                                let center = slice_layout
                                    .children()
                                    .nth(active_in_slice)
                                    .expect(
                                        "Index (in slice space) is not within the slice layout. \
                                        This should not happen, please report this issue",
                                    )
                                    .bounds()
                                    .center();
                                mouse::Cursor::Available(center)
                            }
                            PathHighlight::Fill => mouse::Cursor::Unavailable,
                        }
                    } else {
                        cursor
                    };

                    let mut temp_messages = vec![];
                    let mut temp_shell = Shell::new(&mut temp_messages);

                    let redraw_event =
                        Event::Window(window::Event::RedrawRequested(Instant::now()));

                    itl_iter_slice!(
                        menu_state.slice,
                        self.items;iter_mut,
                        item_trees;iter_mut,
                        slice_layout.children()
                    )
                    .for_each(|((item, tree), layout)| {
                        item.update(
                            tree,
                            &redraw_event,
                            layout,
                            cursor,
                            renderer,
                            clipboard,
                            &mut temp_shell,
                            viewport,
                        );
                    });
                    shell.merge(temp_shell, |message| message);
                }
                Op::LeftPress => {
                    if matches!(
                        event,
                        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
                    ) {
                        schedule_close_on_click(
                            global_state,
                            global_parameters,
                            menu_state.slice,
                            &mut self.items,
                            slice_layout.children(),
                            cursor,
                        );
                    }
                }
                Op::ScrollEvent => {
                    if let Event::Mouse(mouse::Event::WheelScrolled { delta }) = event {
                        if cursor.is_over(background_bounds) {
                            let delta_y = match delta {
                                mouse::ScrollDelta::Lines { y, .. } => {
                                    y * global_parameters.scroll_speed.per_line
                                }
                                mouse::ScrollDelta::Pixels { y, .. } => {
                                    y * global_parameters.scroll_speed.per_pixel
                                }
                            };

                            let max_offset = (0.0 - items_bounds.y).max(0.0);
                            let min_offset = (viewport.size().height
                                - (items_bounds.y + items_bounds.height))
                                .min(0.0);
                            menu_state.scroll_offset =
                                (menu_state.scroll_offset + delta_y).clamp(min_offset, max_offset);
                        }
                        shell.request_redraw();
                    }
                }
                Op::OpenEvent => {
                    if !global_state.pressed {
                        assert!(
                            menu_state.active.is_none(),
                            "
                        Menu::open_event() is called only when RecEvent::Close is returned, \
                        which means no child menu should be open (menu_state.active must be None). \
                        If this assert fails, please report this issue.
                    "
                        );

                        try_open_menu(
                            &mut self.items,
                            menu_state,
                            item_trees,
                            slice_layout.children(),
                            cursor,
                            shell,
                        );
                    }
                }
            }
        };

        let mut update = |global_state: &mut GlobalState, tree: &mut Tree, ops: &[Op]| {
            for op in ops.iter() {
                run_op(global_state, tree, op);
            }
        };

        match rec_event {
            RecEvent::Event => {
                // menu not in focus
                update(global_state, tree, &[Op::RedrawUpdate]);
                shell.capture_event();
                RecEvent::Event
            }
            RecEvent::Close => {
                if cursor.is_over(background_bounds) || cursor.is_over(offset_bounds) {
                    // menu in focus
                    update(
                        global_state,
                        tree,
                        &[
                            Op::UpdateItems,
                            Op::LeftPress,
                            Op::ScrollEvent,
                            Op::OpenEvent,
                        ],
                    );
                    shell.capture_event();
                    RecEvent::Event
                } else if cursor.is_over(parent_bounds) {
                    // menu not in focus
                    update(global_state, tree, &[Op::RedrawUpdate]);
                    // the cursor is over the parent bounds
                    // let the parent process the event
                    assert!(!shell.is_event_captured(), "Returning RecEvent::None");
                    RecEvent::None
                } else {
                    let menu_state = tree.state.downcast_ref::<MenuState>();
                    let in_safe_triangle = if let (Some(cursor_pos), Some(triangle)) =
                        (cursor.position(), menu_state.safe_triangle)
                    {
                        triangle.contains(cursor_pos)
                    } else {
                        false
                    };

                    let open = {
                        if global_state.pressed {
                            true
                        } else if in_safe_triangle {
                            true
                        } else if prev_bounds_list.iter().any(|r| cursor.is_over(*r)) {
                            false
                        } else {
                            cursor.is_over(safe_bounds)
                        }
                    };

                    if open {
                        // the current menu is not ready to close
                        update(global_state, tree, &[Op::UpdateItems, Op::LeftPress]);
                        shell.capture_event();
                        RecEvent::Event
                    } else {
                        // the current menu is ready to close
                        assert!(!shell.is_event_captured(), "Returning RecEvent::Close");
                        *prev_active = None;
                        if tree.children.len() == 2 {
                            // prune the menu tree when the menu is closed
                            let _ = tree.children.pop();
                        }
                        shell.invalidate_layout();
                        shell.request_redraw();
                        RecEvent::Close
                    }
                }
            }
            RecEvent::None => {
                update(
                    global_state,
                    tree,
                    &[Op::UpdateItems, Op::LeftPress, Op::ScrollEvent],
                );
                shell.capture_event();
                RecEvent::Event
            }
        }
    }

    pub(crate) fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation<()>,
    ) {
        let mut lc = layout.children();
        let slice_layout = lc.next().unwrap();

        let menu_state = tree.state.downcast_mut::<MenuState>();
        let slice = menu_state.slice;

        operation.container(None, layout.bounds());
        operation.traverse(&mut |operation| {
            itl_iter_slice!(slice, self.items;iter_mut, tree.children;iter_mut, slice_layout.children())
                .for_each(|((child, state), layout)| {
                    child.operate(state, layout, renderer, operation);
                });
        });
    }

    /// tree: Tree{ menu_state, \[item_tree...] }
    ///
    /// layout: Node{inf, \[ slice_node, items_bounds, offset_bounds]}
    pub(crate) fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let mut lc = layout.children();
        let slice_layout = lc.next().unwrap();

        let menu_state = tree.state.downcast_ref::<MenuState>();
        let slice = menu_state.slice;

        itl_iter_slice!(slice, self.items;iter, tree.children;iter, slice_layout.children())
            .map(|((item, tree), layout)| item.mouse_interaction(tree, layout, cursor, renderer))
            .max()
            .unwrap_or_default()
    }

    /// tree: Tree{menu_state, \[item_tree...]}
    ///
    /// layout: Node{inf, \[ items_node, slice_node, items_bounds, offset_bounds]}
    pub(crate) fn draw(
        &self,
        path_highlight: &PathHighlight,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        theme_style: &Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let mut lc = layout.children();
        let slice_layout = lc.next().unwrap();
        let items_bounds = lc.next().unwrap().bounds();

        let menu_state = tree.state.downcast_ref::<MenuState>();
        let slice = menu_state.slice;

        // draw background
        let pad_rectangle = pad_rectangle(items_bounds, self.padding);
        if pad_rectangle.intersects(viewport) {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: pad_rectangle,
                    border: theme_style.menu_border,
                    shadow: theme_style.menu_shadow,
                    ..Default::default()
                },
                theme_style.menu_background,
            );
        }

        if let (PathHighlight::Fill, Some(active)) = (path_highlight, menu_state.active) {
            let active_in_slice = active - menu_state.slice.start_index;
            let active_bounds = slice_layout
                .children()
                .nth(active_in_slice)
                .expect(
                    "Index (in slice space) is not within the slice layout. \
                    This should not happen, please report this issue",
                )
                .bounds();

            renderer.fill_quad(
                renderer::Quad {
                    bounds: active_bounds,
                    border: theme_style.path_border,
                    ..Default::default()
                },
                theme_style.path,
            );
        }

        renderer.with_layer(items_bounds, |r| {
            itl_iter_slice!(slice, self.items;iter, tree.children;iter, slice_layout.children())
                .for_each(|((item, tree), layout)| {
                    item.draw(tree, r, theme, style, layout, cursor, viewport);
                });
        });
    }
}

/// An item inside a [`Menu`] or a root of the [`MenuBar`](crate::MenuBar).
#[must_use]
pub struct Item<'a, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: renderer::Renderer,
{
    pub(crate) item: Element<'a, Message, Theme, Renderer>,
    pub(crate) menu: Option<Box<Menu<'a, Message, Theme, Renderer>>>,
    pub(crate) close_on_click: Option<bool>,
}
impl<'a, Message, Theme, Renderer> Item<'a, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: renderer::Renderer,
{
    /// Creates an [`Item`] with the given element.
    pub fn new(item: impl Into<Element<'a, Message, Theme, Renderer>>) -> Self {
        Self {
            item: item.into(),
            menu: None,
            close_on_click: None,
        }
    }

    /// Creates an [`Item`] with the given element and submenu.
    pub fn with_menu(
        item: impl Into<Element<'a, Message, Theme, Renderer>>,
        menu: Menu<'a, Message, Theme, Renderer>,
    ) -> Self {
        Self {
            item: item.into(),
            menu: Some(Box::new(menu)),
            close_on_click: None,
        }
    }

    /// Keeps the menu open when this entry is clicked, overriding the bar-wide
    /// [`Dismiss`](crate::Dismiss) policy.
    pub fn keep_open(mut self) -> Self {
        self.close_on_click = Some(false);
        self
    }

    /// Closes the menu tree when this entry is clicked, overriding the bar-wide
    /// [`Dismiss`](crate::Dismiss) policy (useful under [`Dismiss::Manual`](crate::Dismiss::Manual)).
    pub fn close_on_click(mut self) -> Self {
        self.close_on_click = Some(true);
        self
    }

    /// Rebuild state tree
    pub(crate) fn tree(&self) -> Tree {
        Tree {
            tag: self.tag(),
            state: self.state(),
            children: self.children(),
        }
    }
}
impl<Message, Theme, Renderer> Item<'_, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: renderer::Renderer,
{
    pub(crate) fn tag(&self) -> tree::Tag {
        tree::Tag::stateless()
    }

    pub(crate) fn state(&self) -> tree::State {
        tree::State::None
    }

    /// out: \[widget_tree, menu_tree]
    pub(crate) fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.item)]
    }

    /// tree: Tree{stateless, \[widget_tree, menu_tree]}
    #[allow(clippy::option_if_let_else)]
    pub(crate) fn diff(&self, tree: &mut Tree) {
        if let Some(t0) = tree.children.get_mut(0) {
            t0.diff(&self.item);
            if let Some(m) = self.menu.as_ref() {
                if let Some(t1) = tree.children.get_mut(1) {
                    m.diff(t1);
                } else {
                    *tree = self.tree();
                }
            }
        } else {
            *tree = self.tree();
        }
    }

    /// tree: Tree{stateless, \[widget_tree, menu_tree]}
    pub(crate) fn update(
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
        self.item.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        )
    }

    /// tree: Tree{stateless, \[widget_tree, menu_tree]}
    pub(crate) fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.item.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            &layout.bounds(),
            renderer,
        )
    }

    /// tree: Tree{stateless, \[widget_tree, menu_tree]}
    pub(crate) fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.item.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    pub(crate) fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation<()>,
    ) {
        self.item
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }
}

/// A thin horizontal divider [`Item`] for separating groups of entries in a [`Menu`].
///
/// It is inert (carries no message) and adapts to the active [`iced::Theme`], matching the
/// crate's baseline flyout styling from [`primary`](crate::primary).
pub fn separator<'a, Message>() -> Item<'a, Message, iced::Theme, iced::Renderer>
where
    Message: 'a,
{
    use iced::widget::{container, text};

    let line = container(text(""))
        .width(Length::Fill)
        .height(1)
        .style(|theme: &iced::Theme| {
            let palette = theme.extended_palette();
            container::Style {
                background: Some(palette.background.strong.color.into()),
                ..container::Style::default()
            }
        });

    Item::new(container(line).padding([4, 6]))
}

/// App-friendly constructors for the built-in [`iced::Theme`].
///
/// These build a styled menu [`button`] for you — the same baseline look as [`menu_item_style`]
/// and [`separator`] — so a consuming app does not need to assemble buttons by hand.
///
/// [`leaf`](Self::leaf) is the shorthand for a plain action row; for anything fancier the
/// builders ([`action`](Self::action), [`submenu`](Self::submenu), [`root`](Self::root)) chain
/// `.icon(..)` / `.hotkey(..)` / `.style(..)` and finish with `.build()`.
impl<'a, Message> Item<'a, Message, iced::Theme, iced::Renderer>
where
    Message: Clone + 'a,
{
    /// Creates a leaf [`Item`]: a full-width menu row that publishes `on_press` when clicked,
    /// styled with the crate's default [`menu_item_style`].
    ///
    /// For an icon, a keyboard-shortcut hint or a custom style, use the [`action`](Self::action)
    /// builder instead.
    pub fn leaf(label: impl iced::widget::text::IntoFragment<'a>, on_press: Message) -> Self {
        Self::leaf_core(
            label.into_fragment(),
            None,
            None,
            on_press,
            Box::new(menu_item_style),
        )
    }

    /// Starts building an action (leaf) row with optional decorations.
    ///
    /// Chain [`icon`](ActionBuilder::icon) (left of the label), [`hotkey`](ActionBuilder::hotkey)
    /// (a dimmed shortcut hint right-aligned on the row) and/or [`style`](ActionBuilder::style),
    /// then finish with [`build`](ActionBuilder::build) (or rely on its `Into<Item>`). For a plain
    /// action with no decorations, [`leaf`](Self::leaf) is shorter.
    ///
    /// Hotkeys are display-only hints — they do not register or handle key events — and are not
    /// available on submenu rows.
    pub fn action(
        label: impl iced::widget::text::IntoFragment<'a>,
        on_press: Message,
    ) -> ActionBuilder<'a, Message> {
        ActionBuilder {
            label: label.into_fragment(),
            on_press,
            icon: None,
            hotkey: None,
            style: None,
        }
    }

    /// Starts building a submenu [`Item`]: a full-width menu row that opens a nested `menu` to the
    /// side. Chain [`icon`](SubmenuBuilder::icon) and/or [`style`](SubmenuBuilder::style), then
    /// finish with [`build`](SubmenuBuilder::build) (or rely on its `Into<Item>`).
    ///
    /// Use this for entries **inside** a [`Menu`]; like [`leaf`](Self::leaf) it fills the row so its
    /// hover highlight spans the whole width, and it keeps a trailing chevron. For top-level bar
    /// entries use [`root`](Self::root). No message is needed — the menu bar opens the submenu.
    pub fn submenu(
        label: impl iced::widget::text::IntoFragment<'a>,
        menu: Menu<'a, Message, iced::Theme, iced::Renderer>,
    ) -> SubmenuBuilder<'a, Message> {
        SubmenuBuilder {
            label: label.into_fragment(),
            menu,
            icon: None,
            style: None,
        }
    }

    /// Starts building a root [`Item`]: a content-sized button that opens `menu`. Chain
    /// [`style`](RootBuilder::style), then finish with [`build`](RootBuilder::build) (or rely on
    /// its `Into<Item>`).
    ///
    /// Use this for the top-level entries of a [`MenuBar`](crate::MenuBar) so the bar buttons hug
    /// their labels instead of stretching across the bar. No message is needed — the menu bar
    /// opens the menu.
    pub fn root(
        label: impl iced::widget::text::IntoFragment<'a>,
        menu: Menu<'a, Message, iced::Theme, iced::Renderer>,
    ) -> RootBuilder<'a, Message> {
        RootBuilder {
            label: label.into_fragment(),
            menu,
            style: None,
        }
    }

    /// Shared layout for [`leaf`](Self::leaf) and the [`action`](Self::action) builder: a
    /// full-width button whose row reserves a fixed-width icon column, the label, and an optional
    /// right-aligned hotkey hint.
    fn leaf_core(
        label: iced::widget::text::Fragment<'a>,
        icon: Option<Element<'a, Message, iced::Theme, iced::Renderer>>,
        hotkey: Option<iced::widget::text::Fragment<'a>>,
        on_press: Message,
        style: ButtonStyleFn<'a>,
    ) -> Self {
        use iced::alignment::Vertical;
        use iced::widget::{button, row, text};

        let mut content = row![icon_slot(icon), text(label).width(Length::Fill)]
            .align_y(Vertical::Center)
            .spacing(8);
        if let Some(hotkey) = hotkey {
            content = content.push(hotkey_label(hotkey));
        }

        Self::new(
            button(content)
                .width(Length::Fill)
                .padding([5, 12])
                .style(style)
                .on_press(on_press),
        )
    }

    /// Shared layout for the [`submenu`](Self::submenu) builder: a full-width button whose row
    /// reserves a fixed-width icon column, the label, and a trailing submenu chevron.
    ///
    /// The button carries no `on_press` — the [`MenuBar`](crate::MenuBar) drives opening from the
    /// cursor — so [`menu_item_style`] renders its `Disabled` state like `Active`.
    fn submenu_core(
        label: iced::widget::text::Fragment<'a>,
        icon: Option<Element<'a, Message, iced::Theme, iced::Renderer>>,
        menu: Menu<'a, Message, iced::Theme, iced::Renderer>,
        style: ButtonStyleFn<'a>,
    ) -> Self {
        use iced::alignment::Vertical;
        use iced::widget::{button, row, text};

        Self::with_menu(
            button(
                row![icon_slot(icon), text(label).width(Length::Fill), submenu_chevron()]
                    .align_y(Vertical::Center)
                    .spacing(8),
            )
            .width(Length::Fill)
            .padding([5, 12])
            .style(style)
            .on_press_maybe(None),
            menu,
        )
    }

    /// Shared layout for the [`root`](Self::root) builder: a content-sized button that opens a
    /// menu. Like [`submenu_core`](Self::submenu_core) it carries no `on_press`.
    fn root_core(
        label: iced::widget::text::Fragment<'a>,
        menu: Menu<'a, Message, iced::Theme, iced::Renderer>,
        style: ButtonStyleFn<'a>,
    ) -> Self {
        use iced::widget::{button, text};

        Self::with_menu(
            button(text(label)).padding([5, 10]).style(style).on_press_maybe(None),
            menu,
        )
    }
}

/// A boxed [`button`] style function, used by [`ActionBuilder`] to carry a per-action custom style.
type ButtonStyleFn<'a> =
    Box<dyn Fn(&iced::Theme, iced::widget::button::Status) -> iced::widget::button::Style + 'a>;

/// A chainable builder for an action (leaf) [`Item`] on the built-in [`iced::Theme`].
///
/// Created by [`Item::action`]. All decorations are optional; finish with [`build`](Self::build)
/// or rely on its [`From`]/`Into<Item>` conversion:
///
/// ```ignore
/// Menu::new(vec![
///     Item::action("Save", Message::Save).hotkey("⌘S").build(),
///     Item::action("Open", Message::Open).icon(icon).hotkey("⌘O").build(),
/// ]);
/// ```
#[must_use]
pub struct ActionBuilder<'a, Message> {
    label: iced::widget::text::Fragment<'a>,
    on_press: Message,
    icon: Option<Element<'a, Message, iced::Theme, iced::Renderer>>,
    hotkey: Option<iced::widget::text::Fragment<'a>>,
    style: Option<ButtonStyleFn<'a>>,
}

impl<'a, Message> ActionBuilder<'a, Message>
where
    Message: Clone + 'a,
{
    /// Sets the icon shown to the left of the label, in the fixed-width column reserved on every
    /// leaf/submenu row. The icon is any [`Element`]; size it to about 16×16. You control its color.
    pub fn icon(
        mut self,
        icon: impl Into<Element<'a, Message, iced::Theme, iced::Renderer>>,
    ) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Sets a keyboard-shortcut hint shown, dimmed and right-aligned, on the row (e.g. `"⌘S"`).
    ///
    /// Display-only — it does not register or handle the key combination.
    pub fn hotkey(mut self, hotkey: impl iced::widget::text::IntoFragment<'a>) -> Self {
        self.hotkey = Some(hotkey.into_fragment());
        self
    }

    /// Swaps in a custom [`button`] style, replacing the crate's default [`menu_item_style`].
    pub fn style(
        mut self,
        style: impl Fn(&iced::Theme, iced::widget::button::Status) -> iced::widget::button::Style
        + 'a,
    ) -> Self {
        self.style = Some(Box::new(style));
        self
    }

    /// Finishes building, producing the action [`Item`].
    pub fn build(self) -> Item<'a, Message, iced::Theme, iced::Renderer> {
        let style = self.style.unwrap_or_else(|| Box::new(menu_item_style));
        Item::leaf_core(self.label, self.icon, self.hotkey, self.on_press, style)
    }
}

impl<'a, Message> From<ActionBuilder<'a, Message>> for Item<'a, Message, iced::Theme, iced::Renderer>
where
    Message: Clone + 'a,
{
    fn from(builder: ActionBuilder<'a, Message>) -> Self {
        builder.build()
    }
}

/// A chainable builder for a submenu [`Item`] on the built-in [`iced::Theme`].
///
/// Created by [`Item::submenu`]. The icon and style are optional; finish with
/// [`build`](Self::build) or rely on its [`From`]/`Into<Item>` conversion. No message is needed —
/// the [`MenuBar`](crate::MenuBar) opens the nested menu.
#[must_use]
pub struct SubmenuBuilder<'a, Message> {
    label: iced::widget::text::Fragment<'a>,
    menu: Menu<'a, Message, iced::Theme, iced::Renderer>,
    icon: Option<Element<'a, Message, iced::Theme, iced::Renderer>>,
    style: Option<ButtonStyleFn<'a>>,
}

impl<'a, Message> SubmenuBuilder<'a, Message>
where
    Message: Clone + 'a,
{
    /// Sets the icon shown to the left of the label, in the fixed-width column reserved on every
    /// leaf/submenu row. The icon is any [`Element`]; size it to about 16×16. You control its color.
    pub fn icon(
        mut self,
        icon: impl Into<Element<'a, Message, iced::Theme, iced::Renderer>>,
    ) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Swaps in a custom [`button`] style, replacing the crate's default [`menu_item_style`].
    pub fn style(
        mut self,
        style: impl Fn(&iced::Theme, iced::widget::button::Status) -> iced::widget::button::Style
        + 'a,
    ) -> Self {
        self.style = Some(Box::new(style));
        self
    }

    /// Finishes building, producing the submenu [`Item`].
    pub fn build(self) -> Item<'a, Message, iced::Theme, iced::Renderer> {
        let style = self.style.unwrap_or_else(|| Box::new(menu_item_style));
        Item::submenu_core(self.label, self.icon, self.menu, style)
    }
}

impl<'a, Message> From<SubmenuBuilder<'a, Message>>
    for Item<'a, Message, iced::Theme, iced::Renderer>
where
    Message: Clone + 'a,
{
    fn from(builder: SubmenuBuilder<'a, Message>) -> Self {
        builder.build()
    }
}

/// A chainable builder for a root (menu-bar) [`Item`] on the built-in [`iced::Theme`].
///
/// Created by [`Item::root`]. The style is optional; finish with [`build`](Self::build) or rely on
/// its [`From`]/`Into<Item>` conversion. No message is needed — the [`MenuBar`](crate::MenuBar)
/// opens the menu.
#[must_use]
pub struct RootBuilder<'a, Message> {
    label: iced::widget::text::Fragment<'a>,
    menu: Menu<'a, Message, iced::Theme, iced::Renderer>,
    style: Option<ButtonStyleFn<'a>>,
}

impl<'a, Message> RootBuilder<'a, Message>
where
    Message: Clone + 'a,
{
    /// Swaps in a custom [`button`] style, replacing the crate's default [`menu_item_style`].
    pub fn style(
        mut self,
        style: impl Fn(&iced::Theme, iced::widget::button::Status) -> iced::widget::button::Style
        + 'a,
    ) -> Self {
        self.style = Some(Box::new(style));
        self
    }

    /// Finishes building, producing the root [`Item`].
    pub fn build(self) -> Item<'a, Message, iced::Theme, iced::Renderer> {
        let style = self.style.unwrap_or_else(|| Box::new(menu_item_style));
        Item::root_core(self.label, self.menu, style)
    }
}

impl<'a, Message> From<RootBuilder<'a, Message>> for Item<'a, Message, iced::Theme, iced::Renderer>
where
    Message: Clone + 'a,
{
    fn from(builder: RootBuilder<'a, Message>) -> Self {
        builder.build()
    }
}

/// Recommended icon box size; the documented target for caller-supplied leaf/submenu icons.
const ICON_SIZE: f32 = 16.0;
/// Fixed width of the reserved icon column. Identical for icon and icon-less rows so labels align.
const ICON_SLOT_WIDTH: f32 = 20.0;

/// The fixed-width left column reserved on every leaf/submenu row.
///
/// `Some(icon)` renders the icon centered in the column; `None` reserves the exact same width with
/// an empty spacer — so labels line up across a menu mixing icon and icon-less entries.
fn icon_slot<'a, Message: 'a>(
    content: Option<Element<'a, Message, iced::Theme, iced::Renderer>>,
) -> Element<'a, Message, iced::Theme, iced::Renderer> {
    use iced::alignment::{Horizontal, Vertical};
    use iced::widget::{Space, container};

    let inner = content.unwrap_or_else(|| Space::new().into());
    container(inner)
        .width(Length::Fixed(ICON_SLOT_WIDTH))
        .height(Length::Fixed(ICON_SIZE))
        .align_x(Horizontal::Center)
        .align_y(Vertical::Center)
        .into()
}

/// Opacity applied to the normal label color for right-side hotkey hints, so they read a touch
/// dimmer and distinct from the label without losing legibility.
const HOTKEY_ALPHA: f32 = 0.6;

/// A dimmed keyboard-shortcut hint shown on the right of an action row.
///
/// Like [`submenu_chevron`], its color is not hover-aware — it stays a faded variant of the menu
/// label's resting text color (see [`menu_item_style`]) across all states.
fn hotkey_label<'a, Message: 'a>(
    hotkey: iced::widget::text::Fragment<'a>,
) -> Element<'a, Message, iced::Theme, iced::Renderer> {
    use iced::widget::text;

    text(hotkey)
        .style(|theme: &iced::Theme| text::Style {
            color: Some(iced::Color {
                a: HOTKEY_ALPHA,
                ..theme.extended_palette().background.base.text
            }),
        })
        .into()
}

/// The trailing arrow drawn on submenu rows to signal they open a nested flyout.
///
/// Colored to match the menu label's resting text color (see [`menu_item_style`]).
fn submenu_chevron<'a, Message: 'a>() -> Element<'a, Message, iced::Theme, iced::Renderer> {
    use iced::widget::svg;

    let handle =
        svg::Handle::from_memory(include_bytes!("../svg/arrow-next-small-svgrepo-com.svg").as_slice());

    svg(handle)
        .width(14)
        .height(14)
        .style(|theme: &iced::Theme, _status| svg::Style {
            color: Some(theme.extended_palette().background.base.text),
        })
        .into()
}

/// Adaptive open direction
#[derive(Debug)]
#[allow(clippy::struct_excessive_bools)]
struct Aod {
    // whether or not to use overlap
    horizontal_overlap: bool,
    vertical_overlap: bool,

    // default direction
    horizontal_direction: Direction,
    vertical_direction: Direction,

    // Offset of the child in the default direction
    horizontal_offset: f32,
    vertical_offset: f32,
}
impl Aod {
    /// Returns (child position, offset position, child direction)
    fn adaptive(
        parent_pos: f32,
        parent_size: f32,
        child_size: f32,
        max_size: f32,
        offset: f32,
        overlap: bool,
        direction: Direction,
    ) -> (f32, f32, Direction) {
        /*
        Imagine there are two sticks, parent and child
        parent: o-----o
        child:  o----------o

        Now we align the child to the parent in one dimension
        There are 4 possibilities:

        1. to the right
                    o-----oo----------o

        2. to the right with overlapping
                    o-----o
                    o----------o

        3. to the left
        o----------oo-----o

        4. to the left with overlapping
                    o-----o
               o----------o

        The child goes to the default direction by default,
        if the space on the default direction runs out it goes to the other,
        whether to use overlap is the caller's decision

        This can be applied to any direction
        */

        match direction {
            Direction::Positive => {
                let space_negative = parent_pos;
                let space_positive = max_size - parent_pos - parent_size;

                if overlap {
                    let overshoot = child_size - parent_size;
                    if space_negative > space_positive && overshoot > space_positive {
                        (
                            parent_pos - overshoot,
                            parent_pos - overshoot,
                            direction.flip(),
                        )
                    } else {
                        (parent_pos, parent_pos, direction)
                    }
                } else {
                    let overshoot = child_size + offset;
                    if space_negative > space_positive && overshoot > space_positive {
                        (
                            parent_pos - overshoot,
                            parent_pos - offset,
                            direction.flip(),
                        )
                    } else {
                        (
                            parent_pos + parent_size + offset,
                            parent_pos + parent_size,
                            direction,
                        )
                    }
                }
            }
            Direction::Negative => {
                let space_positive = parent_pos;
                let space_negative = max_size - parent_pos - parent_size;

                if overlap {
                    let overshoot = child_size - parent_size;
                    if space_negative > space_positive && overshoot > space_positive {
                        (parent_pos, parent_pos, direction.flip())
                    } else {
                        (parent_pos - overshoot, parent_pos - overshoot, direction)
                    }
                } else {
                    let overshoot = child_size + offset;
                    if space_negative > space_positive && overshoot > space_positive {
                        (
                            parent_pos + parent_size + offset,
                            parent_pos + parent_size,
                            direction.flip(),
                        )
                    } else {
                        (parent_pos - overshoot, parent_pos - offset, direction)
                    }
                }
            }
        }
    }

    /// Returns (child position, offset position, child direction)
    fn resolve(
        &self,
        parent_bounds: Rectangle,
        children_size: Size,
        viewport_size: Size,
    ) -> (Point, Point, (Direction, Direction)) {
        let (x, ox, dx) = Self::adaptive(
            parent_bounds.x,
            parent_bounds.width,
            children_size.width,
            viewport_size.width,
            self.horizontal_offset,
            self.horizontal_overlap,
            self.horizontal_direction,
        );
        let (y, oy, dy) = Self::adaptive(
            parent_bounds.y,
            parent_bounds.height,
            children_size.height,
            viewport_size.height,
            self.vertical_offset,
            self.vertical_overlap,
            self.vertical_direction,
        );

        ([x, y].into(), [ox, oy].into(), (dx, dy))
    }

    fn new(
        axis: Axis,
        viewport: Size,
        parent_bounds: Rectangle,
        parent_direction: (Direction, Direction),
        offset: f32,
    ) -> Self {
        let hcenter = viewport.width / 2.0;
        let vcenter = viewport.height / 2.0;

        let phcenter = parent_bounds.x + parent_bounds.width / 2.0;
        let pvcenter = parent_bounds.y + parent_bounds.height / 2.0;

        let (pdx, pdy) = parent_direction;
        match axis {
            Axis::Horizontal => {
                let horizontal_direction = pdx;
                let vertical_direction = if pvcenter < vcenter {
                    Direction::Positive
                } else {
                    Direction::Negative
                };
                Self {
                    horizontal_overlap: false,
                    vertical_overlap: true,
                    horizontal_direction,
                    vertical_direction,
                    horizontal_offset: offset,
                    vertical_offset: 0.0,
                }
            }
            Axis::Vertical => {
                let horizontal_direction = if phcenter < hcenter {
                    Direction::Positive
                } else {
                    Direction::Negative
                };
                let vertical_direction = pdy;
                Self {
                    horizontal_overlap: true,
                    vertical_overlap: false,
                    horizontal_direction,
                    vertical_direction,
                    horizontal_offset: 0.0,
                    vertical_offset: offset,
                }
            }
        }
    }
}

fn cal_bounds_rel_menu(
    items_node: &Node,
    translation: Vector,
    viewport: Size,
    scroll_offset: f32,
) -> (f32, f32) {
    let items_bounds = items_node.bounds() + translation; // viewport space

    // viewport space absolute bounds
    let lower_bound = items_bounds.y.max(0.0);
    let upper_bound = (items_bounds.y + items_bounds.height).min(viewport.height);

    // menu space relative bounds
    let lower_bound_rel = lower_bound - (items_bounds.y + scroll_offset);
    let upper_bound_rel = upper_bound - (items_bounds.y + scroll_offset);

    (lower_bound_rel, upper_bound_rel)
}
