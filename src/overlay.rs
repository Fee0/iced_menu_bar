//! The recursive overlay that renders the open menus of a [`MenuBar`](crate::MenuBar).
#![allow(clippy::unwrap_used)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::wildcard_imports)]
#![allow(clippy::enum_glob_use)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::items_after_statements)]
#![allow(clippy::similar_names)]

use iced::advanced::layout::{Limits, Node};
use iced::advanced::widget::{Operation, Tree};
use iced::advanced::{Clipboard, Layout, Shell, overlay, renderer};
use iced::time::Instant;
use iced::{Event, Point, Rectangle, Size, Vector, keyboard, mouse, window};

use crate::common::*;
use crate::menu::*;
use crate::menu_bar::*;
use crate::style::*;

pub(crate) struct MenuBarOverlay<'a, 'b, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: renderer::Renderer,
{
    pub(crate) menu_bar: &'b mut MenuBar<'a, Message, Theme, Renderer>,
    pub(crate) layout: Layout<'b>,
    pub(crate) translation: Vector,
    /// Tree{ bar, [item_tree...] }
    pub(crate) tree: &'b mut Tree,
}
impl<'b, Message, Theme, Renderer> MenuBarOverlay<'_, 'b, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: renderer::Renderer,
{
    pub(crate) fn overlay_element(self) -> overlay::Element<'b, Message, Theme, Renderer> {
        overlay::Element::new(Box::new(self))
    }

    /// Handles a key press while the menu tree is open. Returns whether the key was consumed.
    ///
    /// Navigation operates on the deepest open menu (the keyboard focus): ↑/↓ move the highlight,
    /// → opens the highlighted submenu (or steps to the next root at the top level), ← closes the
    /// current submenu (or steps to the previous root), Enter/Space activates, Esc closes a level.
    fn handle_key(
        &mut self,
        key: &keyboard::Key,
        layout: Layout<'_>,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
    ) -> bool {
        use keyboard::key::Named;

        let keyboard::Key::Named(named) = key else {
            return false;
        };
        let action = match named {
            Named::ArrowDown => KeyAction::Down,
            Named::ArrowUp => KeyAction::Up,
            Named::ArrowRight => KeyAction::Right,
            Named::ArrowLeft => KeyAction::Left,
            Named::Enter | Named::Space => KeyAction::Activate,
            Named::Escape => KeyAction::Escape,
            _ => return false,
        };

        let Tree {
            state,
            children: item_trees,
            ..
        } = &mut *self.tree;
        let bar = state.downcast_mut::<MenuBarState>();
        if !bar.global_state.open {
            return false;
        }
        let Some(active_root) = bar.menu_state.active else {
            return false;
        };

        // Entering keyboard mode: keep the tree open regardless of cursor position until the mouse
        // moves again (see `GlobalState::keyboard_nav`).
        bar.global_state.keyboard_nav = true;

        let viewport = layout.bounds();
        let mut lc = layout.children();
        let _bar_bounds = lc.next();
        let _roots_layout = lc.next();
        let Some(menu_layouts_layout) = lc.next() else {
            return false;
        };
        let mut menu_layouts = menu_layouts_layout.children();

        let close_on_item_click = self.menu_bar.global_parameters.close_on_item_click;

        let outcome = {
            let root_item = &mut self.menu_bar.roots[active_root];
            let Some(root_menu) = root_item.menu.as_mut() else {
                return false;
            };
            let root_tree = &mut item_trees[active_root];
            if root_tree.children.len() < 2 {
                return false;
            }
            let menu_tree = &mut root_tree.children[1];

            apply_key(
                action,
                &mut root_menu.items,
                menu_tree,
                &mut menu_layouts,
                true,
                renderer,
                clipboard,
                shell,
                viewport,
                close_on_item_click,
            )
        };

        match outcome {
            KeyOutcome::Unhandled => false,
            KeyOutcome::Handled => true,
            KeyOutcome::CloseLevel | KeyOutcome::CloseAll => {
                bar.close(item_trees, shell);
                true
            }
            KeyOutcome::SwitchRoot(delta) => {
                let slice = bar.menu_state.slice;
                let n = self.menu_bar.roots.len();
                if n == 0 {
                    return true;
                }
                let lo = slice.start_index.min(n - 1);
                let hi = slice.end_index.min(n - 1);
                let span = hi - lo + 1;

                // Prune the current root's menu subtree before opening the next one.
                if let Some(t) = item_trees.get_mut(active_root)
                    && t.children.len() == 2
                {
                    let _ = t.children.pop();
                }
                bar.menu_state.active = None;

                // Step to the next root (within the visible slice) that actually has a menu.
                let mut new = active_root;
                for _ in 0..span {
                    new = if delta > 0 {
                        if new >= hi { lo } else { new + 1 }
                    } else if new <= lo {
                        hi
                    } else {
                        new - 1
                    };
                    if self.menu_bar.roots[new].menu.is_some() {
                        let item = &self.menu_bar.roots[new];
                        bar.menu_state
                            .open_new_menu(new, item, &mut item_trees[new]);
                        if let Some(mt) = item_trees[new].children.get_mut(1) {
                            let first = self.menu_bar.roots[new]
                                .menu
                                .as_deref()
                                .and_then(|m| first_navigable(&m.items));
                            mt.state.downcast_mut::<MenuState>().keyboard_highlight = first;
                        }
                        break;
                    }
                }

                shell.invalidate_layout();
                shell.request_redraw();
                true
            }
        }
    }
}

/// A keyboard navigation command, resolved from a key press in
/// [`MenuBarOverlay::handle_key`].
#[derive(Clone, Copy)]
enum KeyAction {
    Down,
    Up,
    Left,
    Right,
    Activate,
    Escape,
}

/// The result of applying a [`KeyAction`] at a menu level, bubbled up the open path.
enum KeyOutcome {
    /// The key was not consumed; let the rest of the widget handle it.
    Unhandled,
    /// The key was consumed; redraw.
    Handled,
    /// Close just this menu level; the parent consumes it (or the bar closes the tree at the top).
    CloseLevel,
    /// Close the entire menu tree (an entry was activated under a close-on-click policy).
    CloseAll,
    /// Move to an adjacent top-level (root) menu by the given signed step.
    SwitchRoot(isize),
}

/// Applies a [`KeyAction`] to the menu at this level, recursing into the open child first so the
/// deepest open menu (the keyboard focus) is the one that acts.
#[allow(clippy::too_many_arguments)]
fn apply_key<'a, 'b, Message, Theme: Catalog, Renderer: renderer::Renderer>(
    action: KeyAction,
    items: &mut [Item<'a, Message, Theme, Renderer>],
    menu_tree: &mut Tree,
    menu_layouts: &mut impl Iterator<Item = Layout<'b>>,
    is_top: bool,
    renderer: &Renderer,
    clipboard: &mut dyn Clipboard,
    shell: &mut Shell<'_, Message>,
    viewport: Rectangle,
    close_on_item_click: bool,
) -> KeyOutcome {
    let Some(menu_layout) = menu_layouts.next() else {
        return KeyOutcome::Unhandled;
    };
    let Some(slice_layout) = menu_layout.children().next() else {
        return KeyOutcome::Unhandled;
    };

    let Tree {
        state,
        children: item_trees,
        ..
    } = menu_tree;
    let menu_state = state.downcast_mut::<MenuState>();

    // Descend into the open child menu first; the deepest open menu owns the keyboard focus.
    if let Some(active) = menu_state.active {
        let outcome = {
            let Some(child_item) = items.get_mut(active) else {
                return KeyOutcome::Unhandled;
            };
            let Some(child_menu) = child_item.menu.as_mut() else {
                return KeyOutcome::Unhandled;
            };
            let child_item_tree = &mut item_trees[active];
            if child_item_tree.children.len() < 2 {
                return KeyOutcome::Unhandled;
            }
            let child_menu_tree = &mut child_item_tree.children[1];

            apply_key(
                action,
                &mut child_menu.items,
                child_menu_tree,
                menu_layouts,
                false,
                renderer,
                clipboard,
                shell,
                viewport,
                close_on_item_click,
            )
        };

        return match outcome {
            KeyOutcome::CloseLevel => {
                // This level owns the child that asked to close.
                menu_state.active = None;
                menu_state.keyboard_highlight = Some(active);
                if item_trees[active].children.len() == 2 {
                    let _ = item_trees[active].children.pop();
                }
                shell.invalidate_layout();
                shell.request_redraw();
                KeyOutcome::Handled
            }
            other => other,
        };
    }

    // This is the focused (deepest) menu.
    if items.is_empty() {
        return KeyOutcome::Unhandled;
    }

    match action {
        KeyAction::Down => {
            if let Some(next) = next_navigable(items, menu_state.keyboard_highlight, true) {
                menu_state.keyboard_highlight = Some(next);
                shell.request_redraw();
            }
            KeyOutcome::Handled
        }
        KeyAction::Up => {
            if let Some(next) = next_navigable(items, menu_state.keyboard_highlight, false) {
                menu_state.keyboard_highlight = Some(next);
                shell.request_redraw();
            }
            KeyOutcome::Handled
        }
        KeyAction::Right => {
            if let Some(i) = menu_state.keyboard_highlight
                && items[i].menu.is_some()
            {
                open_child(menu_state, items, item_trees, i, shell);
                return KeyOutcome::Handled;
            }
            if is_top {
                KeyOutcome::SwitchRoot(1)
            } else {
                KeyOutcome::Handled
            }
        }
        KeyAction::Left => {
            if is_top {
                KeyOutcome::SwitchRoot(-1)
            } else {
                KeyOutcome::CloseLevel
            }
        }
        KeyAction::Escape => KeyOutcome::CloseLevel,
        KeyAction::Activate => {
            let Some(i) = menu_state.keyboard_highlight else {
                return KeyOutcome::Handled;
            };
            if items[i].menu.is_some() {
                open_child(menu_state, items, item_trees, i, shell);
                return KeyOutcome::Handled;
            }

            // Synthesize a click on the focused leaf so its inner widget publishes its message.
            let slice = menu_state.slice;
            if i < slice.start_index || i > slice.end_index {
                return KeyOutcome::Handled;
            }
            let idx_in_slice = i - slice.start_index;
            let Some(item_layout) = slice_layout.children().nth(idx_in_slice) else {
                return KeyOutcome::Handled;
            };
            let center = item_layout.bounds().center();
            let synth_cursor = mouse::Cursor::Available(center);

            let item = &mut items[i];
            let item_tree = &mut item_trees[i];
            for ev in [
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
                Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
            ] {
                // A fresh shell per synthetic event: a button captures the shell on press, and it
                // early-returns on any already-captured event — so feeding both press and release
                // through one shell would make it skip the release and never publish the message.
                let mut synth_messages = vec![];
                let mut synth_shell = Shell::new(&mut synth_messages);
                item.update(
                    item_tree,
                    &ev,
                    item_layout,
                    synth_cursor,
                    renderer,
                    clipboard,
                    &mut synth_shell,
                    &viewport,
                );
                shell.merge(synth_shell, |m| m);
            }

            if item.close_on_click.unwrap_or(close_on_item_click) {
                KeyOutcome::CloseAll
            } else {
                KeyOutcome::Handled
            }
        }
    }
}

/// Opens the submenu of `items[index]` from `menu_state` and focuses its first entry.
fn open_child<Message, Theme: Catalog, Renderer: renderer::Renderer>(
    menu_state: &mut MenuState,
    items: &[Item<'_, Message, Theme, Renderer>],
    item_trees: &mut [Tree],
    index: usize,
    shell: &mut Shell<'_, Message>,
) {
    menu_state.open_new_menu(index, &items[index], &mut item_trees[index]);
    menu_state.keyboard_highlight = Some(index);
    if let Some(child_menu_tree) = item_trees[index].children.get_mut(1) {
        let first = items[index]
            .menu
            .as_deref()
            .and_then(|m| first_navigable(&m.items));
        child_menu_tree
            .state
            .downcast_mut::<MenuState>()
            .keyboard_highlight = first;
    }
    shell.invalidate_layout();
    shell.request_redraw();
}

/// Finds the next entry keyboard navigation can land on, stepping from `from` in the given
/// direction and wrapping around; skips inert rows (separators, disabled actions). Returns `None`
/// when no entry is navigable.
fn next_navigable<Message, Theme: Catalog, Renderer: renderer::Renderer>(
    items: &[Item<'_, Message, Theme, Renderer>],
    from: Option<usize>,
    forward: bool,
) -> Option<usize> {
    let len = items.len();
    if len == 0 {
        return None;
    }
    // Pick a start so the first step lands on index 0 (forward) or the last index (backward) when
    // nothing is highlighted yet.
    let start = match from {
        Some(i) => i,
        None if forward => len - 1,
        None => 0,
    };
    let mut idx = start;
    for _ in 0..len {
        idx = if forward {
            (idx + 1) % len
        } else {
            (idx + len - 1) % len
        };
        if items[idx].navigable {
            return Some(idx);
        }
    }
    None
}

/// The first entry keyboard navigation can land on, used when a menu gains focus.
fn first_navigable<Message, Theme: Catalog, Renderer: renderer::Renderer>(
    items: &[Item<'_, Message, Theme, Renderer>],
) -> Option<usize> {
    next_navigable(items, None, true)
}
impl<Message, Theme, Renderer> overlay::Overlay<Message, Theme, Renderer>
    for MenuBarOverlay<'_, '_, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: renderer::Renderer,
{
    /// out: Node{inf, [ bar_node, roots_node, menu_nodes_node{0, [ menu_node,...]} ]}
    fn layout(&mut self, renderer: &Renderer, bounds: Size) -> Node {
        let translation = self.translation;

        let bar_bounds = self.layout.bounds();
        let slice_layout = self.layout.children().next().unwrap();

        let root_bounds = slice_layout
            .children()
            .map(|l| l.bounds())
            .collect::<Vec<_>>();

        let bar = self.tree.state.downcast_ref::<MenuBarState>();
        let MenuBarState {
            global_state,
            menu_state: bar_menu_state,
            ..
        } = bar;
        let slice = bar_menu_state.slice;

        let bar_node = Node::with_children(bar_bounds.size(), [].into())
            .move_to(bar_bounds.position() + translation);

        let roots_node = Node::with_children(
            Size::ZERO,
            root_bounds
                .iter()
                .map(|r| Node::new(r.size()).move_to(r.position()))
                .collect(),
        )
        .translate(translation);

        if !global_state.open {
            return Node::with_children(bounds, [bar_node, roots_node].into());
        }

        let Some(active) = bar_menu_state.active else {
            return Node::with_children(bounds, [bar_node, roots_node].into());
        };

        let active_root = &mut self.menu_bar.roots[active];
        let active_tree = &mut self.tree.children[active]; // item_tree: Tree{ stateless, [ widget_tree, menu_tree ] }
        let parent_bounds = root_bounds[active - slice.start_index] + translation;

        fn rec<Message, Theme: Catalog, Renderer: renderer::Renderer>(
            renderer: &Renderer,
            item: &mut Item<'_, Message, Theme, Renderer>,
            tree: &mut Tree,
            menu_nodes: &mut Vec<Node>,
            parent_bounds: Rectangle,
            parent_direction: (Direction, Direction),
            viewport: &Rectangle,
        ) {
            if let Some(menu) = item.menu.as_mut() {
                let menu_tree = &mut tree.children[1];

                let (menu_node, direction) = menu.layout(
                    menu_tree,
                    renderer,
                    &Limits::NONE,
                    parent_bounds,
                    parent_direction,
                    viewport,
                );
                // Node{inf, [ slice_node, items_bounds, offset_bounds]}
                menu_nodes.push(menu_node);

                let menu_state = menu_tree.state.downcast_ref::<MenuState>();

                if let Some(active) = menu_state.active {
                    let next_item = &mut menu.items[active];
                    let next_tree = &mut menu_tree.children[active];
                    let next_parent_bounds = {
                        let slice_node = &menu_nodes.last().unwrap().children()[0];
                        let active_in_slice = active - menu_state.slice.start_index;
                        let node = &slice_node.children()[active_in_slice];
                        node.bounds() + (slice_node.bounds().position() - Point::ORIGIN)
                    };
                    rec(
                        renderer,
                        next_item,
                        next_tree,
                        menu_nodes,
                        next_parent_bounds,
                        direction,
                        viewport,
                    );
                }
            }
        }

        let mut menu_nodes = vec![];

        let parent_direction = {
            let hcenter = bounds.width / 2.0;
            let vcenter = bounds.height / 2.0;

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

        rec(
            renderer,
            active_root,
            active_tree,
            &mut menu_nodes,
            parent_bounds,
            parent_direction,
            &Rectangle::new(Point::ORIGIN, bounds),
        );

        Node::with_children(
            bounds,
            [
                bar_node,
                roots_node,
                Node::with_children(Size::ZERO, menu_nodes),
            ]
            .into(),
        )
    }

    #[allow(unused_results)]
    fn update(
        &mut self,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
    ) {
        if let Event::Keyboard(keyboard::Event::KeyPressed { key, .. }) = event
            && self.handle_key(key, layout, renderer, clipboard, shell)
        {
            shell.capture_event();
            shell.request_redraw();
            return;
        }

        let bar = self.tree.state.downcast_mut::<MenuBarState>();
        let MenuBarState {
            global_state,
            menu_state: bar_menu_state,
            ..
        } = bar;
        let slice = bar_menu_state.slice;

        if !global_state.open {
            return;
        }

        let Some(active) = bar_menu_state.active else {
            return;
        };

        let viewport = layout.bounds();
        let mut lc = layout.children();
        let bar_bounds = lc.next().unwrap().bounds();
        let roots_layout = lc.next().unwrap();

        let parent_bounds = roots_layout
            .children()
            .nth(active - slice.start_index)
            .unwrap()
            .bounds();
        let menu_layouts_layout = lc.next().unwrap(); // Node{0, [menu_node...]}
        let mut menu_layouts = menu_layouts_layout.children(); // [menu_node...]

        let active_root = &mut self.menu_bar.roots[active];
        let active_tree = &mut self.tree.children[active];
        let mut prev_bounds_list = vec![bar_bounds];

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                global_state.pressed = true;
                global_state.keyboard_nav = false;
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                global_state.pressed = false;
                shell.request_redraw();
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                // The mouse takes over again: hand cursor-based open/close back to the pointer.
                global_state.keyboard_nav = false;
            }
            Event::Window(window::Event::Resized { .. }) => {
                bar.close(self.tree.children.as_mut_slice(), shell);
                return;
            }
            _ => {}
        }

        // While navigating by keyboard, hide the cursor from the menu recursion so a keyboard-opened
        // submenu is not closed just because the pointer is elsewhere.
        let cursor = if global_state.keyboard_nav {
            mouse::Cursor::Unavailable
        } else {
            cursor
        };

        #[rustfmt::skip]
        fn rec<'a, 'b, Message, Theme: Catalog, Renderer: renderer::Renderer>(
            global_state: &mut GlobalState,
            global_parameters: &GlobalParameters<'a, Theme>,
            tree: &mut Tree,
            item: &mut Item<'a, Message, Theme, Renderer>,
            event: &Event,
            layout_iter: &mut impl Iterator<Item = Layout<'b>>,
            cursor: mouse::Cursor,
            renderer: &Renderer,
            clipboard: &mut dyn Clipboard,
            shell: &mut Shell<'_, Message>,
            parent_bounds: Rectangle,
            viewport: &Rectangle,
            prev_bounds_list: &mut Vec<Rectangle>,
            prev_active: &mut Index,
        ) -> RecEvent {
            let Some(menu) = item.menu.as_mut() else {
                return RecEvent::Close;
            };
            let menu_tree = &mut tree.children[1];

            let Some(menu_layout) = layout_iter.next() else {
                return RecEvent::Close;
            }; // menu_node: Node{inf, [ slice_node, items_bounds, offset_bounds]}

            let mut mc = menu_layout.children();
            let slice_layout = mc.next().unwrap(); // slice_node
            let items_bounds = mc.next().unwrap().bounds();
            let background_bounds = pad_rectangle(items_bounds, menu.padding);

            prev_bounds_list.push(background_bounds);

            let menu_state = menu_tree.state.downcast_mut::<MenuState>();

            let rec_event = if let Some(active) = menu_state.active {
                let next_tree = &mut menu_tree.children[active];
                let next_item = &mut menu.items[active];
                let active_in_slice = active - menu_state.slice.start_index;
                let next_parent_bounds = slice_layout
                    .children()
                    .nth(active_in_slice)
                    .expect(
                        "Index (in slice space) is not within the slice layout. \
                        This should not happen, please report this issue"
                    )
                    .bounds();

                rec(
                    global_state,
                    global_parameters,
                    next_tree,
                    next_item,
                    event,
                    layout_iter,
                    cursor,
                    renderer,
                    clipboard,
                    shell,
                    next_parent_bounds,
                    viewport,
                    prev_bounds_list,
                    &mut menu_state.active,
                )
            } else if cursor == mouse::Cursor::Unavailable{
                RecEvent::Event
            } else {
                RecEvent::Close
            };

            prev_bounds_list.pop();

            menu.update(
                global_state,
                global_parameters,
                rec_event,
                menu_tree,
                event,
                menu_layout,
                cursor,
                renderer,
                clipboard,
                shell,
                viewport,
                parent_bounds,
                prev_bounds_list,
                prev_active,
            )
        }

        let re = rec(
            global_state,
            &self.menu_bar.global_parameters,
            active_tree,
            active_root,
            event,
            &mut menu_layouts,
            cursor,
            renderer,
            clipboard,
            shell,
            parent_bounds,
            &viewport,
            &mut prev_bounds_list,
            &mut bar_menu_state.active,
        );

        if matches!(
            event,
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
        ) && matches!(global_state.task(), Some(MenuBarTask::CloseOnClick))
        {
            bar.close(self.tree.children.as_mut_slice(), shell);
            return;
        }

        match re {
            RecEvent::Event => {
                let redraw_event = Event::Window(window::Event::RedrawRequested(Instant::now()));
                let mut fake_messages = vec![];
                let mut fake_shell = Shell::new(&mut fake_messages);

                let Self {
                    menu_bar,
                    layout,
                    tree,
                    ..
                } = self;
                let cursor = {
                    let center = parent_bounds.center();
                    mouse::Cursor::Available(center - self.translation)
                };

                let slice_layout = layout.children().next().unwrap();
                itl_iter_slice!(
                    slice,
                    menu_bar.roots;iter_mut,
                    tree.children;iter_mut,
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
                        &mut fake_shell,
                        &viewport,
                    );
                });
            }
            RecEvent::Close => {
                if !cursor.is_over(bar_bounds) {
                    bar.close(self.tree.children.as_mut_slice(), shell);
                }

                assert!(
                    !shell.is_event_captured(),
                    "MenuBarOverlay::update() | RecEvent::Close | Returning"
                );
                // let the menu bar process the event
            }
            RecEvent::None => {
                assert!(
                    !shell.is_event_captured(),
                    "MenuBarOverlay::update() | RecEvent::None | Returning"
                );
                // let the menu bar process the event
            }
        }
    }

    fn mouse_interaction(
        &self,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let bar = self.tree.state.downcast_ref::<MenuBarState>();
        let MenuBarState {
            global_state,
            menu_state: bar_menu_state,
            ..
        } = bar;

        if !global_state.open {
            return mouse::Interaction::default();
        }

        let Some(active) = bar_menu_state.active else {
            return mouse::Interaction::default();
        };

        let mut lc = layout.children();
        let _bar_bounds = lc.next().unwrap().bounds();
        let _roots_layout = lc.next().unwrap();

        let menu_layouts_layout = lc.next().unwrap(); // Node{0, [menu_node...]}
        let mut menu_layouts = menu_layouts_layout.children(); // [menu_node...]

        let active_root = &self.menu_bar.roots[active];
        let active_tree = &self.tree.children[active];

        fn rec<'a, 'b, Message, Theme: Catalog, Renderer: renderer::Renderer>(
            tree: &Tree,
            item: &Item<'a, Message, Theme, Renderer>,
            layout_iter: &mut impl Iterator<Item = Layout<'b>>,
            cursor: mouse::Cursor,
            renderer: &Renderer,
        ) -> mouse::Interaction {
            let Some(menu) = item.menu.as_ref() else {
                return mouse::Interaction::default();
            };
            let menu_tree = &tree.children[1];

            let Some(menu_layout) = layout_iter.next() else {
                return mouse::Interaction::default();
            }; // menu_node: Node{inf, [ slice_node, items_bounds, offset_bounds]}

            let menu_state = menu_tree.state.downcast_ref::<MenuState>();

            let i = menu.mouse_interaction(menu_tree, menu_layout, cursor, renderer);

            menu_state.active.map_or(i, |active| {
                let next_tree = &menu_tree.children[active];
                let next_item = &menu.items[active];
                rec(next_tree, next_item, layout_iter, cursor, renderer).max(i)
            })
        }

        rec(
            active_tree,
            active_root,
            &mut menu_layouts,
            cursor,
            renderer,
        )
    }

    fn operate(
        &mut self,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation<()>,
    ) {
        let bar = self.tree.state.downcast_ref::<MenuBarState>();
        let MenuBarState {
            global_state,
            menu_state: bar_menu_state,
            ..
        } = bar;

        if !global_state.open {
            return;
        }

        let Some(active) = bar_menu_state.active else {
            return;
        };

        let mut lc = layout.children();
        let _bar_bounds = lc.next().unwrap().bounds();
        let _roots_layout = lc.next().unwrap();

        let menu_layouts_layout = lc.next().unwrap(); // Node{0, [menu_node...]}
        let mut menu_layouts = menu_layouts_layout.children(); // [menu_node...]

        let active_root = &mut self.menu_bar.roots[active];
        let active_tree = &mut self.tree.children[active];

        fn rec<'a, 'b, Message, Theme: Catalog, Renderer: renderer::Renderer>(
            tree: &mut Tree,
            item: &mut Item<'a, Message, Theme, Renderer>,
            layout_iter: &mut impl Iterator<Item = Layout<'b>>,
            renderer: &Renderer,
            operation: &mut dyn Operation<()>,
        ) {
            let Some(menu) = item.menu.as_mut() else {
                return;
            };

            let menu_tree = &mut tree.children[1];

            let Some(menu_layout) = layout_iter.next() else {
                return;
            };

            menu.operate(menu_tree, menu_layout, renderer, operation);

            operation.container(None, menu_layout.bounds());
            operation.traverse(&mut |operation| {
                menu.items
                    .iter_mut() // [Item...]
                    .zip(menu_tree.children.iter_mut()) // [item_tree...] // [widget_node...]
                    .for_each(|(child, state)| {
                        rec(state, child, layout_iter, renderer, operation);
                    });
            });
        }

        rec(
            active_tree,
            active_root,
            &mut menu_layouts,
            renderer,
            operation,
        );
    }

    fn overlay<'c>(
        &'c mut self,
        layout: Layout<'c>,
        renderer: &Renderer,
    ) -> Option<overlay::Element<'c, Message, Theme, Renderer>> {
        let Tree {
            state,
            children: item_trees,
            ..
        } = self.tree;
        let bar = state.downcast_ref::<MenuBarState>();
        let MenuBarState {
            global_state,
            menu_state: bar_menu_state,
            ..
        } = bar;
        let slice = bar_menu_state.slice;

        if !global_state.open {
            return None;
        }

        let active = bar_menu_state.active?;

        let mut lc = layout.children();
        let viewport = layout.bounds();
        let _bar_bounds = lc.next()?.bounds();
        let _roots_layout = lc.next()?;
        let menu_layouts_layout = lc.next()?; // Node{0, [menu_node...]}
        let mut menu_layouts = menu_layouts_layout.children(); // [menu_node...]

        fn rec<'a, 'b, Message, Theme: Catalog, Renderer: renderer::Renderer>(
            items: &'b mut [Item<'a, Message, Theme, Renderer>],
            menu_tree: &'b mut Tree, // Tree{ menu_state, [item_tree...] }
            menu_layouts: &mut impl Iterator<Item = Layout<'b>>, // [menu_node...]
            overlays: &mut Vec<overlay::Element<'b, Message, Theme, Renderer>>,
            renderer: &Renderer,
            viewport: &Rectangle,
        ) {
            let menu_state = menu_tree.state.downcast_mut::<MenuState>();
            let menu_layout = menu_layouts.next().unwrap(); // menu_node: Node{inf, [ slice_node, items_bounds, offset_bounds]}
            let mut mlc = menu_layout.children();
            let slice_layout = mlc.next().unwrap(); // slice_node: Node{inf, [item_node...]}

            let slice = menu_state.slice;

            if let Some(active) = menu_state.active {
                let mut next = None;

                for (i, ((item, item_tree), item_layout)) in itl_iter_slice_enum!(
                    slice,
                    items;iter_mut,
                    menu_tree.children;iter_mut,
                    slice_layout.children()
                ) {
                    let Item {
                        item: item_widget,
                        menu: item_menu,
                        ..
                    } = item;

                    let item_widget_tree = if i == active {
                        let [item_widget_tree, item_menu_tree] = item_tree.children.as_mut_slice()
                        else {
                            continue;
                        };
                        next = Some((item_menu.as_mut().unwrap(), item_menu_tree));
                        item_widget_tree
                    } else {
                        &mut item_tree.children.as_mut_slice()[0]
                    };

                    if let Some(overlay) = item_widget.as_widget_mut().overlay(
                        item_widget_tree,
                        item_layout,
                        renderer,
                        viewport,
                        Vector::ZERO,
                    ) {
                        overlays.push(overlay);
                    }
                }

                if let Some((next_menu, next_menu_tree)) = next {
                    rec(
                        &mut next_menu.items,
                        next_menu_tree,
                        menu_layouts,
                        overlays,
                        renderer,
                        viewport,
                    );
                }
            } else {
                for ((item, item_tree), item_layout) in itl_iter_slice!(
                    slice,
                    items;iter_mut,
                    menu_tree.children;iter_mut,
                    slice_layout.children()
                ) {
                    let Item {
                        item: item_widget, ..
                    } = item;

                    if let Some(overlay) = item_widget.as_widget_mut().overlay(
                        &mut item_tree.children[0],
                        item_layout,
                        renderer,
                        viewport,
                        Vector::ZERO,
                    ) {
                        overlays.push(overlay);
                    }
                }
            }
        }

        let mut overlays = vec![];
        let mut next = None;

        let slice_layout = self.layout.children().next()?;

        for (i, ((item, item_tree), item_layout)) in itl_iter_slice_enum!(
            slice,
            self.menu_bar.roots;iter_mut,
            item_trees;iter_mut,
            slice_layout.children()
        ) {
            let Item {
                item: item_widget,
                menu: item_menu,
                ..
            } = item;
            let [item_widget_tree, item_menu_tree] = item_tree.children.as_mut_slice() else {
                continue;
            };

            if i == active
                && let Some(menu) = item_menu.as_mut()
            {
                next = Some((menu, item_menu_tree));
            }
            if let Some(overlay) = item_widget.as_widget_mut().overlay(
                item_widget_tree,
                item_layout,
                renderer,
                &viewport,
                self.translation,
            ) {
                overlays.push(overlay);
            }
        }

        if let Some((next_menu, next_menu_tree)) = next {
            rec(
                &mut next_menu.items,
                next_menu_tree,
                &mut menu_layouts,
                &mut overlays,
                renderer,
                &viewport,
            );
        }

        if overlays.is_empty() {
            None
        } else {
            Some(overlay::Group::with_children(overlays).overlay())
        }
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
    ) {
        let bar = self.tree.state.downcast_ref::<MenuBarState>();
        let MenuBarState {
            global_state,
            menu_state: bar_menu_state,
            ..
        } = bar;

        if !global_state.open {
            return;
        }

        let Some(active) = bar_menu_state.active else {
            return;
        };

        let viewport = layout.bounds();
        let mut lc = layout.children();
        let _bar_bounds = lc.next().unwrap().bounds();
        let _roots_layout = lc.next().unwrap();

        let menu_layouts_layout = lc.next().unwrap(); // Node{0, [menu_node...]}
        let mut menu_layouts = menu_layouts_layout.children(); // [menu_node...]

        let active_root = &self.menu_bar.roots[active];
        let active_tree = &self.tree.children[active];

        fn rec<'a, 'b, Message, Theme: Catalog, Renderer: renderer::Renderer>(
            global_parameters: &GlobalParameters<'a, Theme>,
            tree: &Tree,
            item: &Item<'a, Message, Theme, Renderer>,
            layout_iter: &mut impl Iterator<Item = Layout<'b>>,
            cursor: mouse::Cursor,
            renderer: &mut Renderer,
            theme: &Theme,
            style: &renderer::Style,
            theme_style: &Style,
            viewport: &Rectangle,
        ) {
            let Some(menu) = item.menu.as_ref() else {
                return;
            };

            let menu_tree = &tree.children[1];

            let Some(menu_layout) = layout_iter.next() else {
                return;
            }; // menu_node: Node{inf, [ slice_node, items_bounds, offset_bounds]}

            let menu_state = menu_tree.state.downcast_ref::<MenuState>();

            menu.draw(
                &global_parameters.path_highlight,
                menu_tree,
                renderer,
                theme,
                style,
                theme_style,
                menu_layout,
                cursor,
                viewport,
            );

            if let Some(active) = menu_state.active {
                let next_tree = &menu_tree.children[active];
                let next_item = &menu.items[active];

                renderer.with_layer(*viewport, |r| {
                    rec(
                        global_parameters,
                        next_tree,
                        next_item,
                        layout_iter,
                        cursor,
                        r,
                        theme,
                        style,
                        theme_style,
                        viewport,
                    );
                });
            }
        }

        let theme_style = <Theme as Catalog>::style(theme, &self.menu_bar.global_parameters.class, Status::Selected);

        rec(
            &self.menu_bar.global_parameters,
            active_tree,
            active_root,
            &mut menu_layouts,
            cursor,
            renderer,
            theme,
            style,
            &theme_style,
            &viewport,
        );
    }
}
