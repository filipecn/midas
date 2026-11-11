use crossterm::event::KeyCode;
use ratatui::widgets::Widget;
use ratatui_interact::components::{TreeNode, TreeStyle, TreeView, TreeViewState, get_selected_id};
use regex::Regex;

use crate::{
    budget::{Category, data::Categories},
    widgets::{ActionResult, InteractableWidget},
};

/// Data for tree nodes
#[derive(Debug, Clone)]
struct CategoryEntry {
    name: String,
    is_parent: bool,
}

pub struct CategoryView {
    tree_nodes: Vec<TreeNode<CategoryEntry>>,
    tree_state: TreeViewState,
}

impl CategoryView {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        CategoryView::filter(String::new().as_str())
    }
    pub fn filter(re: &str) -> Result<Self, Box<dyn std::error::Error>> {
        CategoryView::filter_by_type(re, None)
    }
    pub fn filter_by_type(
        re: &str,
        category_type: Option<&str>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let regex = if re.is_empty() {
            None
        } else {
            Some(Regex::new(&re.to_uppercase()).unwrap())
        };
        let mut tree_nodes: Vec<TreeNode<CategoryEntry>> = Vec::new();
        for category in Categories::top_categories() {
            if category_type.is_some_and(|category_type| category.category_type != category_type) {
                continue;
            }
            let subcategories = Categories::subcategories(&category.name);
            let mut node_children: Vec<TreeNode<CategoryEntry>> = Vec::new();

            for subcategory in subcategories {
                if let Some(reg) = regex.as_ref() {
                    if !reg.is_match(&subcategory.name.to_uppercase()) {
                        continue;
                    }
                }
                node_children.push(TreeNode::new(
                    subcategory.name.clone(),
                    CategoryEntry {
                        name: subcategory.name,
                        is_parent: false,
                    },
                ));
            }
            if !node_children.is_empty() {
                tree_nodes.push(
                    TreeNode::new(
                        category.name.clone(),
                        CategoryEntry {
                            name: category.name,
                            is_parent: true,
                        },
                    )
                    .with_children(node_children),
                );
            }
        }

        Ok(CategoryView {
            tree_nodes,
            tree_state: TreeViewState::new(),
        })
    }

    pub fn selected_category(&self) -> Option<Category> {
        if let Some(selected_node) = get_selected_id(&self.tree_nodes, &self.tree_state) {
            if let Some(category) = Categories::get(selected_node.as_str()) {
                if let Some(parent) = &category.parent {
                    Some(Category {
                        parent: Some(parent.clone()),
                        name: category.name.clone(),
                        category_type: category.category_type.clone(),
                        frequency: category.frequency.clone(),
                        description: category.description.clone(),
                    })
                } else {
                    Some(Category {
                        name: category.name.clone(),
                        category_type: category.category_type.clone(),
                        frequency: category.frequency.clone(),
                        parent: None,
                        description: category.description.clone(),
                    })
                }
            } else {
                None
            }
        } else {
            None
        }
    }

    fn visible_tree_count(&self) -> usize {
        let tree = TreeView::new(&self.tree_nodes, &self.tree_state);
        tree.visible_count()
    }
}

impl InteractableWidget for CategoryView {
    fn render(&self, area: ratatui::layout::Rect, frame: &mut ratatui::Frame) {
        let style = TreeStyle::default();

        let tree = TreeView::new(&self.tree_nodes, &self.tree_state)
            .style(style)
            .render_item(|node, _is_selected| {
                let icon = if node.data.is_parent {
                    if self.tree_state.is_collapsed(&node.id) {
                        ""
                    } else {
                        ""
                    }
                } else {
                    match node.data.name.split('.').last() {
                        Some("rs") => "",
                        Some("toml") => "",
                        Some("md") => "",
                        _ => "",
                    }
                };

                format!("{} {}", icon, node.data.name)
            });

        tree.render(area, frame.buffer_mut());
    }

    fn handle_key(&mut self, key: &crossterm::event::KeyEvent) -> ActionResult {
        let mut result = ActionResult::Consumed;
        match key.code {
            KeyCode::Enter => {
                if let Some(category) = self.selected_category() {
                    result = ActionResult::Category(category);
                }
            }
            KeyCode::Up => {
                self.tree_state.select_prev();
                self.tree_state.ensure_visible(10);
            }
            KeyCode::Down => {
                let count = self.visible_tree_count();
                self.tree_state.select_next(count);
                self.tree_state.ensure_visible(10);
            }
            KeyCode::Left => {
                if let Some(id) = get_selected_id(&self.tree_nodes, &self.tree_state) {
                    self.tree_state.collapse(&id);
                }
            }
            KeyCode::Right => {
                if let Some(id) = get_selected_id(&self.tree_nodes, &self.tree_state) {
                    self.tree_state.expand(&id);
                }
            }
            _ => result = ActionResult::None,
        };
        result
    }
}
