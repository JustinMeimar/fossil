use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Clear, Paragraph,
};

const SECTIONS: &[(&str, &[(&str, &str)])] = &[
    (
        "Navigation",
        &[
            ("h / Left", "previous column"),
            ("l / Right", "next column"),
            ("j / Down", "move down"),
            ("k / Up", "move up"),
            ("g", "jump to top"),
            ("G", "jump to bottom"),
            ("Ctrl-d", "half page down"),
            ("Ctrl-u", "half page up"),
            ("Tab", "toggle list / preview"),
        ],
    ),
    (
        "Selection",
        &[
            ("Space", "toggle select record"),
            ("Esc", "clear selection"),
        ],
    ),
    (
        "Actions",
        &[
            ("p", "switch project"),
            ("f", "switch fossil"),
            ("e", "edit config / scripts"),
            ("a", "run analysis"),
            ("b", "bury variant"),
            ("d", "delete record"),
            ("q", "quit"),
            ("Ctrl-c", "force quit"),
            ("?", "toggle this help"),
        ],
    ),
];

pub struct HelpOverlay;

impl HelpOverlay {
    pub fn handle_key(key: KeyEvent) -> bool {
        matches!(
            key.code,
            KeyCode::Char('?')
                | KeyCode::Esc
                | KeyCode::Char('q')
        )
    }

    pub fn render(frame: &mut Frame, area: Rect) {
        let width =
            50u16.min(area.width.saturating_sub(4));
        let height =
            28u16.min(area.height.saturating_sub(4));

        let [popup_area] = Layout::horizontal([
            Constraint::Length(width),
        ])
        .flex(Flex::Center)
        .areas(
            Layout::vertical([
                Constraint::Length(height),
            ])
            .flex(Flex::Center)
            .areas::<1>(area)[0],
        );

        frame.render_widget(Clear, popup_area);

        let mut lines = Vec::new();
        for (section, bindings) in SECTIONS {
            lines.push(Line::from(Span::styled(
                format!(" {section}"),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(""));
            for (key, desc) in *bindings {
                lines.push(Line::from(vec![
                    Span::styled(
                        format!("  {key:<14}"),
                        Style::default()
                            .fg(Color::Yellow),
                    ),
                    Span::styled(
                        desc.to_string(),
                        Style::default()
                            .fg(Color::White),
                    ),
                ]));
            }
            lines.push(Line::from(""));
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(
                Style::default().fg(Color::DarkGray),
            )
            .title(Span::styled(
                " keybindings ",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ));
        let paragraph =
            Paragraph::new(lines).block(block);
        frame.render_widget(paragraph, popup_area);
    }
}
