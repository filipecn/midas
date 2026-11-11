use std::collections::HashMap;

use crossterm::event::KeyCode;
use ratatui::{
    layout::{Constraint, Layout},
    style::Style,
    text::{Line, Span},
    widgets::{Cell, Paragraph, Row, Table, Widget},
};
use ratatui_interact::components::{TreeNode, TreeStyle, TreeView, TreeViewState, get_selected_id};

use crate::{investments::Investment, tui::TUI, widgets::InteractableWidget};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InvestmentTreeEntry {
    pub name: String,
    pub parent_name: Option<String>,
    pub total_amount: i64,
    pub total_invested: i64,
    pub is_parent: bool,
}

pub struct InvestmentsTree {
    pub tree_nodes: Vec<TreeNode<InvestmentTreeEntry>>,
    pub tree_state: TreeViewState,
}

impl InvestmentsTree {
    pub fn update(&mut self, investments: &[Investment], invested_amounts: &HashMap<String, i64>) {
        let mut grouped: HashMap<String, HashMap<String, (i64, i64)>> = HashMap::new();

        for investment in investments
            .iter()
            .filter(|item| item.status.eq_ignore_ascii_case("active"))
        {
            let investment_type = investment.investment_type.trim();
            if investment_type.is_empty() {
                continue;
            }

            let type_totals = grouped.entry(investment_type.to_string()).or_default();
            let subtype = investment.subtype.trim();
            let subtype_key = if subtype.is_empty() {
                String::new()
            } else {
                subtype.to_string()
            };
            let totals = type_totals.entry(subtype_key).or_insert((0, 0));
            totals.0 += investment.amount_in_cents;
            totals.1 += invested_amounts
                .get(&investment.identifier)
                .copied()
                .unwrap_or_default();
        }

        let mut tree_nodes: Vec<TreeNode<InvestmentTreeEntry>> = Vec::new();
        let mut grouped_types: Vec<(String, HashMap<String, (i64, i64)>)> =
            grouped.into_iter().collect();
        grouped_types.sort_by(|(left, _), (right, _)| left.cmp(right));

        for (investment_type, mut subtypes) in grouped_types {
            let mut children: Vec<TreeNode<InvestmentTreeEntry>> = Vec::new();
            let mut type_total_amount = 0;
            let mut type_total_invested = 0;

            let mut subtype_entries: Vec<(String, (i64, i64))> = subtypes.drain().collect();
            subtype_entries.sort_by(|(left, _), (right, _)| left.cmp(right));

            for (subtype_name, (amount, invested)) in subtype_entries {
                type_total_amount += amount;
                type_total_invested += invested;

                if subtype_name.is_empty() {
                    continue;
                }

                children.push(TreeNode::new(
                    subtype_name.clone(),
                    InvestmentTreeEntry {
                        name: subtype_name,
                        parent_name: Some(investment_type.clone()),
                        total_amount: amount,
                        total_invested: invested,
                        is_parent: false,
                    },
                ));
            }

            if !children.is_empty() {
                tree_nodes.push(
                    TreeNode::new(
                        investment_type.clone(),
                        InvestmentTreeEntry {
                            name: investment_type.clone(),
                            parent_name: None,
                            total_amount: type_total_amount,
                            total_invested: type_total_invested,
                            is_parent: true,
                        },
                    )
                    .with_children(children),
                );
            }
        }

        self.tree_state.collapsed = tree_nodes.iter().map(|node| node.id.clone()).collect();
        self.tree_nodes = tree_nodes;
    }

    pub fn new() -> Self {
        Self {
            tree_nodes: Vec::new(),
            tree_state: TreeViewState::default(),
        }
    }

    fn visible_tree_count(&self) -> usize {
        let tree = TreeView::new(&self.tree_nodes, &self.tree_state);
        tree.visible_count()
    }

    fn visible_nodes<'a>(
        nodes: &'a [TreeNode<InvestmentTreeEntry>],
        state: &TreeViewState,
        visible: &mut Vec<&'a TreeNode<InvestmentTreeEntry>>,
    ) {
        for node in nodes {
            visible.push(node);
            if node.has_children() && !state.is_collapsed(&node.id) {
                Self::visible_nodes(&node.children, state, visible);
            }
        }
    }

    pub fn selected_entry(&self) -> Option<InvestmentTreeEntry> {
        get_selected_id(&self.tree_nodes, &self.tree_state).and_then(|selected_id| {
            let mut found = None;

            fn walk<'a>(
                nodes: &'a [TreeNode<InvestmentTreeEntry>],
                selected_id: &str,
                found: &mut Option<InvestmentTreeEntry>,
            ) {
                for node in nodes {
                    if node.id == *selected_id {
                        *found = Some(node.data.clone());
                        return;
                    }
                    if node.has_children() {
                        walk(&node.children, selected_id, found);
                        if found.is_some() {
                            return;
                        }
                    }
                }
            }

            walk(&self.tree_nodes, &selected_id, &mut found);
            found
        })
    }
}

impl InteractableWidget for InvestmentsTree {
    fn render(&self, area: ratatui::layout::Rect, frame: &mut ratatui::Frame) {
        let style = TreeStyle::default();
        let palette = TUI::color_palette();
        let [header_area, tree_area, totals_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .areas(area);
        let [header_names_area, header_values_area] =
            Layout::horizontal([Constraint::Percentage(30), Constraint::Min(1)]).areas(header_area);
        let [names_area, values_area] =
            Layout::horizontal([Constraint::Percentage(30), Constraint::Min(1)]).areas(tree_area);
        let [totals_names_area, totals_values_area] =
            Layout::horizontal([Constraint::Percentage(30), Constraint::Min(1)]).areas(totals_area);

        frame.render_widget(Paragraph::new("Type"), header_names_area);
        let header = Row::new([
            Cell::from(format!("{:>8}", "Invested")),
            Cell::from(format!("{:>8}", "Amount")),
            Cell::from(format!("{:>8}", "Change %")),
        ]);
        let header_table = Table::new(
            std::iter::once(header),
            [
                Constraint::Length(8),
                Constraint::Length(8),
                Constraint::Length(8),
            ],
        )
        .column_spacing(1)
        .style(Style::default());
        frame.render_widget(header_table, header_values_area);

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
            .map(|node| {
                let diff = node.data.total_amount - node.data.total_invested;
                let pct = if node.data.total_invested == 0 {
                    0.0
                } else {
                    (diff as f64 / node.data.total_invested as f64) * 100.0
                };
                let pct_color = if pct >= 0.0 {
                    palette.green
                } else {
                    palette.spent
                };

                Row::new([
                    Cell::from(if TUI::sensitive_values_visible() {
                        format!("{:>8.2}", node.data.total_invested as f64 / 100.0)
                    } else {
                        TUI::format_amount_for_display(node.data.total_invested, 8)
                    }),
                    Cell::from(if TUI::sensitive_values_visible() {
                        format!("{:>8.2}", node.data.total_amount as f64 / 100.0)
                    } else {
                        TUI::format_amount_for_display(node.data.total_amount, 8)
                    }),
                    Cell::from(format!("{:>8}", format!("{pct:+.2}%")))
                        .style(Style::default().fg(pct_color)),
                ])
            });

        let table = Table::new(
            rows,
            [
                Constraint::Length(8),
                Constraint::Length(8),
                Constraint::Length(8),
            ],
        )
        .column_spacing(1)
        .style(Style::default());
        frame.render_widget(table, values_area);

        let (total_invested, total_amount): (i64, i64) =
            self.tree_nodes
                .iter()
                .fold((0, 0), |(invested, amount), node| {
                    (
                        invested + node.data.total_invested,
                        amount + node.data.total_amount,
                    )
                });
        let total_change = total_amount - total_invested;
        let total_pct = if total_invested == 0 {
            0.0
        } else {
            (total_change as f64 / total_invested as f64) * 100.0
        };
        let pct_color = if total_pct >= 0.0 {
            palette.green
        } else {
            palette.spent
        };

        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "Total",
                Style::default().add_modifier(ratatui::style::Modifier::BOLD),
            ))),
            totals_names_area,
        );

        let total_row = Row::new([
            Cell::from(if TUI::sensitive_values_visible() {
                format!("{:>8.2}", total_invested as f64 / 100.0)
            } else {
                TUI::format_amount_for_display(total_invested, 8)
            }),
            Cell::from(if TUI::sensitive_values_visible() {
                format!("{:>8.2}", total_amount as f64 / 100.0)
            } else {
                TUI::format_amount_for_display(total_amount, 8)
            }),
            Cell::from(format!("{:>8}", format!("{total_pct:+.2}%")))
                .style(Style::default().fg(pct_color)),
        ])
        .style(Style::default().add_modifier(ratatui::style::Modifier::BOLD));

        let totals_table = Table::new(
            std::iter::once(total_row),
            [
                Constraint::Length(8),
                Constraint::Length(8),
                Constraint::Length(8),
            ],
        )
        .column_spacing(1)
        .style(Style::default());
        frame.render_widget(totals_table, totals_values_area);
    }

    fn handle_key(&mut self, key: &crossterm::event::KeyEvent) -> crate::widgets::ActionResult {
        let mut result = crate::widgets::ActionResult::Consumed;
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
            _ => result = crate::widgets::ActionResult::None,
        }
        result
    }
}
