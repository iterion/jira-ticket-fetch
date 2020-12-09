use crate::app::{InputMode, App};
use tui::{
    backend::{Backend},
    layout::{Constraint, Direction, Layout, Rect},
    Frame,
    style::{Color, Modifier, Style},
    text::Spans,
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};

pub fn draw<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    let size = f.size();

    let help_drawer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(10), Constraint::Length(2)])
        .split(size);

    let help = Paragraph::new("Arrows: Navigate - Enter: Select")
        .style(Style::default().fg(Color::White))
        .block(Block::default().borders(Borders::NONE));
    f.render_widget(help, help_drawer[1]);

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
        .split(help_drawer[0]);

    draw_issues(f, app, chunks[0]);
    draw_branches(f, app, chunks[1]);

    match app.input_mode {
        InputMode::Editing => {
            draw_branch_input(f, app, size);
        }
        _ => (),
    }
}

fn draw_issues<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
    let issues: Vec<ListItem> = app
        .issues
        .items
        .iter()
        .map(|i| {
            let line_content = format!("{}: {}", i.key, i.summary);
            let lines = vec![Spans::from(line_content)];
            ListItem::new(lines).style(Style::default().fg(Color::Black).bg(Color::White))
        })
        .collect();
    let issues = List::new(issues)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("In Progress Jira Issues"),
        )
        .highlight_style(
            Style::default()
                .bg(Color::LightGreen)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");
    f.render_stateful_widget(issues, area, &mut app.issues.state);
}

fn draw_branches<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
    let branches: Vec<ListItem> = app
        .branches
        .items
        .iter()
        .map(|i| {
            let line_content = format!("{}", i.name);
            let lines = vec![Spans::from(line_content)];
            ListItem::new(lines).style(Style::default().fg(Color::Black).bg(Color::White))
        })
        .collect();
    let branches = List::new(branches)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Existing Branches"),
        )
        .highlight_style(
            Style::default()
                .bg(Color::LightGreen)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    f.render_stateful_widget(branches, area, &mut app.branches.state);
}

fn draw_branch_input<B: Backend>(f: &mut Frame<B>, app: &mut App, area: Rect) {
            let area = centered_rect(60, 20, area);
            let input = Paragraph::new(app.new_branch_name().clone())
                .style(Style::default().fg(Color::Yellow))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Enter new branch name"),
                );
            f.render_widget(Clear, area);
            f.render_widget(input, area);
            f.set_cursor(
                // Put cursor past the end of the input text
                area.x + app.new_branch_name().len() as u16 + 1,
                // Move one line down, from the border to the input line
                area.y + 1,
            );
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ]
            .as_ref(),
        )
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ]
            .as_ref(),
        )
        .split(popup_layout[1])[1]
}
