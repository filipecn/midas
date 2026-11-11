use std::collections::HashMap;

use crossterm::event::KeyCode;
use ratatui::{
    layout::{Constraint, Layout},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Cell, Paragraph, Row, Table, Widget},
};
use ratatui_interact::components::{TreeNode, TreeStyle, TreeView, TreeViewState, get_selected_id};

use crate::{
    budget::{Category, data::Categories},
    tui::TUI,
    widgets::{ActionResult, InteractableWidget},
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExpenseGroupEntry {
    pub name: String,
    pub actual: i64,
    pub planned: i64,
    pub is_parent: bool,
}

pub struct ExpenseGroupTree {
    pub tree_nodes: Vec<TreeNode<ExpenseGroupEntry>>,
    pub tree_state: TreeViewState,
    total_spent: u64,
}

fn spending_percentage(amount_in_cents: i64, total_spent: u64) -> f64 {
    if total_spent == 0 {
        0.0
    } else {
        amount_in_cents.unsigned_abs() as f64 / total_spent as f64 * 100.0
    }
}

impl ExpenseGroupTree {
    pub fn update(
        &mut self,
        amounts: &HashMap<String, i64>,
        planned_amounts: &HashMap<String, i64>,
        frequency: &str,
        total_spent: u64,
    ) {
        self.total_spent = total_spent;
        let mut tree_nodes: Vec<TreeNode<ExpenseGroupEntry>> = Vec::new();
        let expense_groups = Categories::get_expense_groups();
        for expense_group in expense_groups {
            let mut node_children: Vec<TreeNode<ExpenseGroupEntry>> = Vec::new();
            for subcategory in &expense_group.1 {
                if subcategory.frequency != frequency {
                    continue;
                }
                if amounts.get(&subcategory.name).is_none() {
                    continue; // Skip this subcategory if it has no transactions
                }
                node_children.push(TreeNode::new(
                    subcategory.name.clone(),
                    ExpenseGroupEntry {
                        actual: *amounts.get(&subcategory.name).unwrap_or(&0),
                        planned: *planned_amounts.get(&subcategory.name).unwrap_or(&0),
                        name: subcategory.name.clone(),
                        is_parent: false,
                    },
                ));
            }
            let all_subcategory_actual: i64 = expense_group
                .1
                .iter()
                .map(|subcategory| amounts.get(&subcategory.name).copied().unwrap_or(0))
                .sum();
            let all_subcategory_planned: i64 = expense_group
                .1
                .iter()
                .map(|subcategory| planned_amounts.get(&subcategory.name).copied().unwrap_or(0))
                .sum();
            let include_group_amount = Categories::get(&expense_group.0)
                .as_ref()
                .is_some_and(|category| category.frequency == frequency);
            let direct_actual = if include_group_amount {
                amounts.get(&expense_group.0).copied().unwrap_or(0) - all_subcategory_actual
            } else {
                0
            };
            if !node_children.is_empty() || direct_actual != 0 {
                let matching_actual: i64 = node_children.iter().map(|node| node.data.actual).sum();
                let actual = matching_actual + direct_actual;
                let planned = expense_group
                    .1
                    .iter()
                    .filter(|subcategory| subcategory.frequency == frequency)
                    .map(|subcategory| planned_amounts.get(&subcategory.name).copied().unwrap_or(0))
                    .sum::<i64>()
                    + if include_group_amount {
                        planned_amounts
                            .get(&expense_group.0)
                            .copied()
                            .unwrap_or(0)
                            .saturating_sub(all_subcategory_planned)
                    } else {
                        0
                    };
                tree_nodes.push(
                    TreeNode::new(
                        expense_group.0.clone(),
                        ExpenseGroupEntry {
                            actual,
                            planned,
                            name: expense_group.0.clone(),
                            is_parent: true,
                        },
                    )
                    .with_children(node_children),
                );
            }
        }
        self.tree_state.collapsed = tree_nodes.iter().map(|node| node.id.clone()).collect();
        self.tree_nodes = tree_nodes;

        if self.tree_nodes.is_empty() {
            self.tree_state.selected_index = 0;
            return;
        }

        let max_index = self.visible_tree_count().saturating_sub(1);
        if self.tree_state.selected_index > max_index {
            self.tree_state.selected_index = 0;
        }
        if get_selected_id(&self.tree_nodes, &self.tree_state).is_none() {
            self.tree_state.selected_index = 0;
        }
    }
    pub fn new() -> Self {
        ExpenseGroupTree {
            tree_nodes: Vec::new(),
            tree_state: TreeViewState::default(),
            total_spent: 0,
        }
    }
    fn visible_tree_count(&self) -> usize {
        let tree = TreeView::new(&self.tree_nodes, &self.tree_state);
        tree.visible_count()
    }
    fn visible_nodes<'a>(
        nodes: &'a [TreeNode<ExpenseGroupEntry>],
        state: &TreeViewState,
        visible: &mut Vec<&'a TreeNode<ExpenseGroupEntry>>,
    ) {
        for node in nodes {
            visible.push(node);
            if node.has_children() && !state.is_collapsed(&node.id) {
                Self::visible_nodes(&node.children, state, visible);
            }
        }
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

    pub fn totals(&self) -> (i64, i64) {
        self.tree_nodes
            .iter()
            .map(|node| (node.data.planned, node.data.actual))
            .fold((0, 0), |(budget, spent), (node_budget, node_spent)| {
                (budget + node_budget, spent + node_spent)
            })
    }
}

impl InteractableWidget for ExpenseGroupTree {
    fn render(&self, area: ratatui::layout::Rect, frame: &mut ratatui::Frame) {
        let palette = TUI::color_palette();
        let style = TreeStyle::default();
        let value_width = 10;
        let percent_width = 7;
        let [header_area, tree_area] =
            Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(area);
        let [names_area, values_area] =
            Layout::horizontal([Constraint::Percentage(35), Constraint::Min(1)]).areas(tree_area);
        let [header_names_area, header_values_area] =
            Layout::horizontal([Constraint::Percentage(35), Constraint::Min(1)]).areas(header_area);
        frame.render_widget(Paragraph::new("Category"), header_names_area);
        let header = Line::from(vec![
            Span::styled(
                format!("{:>value_width$} ", "Budget", value_width = value_width),
                Style::default().fg(palette.budget),
            ),
            Span::styled(
                format!("{:>value_width$} ", "Spent", value_width = value_width),
                Style::default().fg(palette.spent),
            ),
            Span::raw(format!(
                "{:>percent_width$} ",
                "%",
                percent_width = percent_width
            )),
            Span::raw(format!(
                "{:>value_width$} ",
                "Net",
                value_width = value_width
            )),
            Span::raw(format!(
                "{:>value_width$}",
                "Usage",
                value_width = value_width
            )),
        ]);
        frame.render_widget(Paragraph::new(header), header_values_area);

        let tree = TreeView::new(&self.tree_nodes, &self.tree_state)
            .style(style.clone())
            .render_item(|node, _is_selected| node.data.name.clone());

        tree.render(names_area, frame.buffer_mut());

        let mut visible_nodes = Vec::new();
        Self::visible_nodes(&self.tree_nodes, &self.tree_state, &mut visible_nodes);
        let rows = visible_nodes
            .into_iter()
            .skip(self.tree_state.scroll as usize)
            .take(values_area.height as usize)
            .enumerate()
            .map(|(index, node)| {
                let is_selected =
                    index + self.tree_state.scroll as usize == self.tree_state.selected_index;
                let row_style = if is_selected {
                    style.selected_style
                } else {
                    style.normal_style
                };
                let (budget, spent, net) = if node.data.is_parent {
                    (
                        node.data.planned as f64 / 100.0,
                        node.data.actual as f64 / 100.0,
                        (node.data.planned + node.data.actual) as f64 / 100.0,
                    )
                } else {
                    (0.0, node.data.actual as f64 / 100.0, 0.0)
                };
                let hide_values = !TUI::sensitive_values_visible();
                let budget_text = if hide_values {
                    TUI::format_amount_for_display(node.data.planned, value_width)
                } else if budget == 0.0 {
                    String::new()
                } else {
                    format!("{budget:.2}")
                };
                let net_text = if node.data.is_parent {
                    if hide_values {
                        TUI::format_amount_for_display(
                            node.data.planned + node.data.actual,
                            value_width,
                        )
                    } else {
                        format!("{net:.2}")
                    }
                } else {
                    String::new()
                };
                let usage = if node.data.is_parent && budget != 0.0 {
                    (spent / budget).abs() * 100.0
                } else {
                    0.0
                };
                let spending_percentage = spending_percentage(node.data.actual, self.total_spent);
                let spending_percentage_text = format!("{spending_percentage:>percent_width$.2}%");
                let usage_text = if node.data.is_parent && budget != 0.0 {
                    format!("{usage:.2}%")
                } else {
                    String::new()
                };
                let net_color = if usage < 100.0 {
                    palette.green
                } else if usage > 100.0 {
                    palette.spent
                } else {
                    Color::Yellow
                };
                let usage_color = if usage < 100.0 {
                    palette.green
                } else if usage > 100.0 {
                    palette.spent
                } else {
                    Color::Yellow
                };
                let spent_text = if hide_values {
                    TUI::format_amount_for_display(node.data.actual, value_width)
                } else {
                    format!("{spent:>value_width$.2}")
                };
                Row::new([
                    Cell::from(format!("{budget_text:>value_width$}"))
                        .style(row_style.fg(palette.budget)),
                    Cell::from(spent_text).style(row_style.fg(palette.spent)),
                    Cell::from(spending_percentage_text).style(row_style.fg(palette.spent)),
                    Cell::from(format!("{net_text:>value_width$}")).style(row_style.fg(net_color)),
                    Cell::from(format!("{usage_text:>value_width$}"))
                        .style(row_style.fg(usage_color)),
                ])
            });
        let table = Table::new(
            rows,
            [
                Constraint::Length(value_width as u16),
                Constraint::Length(value_width as u16),
                Constraint::Length(percent_width as u16),
                Constraint::Length(value_width as u16),
                Constraint::Length(value_width as u16),
            ],
        )
        .column_spacing(1)
        .style(Style::default());
        frame.render_widget(table, values_area);
    }
    fn handle_key(&mut self, key: &crossterm::event::KeyEvent) -> ActionResult {
        let mut result = ActionResult::Consumed;
        match key.code {
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
            KeyCode::Enter => {
                if let Some(id) = get_selected_id(&self.tree_nodes, &self.tree_state) {
                    self.tree_state.toggle_collapsed(&id);
                }
            }
            _ => result = ActionResult::None,
        };
        result
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;

    use super::{ExpenseGroupTree, spending_percentage};
    use crate::budget::data::Categories;

    #[test]
    fn defaults_to_first_expense_group_when_selection_is_missing() {
        let categories_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("categories.json");
        Categories::load(&categories_path).expect("categories.json should load");

        let (group_name, subcategory) = Categories::get_expense_groups()
            .into_iter()
            .find_map(|(group_name, subcategories)| {
                subcategories
                    .into_iter()
                    .find(|category| category.frequency == "variable")
                    .map(|subcategory| (group_name, subcategory))
            })
            .expect("an expense group should include a variable subcategory");
        let mut tree = ExpenseGroupTree::new();
        let amounts = HashMap::from([(group_name.clone(), 1200), (subcategory.name.clone(), 800)]);

        let planned = HashMap::from([(subcategory.name, 700)]);

        tree.update(&amounts, &planned, "variable", 2_000);

        assert!(tree.selected_category().is_some());
        assert_eq!(tree.selected_category().unwrap().name, group_name);
    }

    #[test]
    fn separates_fixed_and_variable_subcategories_and_totals() {
        let categories_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("categories.json");
        Categories::load(&categories_path).expect("categories.json should load");

        let (group_name, subcategories) = Categories::get_expense_groups()
            .into_iter()
            .find(|(_, subcategories)| {
                subcategories
                    .iter()
                    .any(|category| category.frequency == "fixed")
                    && subcategories
                        .iter()
                        .any(|category| category.frequency == "variable")
            })
            .expect("an expense group should include both frequencies");
        let fixed_category = subcategories
            .iter()
            .find(|category| category.frequency == "fixed")
            .unwrap();
        let variable_category = subcategories
            .iter()
            .find(|category| category.frequency == "variable")
            .unwrap();
        let group_frequency = Categories::get(&group_name)
            .expect("expense group should be a category")
            .frequency;

        let amounts = HashMap::from([
            (group_name.clone(), 1_100),
            (fixed_category.name.clone(), 300),
            (variable_category.name.clone(), 700),
        ]);
        let planned = HashMap::from([
            (group_name.clone(), 1_200),
            (fixed_category.name.clone(), 400),
            (variable_category.name.clone(), 600),
        ]);

        let mut fixed_tree = ExpenseGroupTree::new();
        fixed_tree.update(&amounts, &planned, "fixed", 2_000);
        let mut variable_tree = ExpenseGroupTree::new();
        variable_tree.update(&amounts, &planned, "variable", 2_000);

        let fixed_group = fixed_tree
            .tree_nodes
            .iter()
            .find(|node| node.data.name == group_name)
            .expect("fixed tree should include the group");
        let fixed_direct_amount = if group_frequency == "fixed" { 100 } else { 0 };
        let fixed_direct_plan = if group_frequency == "fixed" { 200 } else { 0 };
        assert_eq!(fixed_group.data.actual, 300 + fixed_direct_amount);
        assert_eq!(fixed_group.data.planned, 400 + fixed_direct_plan);
        assert_eq!(fixed_group.children.len(), 1);
        assert_eq!(fixed_group.children[0].data.name, fixed_category.name);
        assert_eq!(
            fixed_tree.totals(),
            (400 + fixed_direct_plan, 300 + fixed_direct_amount)
        );

        let variable_group = variable_tree
            .tree_nodes
            .iter()
            .find(|node| node.data.name == group_name)
            .expect("variable tree should include the group");
        let variable_direct_amount = if group_frequency == "variable" {
            100
        } else {
            0
        };
        let variable_direct_plan = if group_frequency == "variable" {
            200
        } else {
            0
        };
        assert_eq!(variable_group.data.actual, 700 + variable_direct_amount);
        assert_eq!(variable_group.data.planned, 600 + variable_direct_plan);
        assert_eq!(variable_group.children.len(), 1);
        assert_eq!(variable_group.children[0].data.name, variable_category.name);
        assert_eq!(
            variable_tree.totals(),
            (600 + variable_direct_plan, 700 + variable_direct_amount)
        );
    }

    #[test]
    fn includes_transactions_categorized_at_the_expense_group_level() {
        let categories_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("categories.json");
        Categories::load(&categories_path).expect("categories.json should load");

        let (group_name, group_frequency) = Categories::get_expense_groups()
            .into_iter()
            .find_map(|(group_name, subcategories)| {
                (!subcategories.is_empty()).then(|| {
                    let frequency = Categories::get(&group_name)
                        .expect("expense group should be a category")
                        .frequency;
                    (group_name, frequency)
                })
            })
            .expect("an expense group should exist");
        let amounts = HashMap::from([(group_name.clone(), 450)]);
        let planned = HashMap::from([(group_name.clone(), 600)]);

        let mut matching_tree = ExpenseGroupTree::new();
        matching_tree.update(&amounts, &planned, &group_frequency, 1_000);
        let mut other_tree = ExpenseGroupTree::new();
        let other_frequency = if group_frequency == "fixed" {
            "variable"
        } else {
            "fixed"
        };
        other_tree.update(&amounts, &planned, other_frequency, 1_000);

        assert_eq!(matching_tree.tree_nodes.len(), 1);
        assert_eq!(matching_tree.tree_nodes[0].data.actual, 450);
        assert_eq!(matching_tree.tree_nodes[0].data.planned, 600);
        assert!(matching_tree.tree_nodes[0].children.is_empty());
        assert!(other_tree.tree_nodes.is_empty());
    }

    #[test]
    fn spending_percentage_uses_absolute_spending_and_handles_empty_months() {
        assert_eq!(spending_percentage(-250, 1_000), 25.0);
        assert_eq!(spending_percentage(250, 1_000), 25.0);
        assert_eq!(spending_percentage(-250, 0), 0.0);
    }
}
