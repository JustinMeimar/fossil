use std::path::PathBuf;
use std::time::{Duration, Instant};
use crate::error::FossilError;
use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use super::views::AppAction;
use super::views::help::HelpOverlay;
use super::views::main_view::MainView;
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture,
    Event, KeyCode, KeyModifiers,
};
use crossterm::execute;
use crossterm::terminal::{
    LeaveAlternateScreen, EnterAlternateScreen,
    disable_raw_mode, enable_raw_mode,
};

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
            if self.apply(action, terminal) {
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
                if self.apply(action, terminal) {
                    break;
                }
            }
        }
        Ok(())
    }

    fn apply(
        &mut self,
        action: AppAction,
        terminal: &mut ratatui::DefaultTerminal,
    ) -> bool {
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
            AppAction::Edit(path) => {
                let msg = self.spawn_editor(
                    terminal, &path,
                );
                self.view.reload();
                if let Err(e) = msg {
                    self.flash = Some((
                        e.to_string(),
                        Instant::now(),
                    ));
                }
                false
            }
        }
    }

    fn spawn_editor(
        &self,
        terminal: &mut ratatui::DefaultTerminal,
        path: &std::path::Path,
    ) -> Result<(), String> {
        let editor = std::env::var("EDITOR")
            .unwrap_or_else(|_| "vi".into());

        let _ = disable_raw_mode();
        let _ = execute!(
            std::io::stderr(),
            LeaveAlternateScreen,
            DisableMouseCapture
        );

        let status = std::process::Command::new(&editor)
            .arg(path)
            .status();

        let _ = enable_raw_mode();
        let _ = execute!(
            std::io::stdout(),
            EnterAlternateScreen,
            EnableMouseCapture
        );
        let _ = terminal.clear();

        match status {
            Ok(s) if s.success() => Ok(()),
            Ok(s) => Err(format!(
                "{editor} exited with {}",
                s.code().unwrap_or(-1)
            )),
            Err(e) => Err(format!(
                "failed to run {editor}: {e}"
            )),
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
                Style::default().fg(Color::Gray),
            ),
            Span::styled(
                " > ",
                Style::default().fg(Color::Gray),
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
                Style::default().fg(Color::Gray),
            ));
            spans.push(Span::styled(
                format!(":{desc}"),
                Style::default().fg(Color::Gray),
            ));
        }
        spans
    }
}
