use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap};

use super::app::{App, Focus, InputMode, chat_title, message_preview, short_jid};

const HELP: &str = "\
q / Shift+Q / Esc Esc   quit
i / Enter              compose
Enter (compose)        send
Esc (compose)          cancel
Tab / Space            switch pane
j/k / Up/Down          move selection
/                      filter chats
r                      reply to message
?                      close this help";

pub fn draw(frame: &mut Frame, app: &App) {
    let compact = frame.area().width < 64;
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(frame.area());

    if compact {
        // One focus-driven pane: two 32/68 panes are unusable on a phone.
        match app.focus {
            Focus::Chats => draw_chats(frame, root[0], app, true),
            Focus::Messages | Focus::Compose => draw_messages(frame, root[0], app, true),
        }
    } else {
        let panes = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(32), Constraint::Percentage(68)])
            .split(root[0]);
        draw_chats(frame, panes[0], app, false);
        draw_messages(frame, panes[1], app, false);
    }

    draw_compose(frame, root[1], app, compact);
    draw_status(frame, root[2], app, compact);
    if app.show_help {
        draw_help(frame, frame.area());
    }
}

fn pane_borders(compact: bool) -> Borders {
    if compact {
        Borders::BOTTOM
    } else {
        Borders::ALL
    }
}

fn pane_title(text: String, focused: bool, compact: bool) -> Line<'static> {
    let style = if focused && compact {
        Style::default().add_modifier(Modifier::REVERSED)
    } else {
        Style::default()
    };
    Line::from(Span::styled(text, style))
}

fn draw_chats(frame: &mut Frame, area: Rect, app: &App, compact: bool) {
    let title = if app.input_mode == InputMode::Filter {
        format!("Chats /{}", app.filter)
    } else if compact {
        "Chats".to_string()
    } else {
        "Chats (/ filter)".to_string()
    };
    let items: Vec<ListItem> = app
        .chats
        .iter()
        .enumerate()
        .map(|(idx, chat)| {
            let selected = idx == app.chat_index;
            let style = if selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            let label = format!("{} ({})", chat_title(chat), chat.message_count);
            ListItem::new(Line::from(Span::styled(label, style)))
        })
        .collect();

    let focused = app.focus == Focus::Chats;
    let block = Block::default()
        .title(pane_title(title, focused, compact))
        .borders(pane_borders(compact))
        .border_style(focus_style(focused));
    frame.render_widget(List::new(items).block(block), area);
}

fn draw_messages(frame: &mut Frame, area: Rect, app: &App, compact: bool) {
    let title = match app.selected_chat() {
        Some(chat) => format!("Thread · {}", chat_title(chat)),
        None => "Thread".to_string(),
    };
    let items: Vec<ListItem> = app
        .messages
        .iter()
        .enumerate()
        .map(|(idx, message)| {
            let selected = idx == app.message_index && app.focus == Focus::Messages;
            let style = if selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else if message.from_me {
                Style::default().fg(Color::Green)
            } else {
                Style::default()
            };
            let who = if message.from_me {
                "me".to_string()
            } else {
                message
                    .sender_name
                    .clone()
                    .or_else(|| {
                        message
                            .sender_jid
                            .as_deref()
                            .map(short_jid)
                            .map(str::to_string)
                    })
                    .unwrap_or_else(|| "?".into())
            };
            let label = format!("{}: {}", who, message_preview(message));
            ListItem::new(Line::from(Span::styled(label, style)))
        })
        .collect();

    let focused = app.focus == Focus::Messages || (compact && app.focus == Focus::Compose);
    let block = Block::default()
        .title(pane_title(title, focused, compact))
        .borders(pane_borders(compact))
        .border_style(focus_style(app.focus == Focus::Messages));
    frame.render_widget(List::new(items).block(block), area);
}

fn draw_compose(frame: &mut Frame, area: Rect, app: &App, compact: bool) {
    let reply = match (&app.reply_to, &app.reply_sender) {
        (Some(id), Some(sender)) => format!(" reply→{} ({})", short_jid(id), short_jid(sender)),
        (Some(id), None) => format!(" reply→{}", short_jid(id)),
        _ => String::new(),
    };
    let title = if app.config.read_only {
        format!("Compose (read-only){reply}")
    } else if compact {
        format!("Compose{reply}")
    } else {
        format!("Compose (Enter send, Esc cancel){reply}")
    };
    let text = if app.input_mode == InputMode::Compose {
        format!("{}█", app.compose)
    } else if app.compose.is_empty() {
        if compact {
            "i to compose".to_string()
        } else {
            "press i or Enter to compose".to_string()
        }
    } else {
        app.compose.clone()
    };
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(focus_style(app.focus == Focus::Compose));
    frame.render_widget(
        Paragraph::new(text).block(block).wrap(Wrap { trim: false }),
        area,
    );
}

fn draw_status(frame: &mut Frame, area: Rect, app: &App, compact: bool) {
    let hints = if compact {
        if area.width < 40 {
            "q quit · i compose"
        } else {
            "q quit · i compose · Tab pane · ? help"
        }
    } else {
        "q quit  j/k move  Tab/Space focus  / filter  r reply  ? help"
    };
    let prefix = " whatshell ";
    let fixed = prefix.chars().count() + hints.chars().count() + 7;
    let budget = area.width.saturating_sub(fixed as u16) as usize;
    let status = truncate(&app.status, budget / 2);
    let store_part = if compact {
        String::new()
    } else if budget > 24 {
        let store = app.config.store_dir.display().to_string();
        format!(
            " │ {}",
            truncate(&store, budget.saturating_sub(status.chars().count()))
        )
    } else {
        String::new()
    };
    let line = Line::from(vec![
        Span::styled(prefix, Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(format!("│ {}{} │ {}", status, store_part, hints)),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

fn draw_help(frame: &mut Frame, area: Rect) {
    let area = Rect {
        x: area.x.saturating_add(1),
        y: area.y.saturating_add(1),
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(HELP).block(
            Block::default()
                .title("Keys (any key to close)")
                .borders(Borders::ALL),
        ),
        area,
    );
}

fn focus_style(focused: bool) -> Style {
    if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    }
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::TermuxMode;
    use crate::config::AppConfig;
    use crate::store::Store;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn test_app() -> (tempfile::TempDir, App) {
        let dir = tempfile::tempdir().unwrap();
        let config = AppConfig::new(
            Some(dir.path().to_path_buf()),
            false,
            60,
            false,
            TermuxMode::Off,
        )
        .unwrap();
        let store = Store::open(&config.index_db()).unwrap();
        (dir, App::new(config, store).unwrap())
    }

    fn render(width: u16, height: u16, app: &App) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| draw(frame, app)).unwrap();
        terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect()
    }

    #[test]
    fn renders_narrow_phone_layout() {
        let (_dir, app) = test_app();
        let text = render(40, 16, &app);
        assert!(text.contains("Chats"));
        assert!(text.contains("i compose"));
        assert!(!text.contains("press i or Enter to compose"));
    }

    #[test]
    fn renders_wide_desktop_layout() {
        let (_dir, app) = test_app();
        let text = render(100, 30, &app);
        assert!(text.contains("Chats"));
        assert!(text.contains("Thread"));
        assert!(text.contains("? help"));
    }

    #[test]
    fn help_overlay_renders_on_narrow_screen() {
        let (_dir, mut app) = test_app();
        app.show_help = true;
        let text = render(40, 16, &app);
        assert!(text.contains("Keys"));
        assert!(text.contains("switch pane"));
    }
}
