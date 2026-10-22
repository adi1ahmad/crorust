use core::mem;

use accesskit::ActionRequest;
use accesskit_consumer::{Node, NodeId, Tree, TreeChangeHandler};

use egui::{
    Context, Event, Frame, FullOutput, Id, Key, KeyboardShortcut,
    Modifiers, Panel, RawInput, ScrollArea, Ui, collapsing_header::CollapsingState,
};

#[derive(Default, Debug)]
pub struct AccessibilityPlugin {
    pub open: bool,
    tree: Option<Tree>,
    selected_node: Option<NodeId>,
    queued_action: Option<ActionRequest>,
}

struct ChangeHandler;

impl TreeChangeHandler for ChangeHandler {
    fn node_added(&mut self, _node: &Node<'_>) {}
    fn node_updated(&mut self, _old_node: &Node<'_>, _new_node: &Node<'_>) {}
    fn focus_moved(&mut self, _old_node: Option<&Node<'_>>, _new_node: Option<&Node<'_>>) {}
    fn node_removed(&mut self, _node: &Node<'_>) {}
}

impl egui::Plugin for AccessibilityPlugin {
    fn debug_name(&self) -> &'static str {
        "Accessibility Inspector"
    }

    fn input_hook(&mut self, _ctx: &Context, input: &mut RawInput) {
        if let Some(queued_action) = self.queued_action.take() {
            input
                .events
                .push(Event::AccessKitActionRequest(queued_action));
        }
    }

    fn output_hook(&mut self, _ctx: &Context, output: &mut FullOutput) {
        if let Some(update) = output.platform_output.accesskit_update.clone() {
            self.tree = match mem::take(&mut self.tree) {
                None => Some(Tree::new(update, true)),
                Some(mut tree) => {
                    tree.update_and_process_changes(update, &mut ChangeHandler);
                    Some(tree)
                }
            };
        }
    }

    fn on_begin_pass(&mut self, ui: &mut Ui) {
        if ui.input_mut(|i| {
            i.consume_shortcut(&KeyboardShortcut::new(
                Modifiers::COMMAND | Modifiers::ALT,
                Key::I,
            ))
        }) {
            self.open = !self.open;
        }

        if !self.open {
            return;
        }

        ui.enable_accesskit();

        Panel::right("accessibility_inspector").show(ui, |ui| {
            ui.heading("🔎 AccessKit Inspector");
            if let Some(selected_node) = self.selected_node {
                Panel::bottom("details_panel")
                    .frame(Frame::new())
                    .show_separator_line(false)
                    .show(ui, |ui| {
                        self.selection_ui(ui, selected_node);
                    });
            }

            ScrollArea::vertical().show(ui, |ui| {
                if let Some(tree) = &self.tree {
                    let root = tree.state().root();
                    Self::show_node(&mut self.selected_node, ui, &root);
                } else {
                    ui.label("No AccessKit tree available yet.");
                }
            });
        });
    }
}

impl AccessibilityPlugin {
    fn show_node(selected_node: &mut Option<NodeId>, ui: &mut Ui, node: &Node<'_>) {
        let is_selected = *selected_node == Some(node.id());
        let label = format!("{:?}: {:?}", node.role(), node.label().unwrap_or_default());
        let id = Id::new(node.id());

        let mut state = CollapsingState::load_with_default_open(ui.ctx(), id, false);
        let header_response = ui.horizontal(|ui| {
            if node.children().count() > 0 {
                state.show_toggle_button(ui, |ui, open, _response| {
                    if open > 0.0 {
                        ui.label("▼");
                    } else {
                        ui.label("▶");
                    }
                });
            }
            let response = ui.selectable_label(is_selected, label);
            if response.clicked() {
                *selected_node = Some(node.id());
            }
            response
        });

        state.show_body_indented(&header_response.response, ui, |ui| {
            for child in node.children() {
                Self::show_node(selected_node, ui, &child);
            }
        });
    }

    fn selection_ui(&mut self, ui: &mut Ui, node_id: NodeId) {
        ui.group(|ui| {
            ui.heading("Node Details");
            if let Some(tree) = &self.tree {
                if let Some(node) = tree.state().node_by_id(node_id) {
                    ui.label(format!("ID: {:?}", node.id()));
                    ui.label(format!("Role: {:?}", node.role()));
                    if let Some(label) = node.label() {
                        ui.label(format!("Label: {label}"));
                    }
                    if let Some(value) = node.value() {
                        ui.label(format!("Value: {value}"));
                    }
                } else {
                    ui.label("Node no longer exists in tree.");
                }
            }
        });
    }
}
