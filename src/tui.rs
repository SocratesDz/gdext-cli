use std::path::PathBuf;

use anyhow::Result;
use ratatui::{
    DefaultTerminal,
    crossterm::event::{self, Event, KeyCode, KeyModifiers},
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};

use crate::{handle_script, handle_startproject, handle_template};

const COMMANDS: [&str; 3] = ["startproject", "script", "template"];

enum Screen {
    CommandSelection,
    ParameterInput,
}

struct App {
    screen: Screen,
    selected_command: usize,
    list_state: ListState,
    params: Vec<(String, String)>,
    active_param: usize,
}

impl App {
    fn new() -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        Self {
            screen: Screen::CommandSelection,
            selected_command: 0,
            list_state,
            params: Vec::new(),
            active_param: 0,
        }
    }

    fn params_for_command(cmd: &str) -> Vec<(String, String)> {
        match cmd {
            "startproject" => vec![
                ("script".to_string(), String::new()),
                ("name".to_string(), String::new()),
                ("godot_dir (optional)".to_string(), String::new()),
            ],
            "script" => vec![
                ("name".to_string(), String::new()),
                ("typenode".to_string(), String::new()),
            ],
            "template" => vec![("name".to_string(), String::new())],
            _ => vec![],
        }
    }

    fn select_command(&mut self) {
        self.params = Self::params_for_command(COMMANDS[self.selected_command]);
        self.active_param = 0;
        self.screen = Screen::ParameterInput;
    }

    fn move_up(&mut self) {
        if self.selected_command > 0 {
            self.selected_command -= 1;
            self.list_state.select(Some(self.selected_command));
        }
    }

    fn move_down(&mut self) {
        if self.selected_command + 1 < COMMANDS.len() {
            self.selected_command += 1;
            self.list_state.select(Some(self.selected_command));
        }
    }

    fn param_next(&mut self) {
        if self.active_param + 1 < self.params.len() {
            self.active_param += 1;
        }
    }

    fn param_prev(&mut self) {
        if self.active_param > 0 {
            self.active_param -= 1;
        }
    }

    fn param_push(&mut self, c: char) {
        self.params[self.active_param].1.push(c);
    }

    fn param_pop(&mut self) {
        self.params[self.active_param].1.pop();
    }

    fn is_last_param(&self) -> bool {
        self.active_param + 1 == self.params.len()
    }
}

pub fn run_wizard() -> Result<()> {
    let mut terminal = ratatui::init();
    let result = run_app(&mut terminal);
    ratatui::restore();
    result
}

fn run_app(terminal: &mut DefaultTerminal) -> Result<()> {
    let mut app = App::new();

    loop {
        terminal.draw(|frame| {
            let area = frame.area();
            match &app.screen {
                Screen::CommandSelection => draw_command_selection(frame, area, &mut app),
                Screen::ParameterInput => draw_parameter_input(frame, area, &app),
            }
        })?;

        if let Event::Key(key) = event::read()? {
            match &app.screen {
                Screen::CommandSelection => match key.code {
                    KeyCode::Up | KeyCode::Char('k') => app.move_up(),
                    KeyCode::Down | KeyCode::Char('j') => app.move_down(),
                    KeyCode::Enter => app.select_command(),
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    _ => {}
                },
                Screen::ParameterInput => match key.code {
                    KeyCode::Esc => {
                        app.screen = Screen::CommandSelection;
                    }
                    KeyCode::Tab | KeyCode::Down => app.param_next(),
                    KeyCode::BackTab | KeyCode::Up => app.param_prev(),
                    KeyCode::Backspace => app.param_pop(),
                    KeyCode::Enter => {
                        if app.is_last_param() {
                            return dispatch(&app);
                        } else {
                            app.param_next();
                        }
                    }
                    KeyCode::Char(c) => {
                        if key.modifiers.contains(KeyModifiers::SHIFT) {
                            // uppercase already handled by KeyCode::Char for shift+letter
                        }
                        app.param_push(c);
                    }
                    _ => {}
                },
            }
        }
    }
}

fn draw_command_selection(
    frame: &mut ratatui::Frame,
    area: ratatui::layout::Rect,
    app: &mut App,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    let title = Paragraph::new("gdext-cli wizard — select a command (↑/↓ to navigate, Enter to select, q to quit)")
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(title, chunks[0]);

    let items: Vec<ListItem> = COMMANDS
        .iter()
        .map(|c| ListItem::new(*c))
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Commands"))
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

    frame.render_stateful_widget(list, chunks[1], &mut app.list_state);
}

fn draw_parameter_input(frame: &mut ratatui::Frame, area: ratatui::layout::Rect, app: &App) {
    let cmd = COMMANDS[app.selected_command];
    let n = app.params.len();
    let header_height = 3u16;
    let field_height = 3u16;
    let constraints: Vec<Constraint> = std::iter::once(Constraint::Length(header_height))
        .chain((0..n).map(|_| Constraint::Length(field_height)))
        .collect();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    let hint = format!(
        "Command: {}  — Tab/↓ next field, ↑/Shift-Tab prev, Enter confirm, Esc back",
        cmd
    );
    let header = Paragraph::new(hint).block(Block::default().borders(Borders::ALL));
    frame.render_widget(header, chunks[0]);

    for (i, (label, value)) in app.params.iter().enumerate() {
        let is_active = i == app.active_param;
        let border_style = if is_active {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default()
        };
        let title_style = if is_active {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        // Show cursor indicator on the active field
        let display_value = if is_active {
            format!("{}_", value)
        } else {
            value.clone()
        };

        let widget = Paragraph::new(Line::from(vec![Span::raw(display_value)])).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title(Span::styled(label.as_str(), title_style)),
        );
        frame.render_widget(widget, chunks[i + 1]);
    }
}

fn dispatch(app: &App) -> Result<()> {
    let cmd = COMMANDS[app.selected_command];
    match cmd {
        "startproject" => {
            let script = &app.params[0].1;
            let name = &app.params[1].1;
            let godot_dir_str = &app.params[2].1;
            let godot_dir: Option<PathBuf> = if godot_dir_str.is_empty() {
                None
            } else {
                Some(PathBuf::from(godot_dir_str))
            };
            handle_startproject(script, name, &godot_dir)
        }
        "script" => {
            let name = &app.params[0].1;
            let typenode = &app.params[1].1;
            handle_script(name, typenode)
        }
        "template" => {
            let name = &app.params[0].1;
            handle_template(name)
        }
        _ => Ok(()),
    }
}
