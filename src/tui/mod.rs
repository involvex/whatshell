mod app;
mod ui;

use std::io::{self, Stdout};
use std::time::Duration;

use anyhow::{Result, anyhow};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use crate::config::AppConfig;
use crate::lock::StoreLock;
use crate::store::Store;
use crate::termux::MessageNotifier;
use crate::whatshell::{self, ControlEvent, LiveSession};

use app::{App, Focus, InputMode};

pub async fn run(config: AppConfig) -> Result<()> {
    if !whatshell::has_session(&config.session_db()) {
        return Err(anyhow!("not authenticated; run `whatshell auth` first"));
    }

    config.ensure_dirs()?;
    let _lock = StoreLock::acquire(&config.lock_path())?;
    let store = Store::open(&config.index_db())?;
    let mut session = LiveSession::start(config.clone()).await?;
    let notifier = MessageNotifier::new(
        config.termux.as_ref().map(|termux| (**termux).clone()),
        true,
    );
    let mut app = App::new(config, store)?;

    let mut terminal = setup_terminal()?;
    let result = run_loop(&mut terminal, &mut app, &mut session, &notifier).await;
    notifier.flush();
    let restore = restore_terminal(&mut terminal);
    let shutdown = session.shutdown().await;
    result.and(restore).and(shutdown.map(|_| ()))
}

async fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut App,
    session: &mut LiveSession,
    notifier: &MessageNotifier,
) -> Result<()> {
    // Redraw only when something actually changed. Phones are CPU- and
    // battery-constrained, and a 50 ms full redraw is wasteful when idle.
    let mut dirty = true;
    loop {
        if dirty {
            terminal.draw(|frame| ui::draw(frame, app))?;
            dirty = false;
        }

        tokio::select! {
            event = session.control_rx.recv() => {
                match event {
                    Some(ControlEvent::LoggedOut) => {
                        return Err(anyhow!("WhatsApp session logged out; run `whatshell auth`"));
                    }
                    Some(ControlEvent::NeedsAuth) => {
                        return Err(anyhow!("not authenticated; run `whatshell auth` first"));
                    }
                    Some(ControlEvent::Connected) => {
                        app.set_status("connected");
                        dirty = true;
                    }
                    Some(ControlEvent::Stored) | Some(ControlEvent::PairSuccess) => {}
                    None => {
                        return Err(anyhow!("live session control channel closed"));
                    }
                }
            }
            record = session.notify_rx.recv() => {
                if let Some(record) = record {
                    let label = record.chat_name.clone().unwrap_or_else(|| {
                        record
                            .chat_jid
                            .split('@')
                            .next()
                            .unwrap_or(&record.chat_jid)
                            .to_string()
                    });
                    app.on_message(record)?;
                    if !app.terminal_focused {
                        notifier.note(&label);
                    }
                    dirty = true;
                }
            }
            _ = tokio::time::sleep(Duration::from_millis(80)) => {
                while event::poll(Duration::ZERO)? {
                    match event::read()? {
                        Event::Key(key)
                            if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) =>
                        {
                            if handle_key(app, session, key.code, key.modifiers).await? {
                                return Ok(());
                            }
                            dirty = true;
                        }
                        Event::FocusGained => {
                            app.terminal_focused = true;
                            dirty = true;
                        }
                        Event::FocusLost => {
                            app.terminal_focused = false;
                            dirty = true;
                        }
                        Event::Resize(_, _) => dirty = true,
                        _ => {}
                    }
                }
            }
        }
    }
}

async fn handle_key(
    app: &mut App,
    session: &mut LiveSession,
    code: KeyCode,
    modifiers: KeyModifiers,
) -> Result<bool> {
    if modifiers.contains(KeyModifiers::CONTROL) && matches!(code, KeyCode::Char('c')) {
        return Ok(true);
    }
    if app.show_help {
        app.show_help = false;
        return Ok(false);
    }
    if code != KeyCode::Esc {
        app.quit_armed = false;
    }

    match app.input_mode {
        InputMode::Normal => match code {
            KeyCode::Char('q') | KeyCode::Char('Q') => return Ok(true),
            KeyCode::Tab | KeyCode::Char(' ') => app.cycle_focus(),
            KeyCode::Char('?') => app.show_help = true,
            KeyCode::Char('/') if app.focus == Focus::Chats => {
                app.input_mode = InputMode::Filter;
                app.filter.clear();
            }
            KeyCode::Char('i') | KeyCode::Enter if app.focus != Focus::Compose => {
                app.focus = Focus::Compose;
                app.input_mode = InputMode::Compose;
            }
            KeyCode::Char('r') => {
                if app.begin_reply() {
                    app.focus = Focus::Compose;
                    app.input_mode = InputMode::Compose;
                }
            }
            KeyCode::Esc => {
                if app.quit_armed {
                    return Ok(true);
                }
                if app.reply_to.is_some() {
                    app.clear_reply();
                    app.set_status("ready");
                } else {
                    // Soft keyboards have no reliable Ctrl key: two Escs quit.
                    app.quit_armed = true;
                    app.set_status("press Esc again to quit");
                }
            }
            KeyCode::Up | KeyCode::Char('k') => app.move_selection(-1)?,
            KeyCode::Down | KeyCode::Char('j') => app.move_selection(1)?,
            KeyCode::Left | KeyCode::Char('h') => {
                app.focus = Focus::Chats;
            }
            KeyCode::Right | KeyCode::Char('l') => {
                app.focus = Focus::Messages;
            }
            _ => {}
        },
        InputMode::Filter => match code {
            KeyCode::Esc => {
                app.input_mode = InputMode::Normal;
                app.filter.clear();
                app.refresh_chats()?;
            }
            KeyCode::Enter => {
                app.input_mode = InputMode::Normal;
                app.refresh_chats()?;
            }
            KeyCode::Backspace => {
                app.filter.pop();
                app.refresh_chats()?;
            }
            KeyCode::Char(ch) => {
                app.filter.push(ch);
                app.refresh_chats()?;
            }
            _ => {}
        },
        InputMode::Compose => match code {
            KeyCode::Esc => {
                app.input_mode = InputMode::Normal;
                app.focus = Focus::Messages;
                app.compose.clear();
                app.clear_reply();
            }
            KeyCode::Enter => {
                if app.config.read_only {
                    app.set_status("read-only mode blocks send");
                } else if let Some((to, text, reply_to, reply_sender)) = app.compose_payload() {
                    match session
                        .send_text(&to, &text, reply_to.as_deref(), reply_sender.as_deref())
                        .await
                    {
                        Ok(summary) => {
                            app.compose.clear();
                            app.clear_reply();
                            app.input_mode = InputMode::Normal;
                            app.focus = Focus::Messages;
                            let status =
                                format!("sent {}", summary.message_id.as_deref().unwrap_or("ok"));
                            if let Some(api) = app.config.termux.as_deref() {
                                api.toast(&status);
                            }
                            app.set_status(status);
                        }
                        Err(err) => {
                            let status = format!("send failed: {err:#}");
                            if let Some(api) = app.config.termux.as_deref() {
                                api.toast(&status);
                            }
                            app.set_status(status);
                        }
                    }
                }
            }
            KeyCode::Backspace => {
                app.compose.pop();
            }
            KeyCode::Char(ch) => {
                app.compose.push(ch);
            }
            _ => {}
        },
    }

    Ok(false)
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    Terminal::new(backend).map_err(Into::into)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}
