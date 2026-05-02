use std::path::PathBuf;
use std::time::{Duration, Instant};

use crossterm::event::{
    self, Event, KeyCode, KeyModifiers,
};
use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::error::FossilError;

use super::views::AppAction;
use super::views::help::HelpOverlay;
use super::views::main_view::MainView;

const MAX_CONTENT_WIDTH: u16 = 220;

fn center(area: Rect, max_w: u16) -> Rect {
    Layout::horizontal([Constraint::Max(max_w)])
        .flex(Flex::Center)
        .areas::<1>(area)[0]
}

pub struct App {
    view: MainView,
    show_help: bool,
    flash: Option<(String, Instant)>,
}

impl App {
    pub fn new(
        fossil_home: PathBuf,
    ) -> Result<Self, FossilError> {
        let projects_dir = fossil_home.join("projects");
        let view = MainView::load(projects_dir)?;
        Ok(Self {
            view,
            show_help: false,
            flash: None,
        })
    }

    pub fn run(
        &mut self,
        terminal: &mut ratatui::DefaultTerminal,
    ) -> Result<(), FossilError> {
        loop {
            terminal.draw(|frame| self.render(frame))?;

            let action = self.view.tick();
            if self.apply(action) {
                break;
            }

            let timeout = Duration::from_millis(100);
            if !event::poll(timeout)? {
                continue;
            }

            if let Event::Key(key) = event::read()? {
                if key.modifiers
                    .contains(KeyModifiers::CONTROL)
                    && key.code == KeyCode::Char('c')
                {
                    break;
                }

                if self.show_help {
                    if HelpOverlay::handle_key(key) {
                        self.show_help = false;
                    }
                    continue;
                }

                let action = self.view.handle_key(key);
                if self.apply(action) {
                    break;
                }
            }
        }
        Ok(())
    }

    fn apply(&mut self, action: AppAction) -> bool {
        match action {
            AppAction::None => false,
            AppAction::Quit => true,
            AppAction::Flash(msg) => {
                self.flash =
                    Some((msg, Instant::now()));
                false
            }
            AppAction::ShowHelp => {
                self.show_help = true;
                false
            }
        }
    }

    fn render(&mut self, frame: &mut Frame) {
        let full = frame.area();
        let inner = center(full, MAX_CONTENT_WIDTH);

        let chunks = Layout::vertical([
            Constraint::Length(2),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(inner);

        let breadcrumb_area = chunks[0];
        let content_area = chunks[1];
        let hints_area = chunks[2];

        let breadcrumb = Line::from(vec![
            Span::styled(
                self.view.project_name().to_string(),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(
                " > ",
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(
                self.view.fossil_name().to_string(),
                Style::default().fg(Color::White),
            ),
        ]);
        frame.render_widget(
            Paragraph::new(breadcrumb),
            breadcrumb_area,
        );

        self.view.render(frame, content_area);

        let hints: Vec<Span> =
            if let Some((ref msg, at)) = self.flash {
                if at.elapsed().as_secs() < 5 {
                    vec![Span::styled(
                        msg.clone(),
                        Style::default()
                            .fg(Color::Yellow),
                    )]
                } else {
                    self.flash = None;
                    self.hint_spans()
                }
            } else {
                self.hint_spans()
            };
        frame.render_widget(
            Paragraph::new(Line::from(hints)),
            hints_area,
        );

        if self.show_help {
            HelpOverlay::render(frame, inner);
        }
    }

    fn hint_spans(&self) -> Vec<Span<'static>> {
        let pairs = self.view.hints();
        let mut spans = Vec::new();
        for (i, (key, desc)) in
            pairs.iter().enumerate()
        {
            if i > 0 {
                spans.push(Span::styled(
                    "  ",
                    Style::default(),
                ));
            }
            spans.push(Span::styled(
                key.to_string(),
                Style::default().fg(Color::DarkGray),
            ));
            spans.push(Span::styled(
                format!(":{desc}"),
                Style::default().fg(Color::DarkGray),
            ));
        }
        spans
    }
}
