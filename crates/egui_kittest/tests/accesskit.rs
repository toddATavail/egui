//! Tests the accesskit accessibility output of egui.

use egui::{
    CentralPanel, Context, RawInput, Ui, Window,
    accesskit::{NodeId, Role, TreeUpdate},
};
use egui_kittest::{Harness, kittest::Queryable as _};

/// Baseline test that asserts there are no spurious nodes in the
/// accesskit output when the ui is empty.
///
/// This gives reasonable certainty that any nodes appearing in the other accesskit outputs
/// are put there because of the widgets rendered.
#[test]
fn empty_ui_should_return_tree_with_only_root_window() {
    let output = accesskit_output_single_egui_frame(|_ui| {
        // Nothing here beyond the default empty UI
    });

    assert_eq!(
        output.nodes.len(),
        2,
        "Expected the root node and the top level Ui; found: {output:#?}",
    );

    assert_eq!(
        output
            .nodes
            .iter()
            .filter(|(_, n)| n.role() == Role::GenericContainer)
            .count(),
        1,
        "Expected a single Ui as a GenericContainer node.",
    );

    let (id, root) = &output.nodes[0];

    assert_eq!(*id, output.tree.unwrap().root);
    assert_eq!(root.role(), Role::Window);
}

#[test]
fn button_node() {
    let button_text = "This is a test button!";

    let output = accesskit_output_single_egui_frame(|ui| {
        CentralPanel::default().show(ui, |ui| ui.button(button_text));
    });

    let (_, button) = output
        .nodes
        .iter()
        .find(|(_, node)| node.role() == Role::Button)
        .expect("Button should exist in the accesskit output");

    assert_eq!(button.label(), Some(button_text));
    assert!(!button.is_disabled());
}

#[test]
fn disabled_button_node() {
    let button_text = "This is a test button!";

    let output = accesskit_output_single_egui_frame(|ui| {
        CentralPanel::default().show(ui, |ui| {
            ui.add_enabled(false, egui::Button::new(button_text))
        });
    });

    let (_, button) = output
        .nodes
        .iter()
        .find(|(_, node)| node.role() == Role::Button)
        .expect("Button should exist in the accesskit output");

    assert_eq!(button.label(), Some(button_text));
    assert!(button.is_disabled());
}

#[test]
fn toggle_button_node() {
    let button_text = "A toggle button";

    let mut selected = false;
    let output = accesskit_output_single_egui_frame(|ui| {
        CentralPanel::default().show(ui, |ui| ui.toggle_value(&mut selected, button_text));
    });

    let (_, toggle) = output
        .nodes
        .iter()
        .find(|(_, node)| node.role() == Role::Button)
        .expect("Toggle button should exist in the accesskit output");

    assert_eq!(toggle.label(), Some(button_text));
    assert!(!toggle.is_disabled());
}

#[test]
fn multiple_disabled_widgets() {
    let output = accesskit_output_single_egui_frame(|ui| {
        CentralPanel::default().show(ui, |ui| {
            ui.add_enabled_ui(false, |ui| {
                let _ = ui.button("Button 1");
                let _ = ui.button("Button 2");
                let _ = ui.button("Button 3");
            })
        });
    });

    assert_eq!(
        output
            .nodes
            .iter()
            .filter(|(_, node)| node.is_disabled())
            .count(),
        3,
        "All widgets should be disabled."
    );
}

#[test]
fn window_children() {
    let output = accesskit_output_single_egui_frame(|ui| {
        let mut open = true;
        Window::new("test window")
            .open(&mut open)
            .resizable(false)
            .show(ui.ctx(), |ui| {
                let _ = ui.button("A button");
            });
    });

    let root = output.tree.as_ref().map(|tree| tree.root).unwrap();

    let window_id = assert_window_exists(&output, "test window", root);
    assert_button_exists(&output, "A button", window_id);
    assert_button_exists(&output, "Close window", window_id);
    assert_button_exists(&output, "Hide", window_id);
}

fn accesskit_output_single_egui_frame(run_ui: impl FnMut(&mut Ui)) -> TreeUpdate {
    let ctx = Context::default();
    // Disable animations, so we do not need to wait for animations to end to see the result.
    ctx.global_style_mut(|style| style.animation_time = 0.0);
    ctx.enable_accesskit();

    let mut output = ctx.run_ui(RawInput::default(), run_ui);
    output.textures_delta.clear(); // Don't panic on drop with unapplied deltas

    output
        .platform_output
        .accesskit_update
        .expect("Missing accesskit update")
}

#[track_caller]
fn assert_button_exists(tree: &TreeUpdate, label: &str, parent: NodeId) {
    let (node_id, _) = tree
        .nodes
        .iter()
        .find(|(_, node)| {
            !node.is_hidden() && node.role() == Role::Button && node.label() == Some(label)
        })
        .expect("No visible button with that label exists.");

    assert_parent_child(tree, parent, *node_id);
}

#[track_caller]
fn assert_window_exists(tree: &TreeUpdate, title: &str, parent: NodeId) -> NodeId {
    let (node_id, _) = tree
        .nodes
        .iter()
        .find(|(_, node)| {
            !node.is_hidden() && node.role() == Role::Window && node.label() == Some(title)
        })
        .expect("No visible window with that title exists.");

    assert_parent_child(tree, parent, *node_id);

    *node_id
}

#[track_caller]
fn assert_parent_child(tree: &TreeUpdate, parent_id: NodeId, child: NodeId) {
    assert!(
        has_child_recursively(tree, parent_id, child),
        "Node is not a child of the given parent."
    );
}

fn has_child_recursively(tree: &TreeUpdate, parent: NodeId, child: NodeId) -> bool {
    let (_, parent) = tree
        .nodes
        .iter()
        .find(|(id, _)| id == &parent)
        .expect("Parent does not exist.");

    for &c in parent.children() {
        if c == child || has_child_recursively(tree, c, child) {
            return true;
        }
    }

    false
}

/// A scroll target inside a `ScrollArea` that cannot scroll in that direction belongs to the
/// nearest enclosing `ScrollArea` that can.
///
/// `egui_extras::Table` with `vscroll(false)` builds exactly such an area around its body.
#[test]
fn scroll_into_view_passes_through_a_scroll_area_that_cannot_scroll() {
    let mut harness = Harness::builder()
        .with_size(egui::vec2(400.0, 300.0))
        .build_ui_state(
            |ui, rect: &mut egui::Rect| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.allocate_space(egui::vec2(ui.available_width(), 1500.0));
                    egui::ScrollArea::new([false, false]).show(ui, |ui| {
                        *rect = ui.button("Target").rect;
                    });
                });
            },
            egui::Rect::NOTHING,
        );
    harness.run();

    harness.get_by_label("Target").scroll_to_me();
    harness.run();

    let rect = *harness.state();
    assert!(
        0.0 <= rect.min.y && rect.max.y <= 300.0,
        "not scrolled into view: {rect:?}"
    );
}

/// The vertical half of a scroll target inside a horizontal `ScrollArea` belongs to the
/// vertical `ScrollArea` around it.
#[test]
fn scroll_into_view_passes_through_a_horizontal_scroll_area() {
    let mut harness = Harness::builder()
        .with_size(egui::vec2(400.0, 300.0))
        .build_ui_state(
            |ui, rect: &mut egui::Rect| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.allocate_space(egui::vec2(ui.available_width(), 1500.0));
                    egui::ScrollArea::horizontal().show(ui, |ui| {
                        *rect = ui.button("Target").rect;
                    });
                });
            },
            egui::Rect::NOTHING,
        );
    harness.run();

    harness.get_by_label("Target").scroll_to_me();
    harness.run();

    let rect = *harness.state();
    assert!(
        0.0 <= rect.min.y && rect.max.y <= 300.0,
        "not scrolled into view: {rect:?}"
    );
}

/// A scroll target passed up by a `ScrollArea` is not taken by a sibling `ScrollArea` shown
/// after it.
#[test]
fn a_passed_up_scroll_target_is_not_taken_by_a_sibling() {
    let mut harness = Harness::builder()
        .with_size(egui::vec2(400.0, 300.0))
        .build_ui_state(
            |ui, rects: &mut [egui::Rect; 2]| {
                egui::ScrollArea::vertical()
                    .id_salt("outer")
                    .show(ui, |ui| {
                        ui.allocate_space(egui::vec2(ui.available_width(), 1500.0));
                        egui::ScrollArea::new([false, false])
                            .id_salt("fixed")
                            .show(ui, |ui| {
                                rects[0] = ui.button("Target").rect;
                            });
                        egui::ScrollArea::vertical()
                            .id_salt("sibling")
                            .max_height(50.0)
                            .show(ui, |ui| {
                                ui.allocate_space(egui::vec2(ui.available_width(), 500.0));
                                rects[1] = ui.label("Sibling top").rect;
                            });
                    });
            },
            [egui::Rect::NOTHING; 2],
        );
    harness.run();
    let sibling_top = harness.state()[1].min.y;
    let target_top = harness.state()[0].min.y;

    harness.get_by_label("Target").scroll_to_me();
    harness.run();

    let [target, sibling] = *harness.state();
    assert!(
        0.0 <= target.min.y && target.max.y <= 300.0,
        "not scrolled into view: {target:?}"
    );
    assert_eq!(
        sibling.min.y - sibling_top,
        target.min.y - target_top,
        "the sibling scrolled by itself rather than with the outer area"
    );
}

/// A scroll target passed up with no enclosing `ScrollArea` to take it does not scroll a
/// sibling `ScrollArea` instead (the layout from <https://github.com/emilk/egui/issues/985>).
#[test]
fn a_passed_up_scroll_target_with_no_enclosing_scroll_area_scrolls_no_sibling() {
    let mut harness = Harness::builder()
        .with_size(egui::vec2(400.0, 300.0))
        .build_ui_state(
            |ui, rects: &mut [egui::Rect; 2]| {
                egui::ScrollArea::horizontal()
                    .id_salt("strip")
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.allocate_space(egui::vec2(1500.0, 20.0));
                            rects[0] = ui.button("Target").rect;
                        });
                    });
                egui::ScrollArea::vertical()
                    .id_salt("sibling")
                    .show(ui, |ui| {
                        rects[1] = ui.label("Sibling top").rect;
                        ui.allocate_space(egui::vec2(ui.available_width(), 1500.0));
                    });
            },
            [egui::Rect::NOTHING; 2],
        );
    harness.run();
    let sibling_before = harness.state()[1];

    harness.get_by_label("Target").scroll_to_me();
    harness.run();

    let [target, sibling] = *harness.state();
    assert!(
        0.0 <= target.min.x && target.max.x <= 400.0,
        "not scrolled into view: {target:?}"
    );
    assert_eq!(sibling, sibling_before, "the sibling scrolled");
}
