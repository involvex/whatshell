use anyhow::Result;

use crate::config::AppConfig;
use crate::store::{ChatRecord, MessageFilter, MessageRecord, Store};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Chats,
    Messages,
    Compose,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Filter,
    Compose,
}

pub struct App {
    pub config: AppConfig,
    store: Store,
    pub chats: Vec<ChatRecord>,
    pub messages: Vec<MessageRecord>,
    pub chat_index: usize,
    pub message_index: usize,
    pub focus: Focus,
    pub input_mode: InputMode,
    pub filter: String,
    pub compose: String,
    pub reply_to: Option<String>,
    pub reply_sender: Option<String>,
    pub status: String,
}

impl App {
    pub fn new(config: AppConfig, store: Store) -> Result<Self> {
        let mut app = Self {
            config,
            store,
            chats: Vec::new(),
            messages: Vec::new(),
            chat_index: 0,
            message_index: 0,
            focus: Focus::Chats,
            input_mode: InputMode::Normal,
            filter: String::new(),
            compose: String::new(),
            reply_to: None,
            reply_sender: None,
            status: "connecting…".into(),
        };
        app.refresh_chats()?;
        app.set_status("connected");
        Ok(app)
    }

    pub fn set_status(&mut self, status: impl Into<String>) {
        self.status = status.into();
    }

    pub fn selected_chat(&self) -> Option<&ChatRecord> {
        self.chats.get(self.chat_index)
    }

    pub fn selected_message(&self) -> Option<&MessageRecord> {
        self.messages.get(self.message_index)
    }

    pub fn cycle_focus(&mut self) {
        self.focus = match self.focus {
            Focus::Chats => Focus::Messages,
            Focus::Messages => Focus::Compose,
            Focus::Compose => Focus::Chats,
        };
        if self.focus == Focus::Compose {
            self.input_mode = InputMode::Compose;
        } else if self.input_mode == InputMode::Compose {
            self.input_mode = InputMode::Normal;
        }
    }

    pub fn refresh_chats(&mut self) -> Result<()> {
        let query = if self.filter.trim().is_empty() {
            None
        } else {
            Some(self.filter.as_str())
        };
        let selected = self.selected_chat().map(|chat| chat.jid.clone());
        self.chats = self.store.list_chats(query, 200)?;
        if let Some(jid) = selected
            && let Some(idx) = self.chats.iter().position(|chat| chat.jid == jid)
        {
            self.chat_index = idx;
        } else if self.chat_index >= self.chats.len() {
            self.chat_index = self.chats.len().saturating_sub(1);
        }
        self.refresh_messages()?;
        Ok(())
    }

    pub fn refresh_messages(&mut self) -> Result<()> {
        let Some(chat) = self.selected_chat().map(|chat| chat.jid.clone()) else {
            self.messages.clear();
            self.message_index = 0;
            return Ok(());
        };
        self.messages = self.store.list_messages(&MessageFilter {
            chat: Some(chat),
            limit: 200,
            asc: true,
            ..Default::default()
        })?;
        if self.message_index >= self.messages.len() {
            self.message_index = self.messages.len().saturating_sub(1);
        }
        Ok(())
    }

    pub fn move_selection(&mut self, delta: isize) -> Result<()> {
        match self.focus {
            Focus::Chats => {
                if self.chats.is_empty() {
                    return Ok(());
                }
                let len = self.chats.len() as isize;
                let next = (self.chat_index as isize + delta).rem_euclid(len) as usize;
                self.chat_index = next;
                self.message_index = 0;
                self.refresh_messages()?;
            }
            Focus::Messages => {
                if self.messages.is_empty() {
                    return Ok(());
                }
                let len = self.messages.len() as isize;
                self.message_index = (self.message_index as isize + delta).rem_euclid(len) as usize;
            }
            Focus::Compose => {}
        }
        Ok(())
    }

    pub fn on_message(&mut self, record: MessageRecord) -> Result<()> {
        let open = self.selected_chat().map(|chat| chat.jid.clone());
        self.refresh_chats()?;
        if open.as_deref() == Some(record.chat_jid.as_str()) {
            self.refresh_messages()?;
            if !self.messages.is_empty() {
                self.message_index = self.messages.len() - 1;
            }
        }
        self.set_status(format!("new in {}", short_jid(&record.chat_jid)));
        Ok(())
    }

    pub fn begin_reply(&mut self) -> bool {
        let Some((msg_id, from_me, sender_jid)) = self.selected_message().map(|message| {
            (
                message.msg_id.clone(),
                message.from_me,
                message.sender_jid.clone(),
            )
        }) else {
            return false;
        };
        self.reply_to = Some(msg_id);
        self.reply_sender = if from_me { None } else { sender_jid };
        true
    }

    pub fn clear_reply(&mut self) {
        self.reply_to = None;
        self.reply_sender = None;
    }

    pub fn compose_payload(&self) -> Option<(String, String, Option<String>, Option<String>)> {
        let chat = self.selected_chat()?;
        let text = self.compose.trim();
        if text.is_empty() {
            return None;
        }
        if self.config.read_only {
            return None;
        }
        Some((
            chat.jid.clone(),
            text.to_string(),
            self.reply_to.clone(),
            self.reply_sender.clone(),
        ))
    }
}

pub fn short_jid(jid: &str) -> &str {
    jid.split('@').next().unwrap_or(jid)
}

pub fn chat_title(chat: &ChatRecord) -> String {
    chat.name
        .clone()
        .unwrap_or_else(|| short_jid(&chat.jid).to_string())
}

pub fn message_preview(message: &MessageRecord) -> String {
    message
        .display_text
        .clone()
        .or_else(|| message.text.clone())
        .or_else(|| message.media_type.as_ref().map(|kind| format!("[{kind}]")))
        .unwrap_or_else(|| "(empty)".into())
}
