use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};

use super::app::{App, Focus, InputMode, chat_title, message_preview, short_jid};

pub fn draw(frame: &mut Frame, app: &App) {
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(frame.area());

    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(32), Constraint::Percentage(68)])
        .split(root[0]);

    draw_chats(frame, panes[0], app);
    draw_messages(frame, panes[1], app);
    draw_compose(frame, root[1], app);
    draw_status(frame, root[2], app);
}

fn draw_chats(frame: &mut Frame, area: Rect, app: &App) {
    let title = if app.input_mode == InputMode::Filter {
        format!("Chats /{}", app.filter)
    } else {
        "Chats (/ filter)".into()
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

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(focus_style(app.focus == Focus::Chats));
    frame.render_widget(List::new(items).block(block), area);
}

fn draw_messages(frame: &mut Frame, area: Rect, app: &App) {
    let title = match app.selected_chat() {
        Some(chat) => format!("Thread · {}", chat_title(chat)),
        None => "Thread".into(),
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

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(focus_style(app.focus == Focus::Messages));
    frame.render_widget(List::new(items).block(block), area);
}

fn draw_compose(frame: &mut Frame, area: Rect, app: &App) {
    let reply = match (&app.reply_to, &app.reply_sender) {
        (Some(id), Some(sender)) => format!(" reply→{} ({})", short_jid(id), short_jid(sender)),
        (Some(id), None) => format!(" reply→{}", short_jid(id)),
        _ => String::new(),
    };
    let title = if app.config.read_only {
        format!("Compose (read-only){reply}")
    } else {
        format!("Compose (Enter send, Esc cancel){reply}")
    };
    let text = if app.input_mode == InputMode::Compose {
        format!("{}█", app.compose)
    } else if app.compose.is_empty() {
        "press i or Enter to compose".into()
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

fn draw_status(frame: &mut Frame, area: Rect, app: &App) {
    let store = app.config.store_dir.display().to_string();
    let line = Line::from(vec![
        Span::styled(" whatshell ", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(format!(
            "│ {} │ {} │ q quit  j/k move  Tab focus  / filter  r reply",
            app.status, store
        )),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

fn focus_style(focused: bool) -> Style {
    if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    }
}
