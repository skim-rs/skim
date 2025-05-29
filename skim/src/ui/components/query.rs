use unicode_width::UnicodeWidthStr;

use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::ui::{SkimEvent, SkimMessage};
use crate::util::read_file_lines;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum QueryMode {
    Cmd,
    Query,
}

/// Enhanced query state for ratatui implementation
#[derive(Debug, Clone)]
pub struct QueryState {
    // Basic state (compatible with current UI system)
    pub content: String,
    pub cursor_pos: usize,
    pub prompt: String,
    
    // Extended state for full Query functionality
    cmd_before: Vec<char>,
    cmd_after: Vec<char>,
    fz_query_before: Vec<char>,
    fz_query_after: Vec<char>,
    yank: Vec<char>,

    mode: QueryMode,
    base_cmd: String,
    replstr: String,
    query_prompt: String,
    cmd_prompt: String,

    cmd_history_before: Vec<String>,
    cmd_history_after: Vec<String>,
    fz_query_history_before: Vec<String>,
    fz_query_history_after: Vec<String>,

    pasted: Option<String>,
}

impl Default for QueryState {
    fn default() -> Self {
        Self {
            content: String::new(),
            cursor_pos: 0,
            prompt: "> ".to_string(),
            
            cmd_before: Vec::new(),
            cmd_after: Vec::new(),
            fz_query_before: Vec::new(),
            fz_query_after: Vec::new(),
            yank: Vec::new(),
            mode: QueryMode::Query,
            base_cmd: String::new(),
            replstr: "{}".to_string(),
            query_prompt: "> ".to_string(),
            cmd_prompt: "c> ".to_string(),

            cmd_history_before: Vec::new(),
            cmd_history_after: Vec::new(),
            fz_query_history_before: Vec::new(),
            fz_query_history_after: Vec::new(),

            pasted: None,
        }
    }
}

impl QueryState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_prompt(mut self, prompt: String) -> Self {
        self.query_prompt = prompt.clone();
        self.prompt = prompt;
        self
    }

    pub fn with_cmd_prompt(mut self, prompt: String) -> Self {
        self.cmd_prompt = prompt;
        self
    }

    pub fn with_initial_query(mut self, query: String) -> Self {
        self.fz_query_before = query.chars().collect();
        self.sync_content();
        self
    }

    pub fn with_base_cmd(mut self, cmd: String) -> Self {
        self.base_cmd = cmd;
        self
    }

    pub fn with_interactive_mode(mut self, interactive: bool) -> Self {
        self.mode = if interactive { QueryMode::Cmd } else { QueryMode::Query };
        self.sync_content();
        self
    }

    pub fn with_history_file(mut self, file: Option<String>) -> Self {
        if let Some(file) = file {
            self.fz_query_history_before = read_file_lines(&file).unwrap_or_default();
        }
        self
    }

    pub fn with_cmd_history_file(mut self, file: Option<String>) -> Self {
        if let Some(file) = file {
            self.cmd_history_before = read_file_lines(&file).unwrap_or_default();
        }
        self
    }

    /// Configure from SkimOptions
    pub fn with_options(&mut self, options: &crate::SkimOptions) {
        if !options.prompt.is_empty() {
            self.query_prompt = options.prompt.clone();
            self.prompt = options.prompt.clone();
        }
        if !options.cmd_prompt.is_empty() {
            self.cmd_prompt = options.cmd_prompt.clone();
        }
        if let Some(ref query) = options.query {
            if !query.is_empty() {
                self.fz_query_before = query.chars().collect();
            }
        }
        if let Some(ref cmd) = options.cmd {
            if !cmd.is_empty() {
                self.base_cmd = cmd.clone();
            }
        }
        self.mode = if options.interactive { QueryMode::Cmd } else { QueryMode::Query };
        // History files handled differently in options
        // self.with_history_file(Some(options.history_file.clone()));
        // self.with_cmd_history_file(Some(options.cmd_history_file.clone()));
        self.sync_content();
    }

    /// Sync the internal state with the simple content/cursor_pos for compatibility
    fn sync_content(&mut self) {
        self.content = self.get_query();
        self.cursor_pos = self.get_before().chars().count();
        self.prompt = self.get_prompt().to_string();
    }

    pub fn in_query_mode(&self) -> bool {
        matches!(self.mode, QueryMode::Query)
    }

    pub fn get_fz_query(&self) -> String {
        self.fz_query_before
            .iter()
            .cloned()
            .chain(self.fz_query_after.iter().cloned().rev())
            .collect()
    }

    pub fn get_cmd(&self) -> String {
        let arg: String = self
            .cmd_before
            .iter()
            .cloned()
            .chain(self.cmd_after.iter().cloned().rev())
            .collect();
        self.base_cmd.replace(&self.replstr, &arg)
    }

    pub fn get_cmd_query(&self) -> String {
        self.cmd_before
            .iter()
            .cloned()
            .chain(self.cmd_after.iter().cloned().rev())
            .collect()
    }

    fn get_query(&self) -> String {
        match self.mode {
            QueryMode::Query => self.get_fz_query(),
            QueryMode::Cmd => self.get_cmd_query(),
        }
    }

    fn get_before(&self) -> String {
        match self.mode {
            QueryMode::Cmd => self.cmd_before.iter().cloned().collect(),
            QueryMode::Query => self.fz_query_before.iter().cloned().collect(),
        }
    }

    fn get_after(&self) -> String {
        match self.mode {
            QueryMode::Cmd => self.cmd_after.iter().cloned().rev().collect(),
            QueryMode::Query => self.fz_query_after.iter().cloned().rev().collect(),
        }
    }

    fn get_prompt(&self) -> &str {
        match self.mode {
            QueryMode::Cmd => &self.cmd_prompt,
            QueryMode::Query => &self.query_prompt,
        }
    }

    fn get_query_ref(&mut self) -> (&mut Vec<char>, &mut Vec<char>) {
        match self.mode {
            QueryMode::Query => (&mut self.fz_query_before, &mut self.fz_query_after),
            QueryMode::Cmd => (&mut self.cmd_before, &mut self.cmd_after),
        }
    }

    fn get_history_ref(&mut self) -> (&mut Vec<String>, &mut Vec<String>) {
        match self.mode {
            QueryMode::Query => (&mut self.fz_query_history_before, &mut self.fz_query_history_after),
            QueryMode::Cmd => (&mut self.cmd_history_before, &mut self.cmd_history_after),
        }
    }

    fn save_yank(&mut self, mut yank: Vec<char>, reverse: bool) {
        if yank.is_empty() {
            return;
        }

        self.yank.clear();

        if reverse {
            self.yank.append(&mut yank.into_iter().rev().collect());
        } else {
            self.yank.append(&mut yank);
        }
    }

    // Actions
    pub fn act_query_toggle_interactive(&mut self) {
        self.mode = match self.mode {
            QueryMode::Query => QueryMode::Cmd,
            QueryMode::Cmd => QueryMode::Query,
        };
        self.sync_content();
    }

    pub fn act_add_char(&mut self, ch: char) {
        let (before, _) = self.get_query_ref();
        before.push(ch);
        self.sync_content();
    }

    pub fn act_backward_delete_char(&mut self) {
        let (before, _) = self.get_query_ref();
        let _ = before.pop();
        self.sync_content();
    }

    pub fn act_delete_char(&mut self) {
        let (_, after) = self.get_query_ref();
        let _ = after.pop();
        self.sync_content();
    }

    pub fn act_backward_char(&mut self) {
        let (before, after) = self.get_query_ref();
        if let Some(ch) = before.pop() {
            after.push(ch);
        }
        self.sync_content();
    }

    pub fn act_forward_char(&mut self) {
        let (before, after) = self.get_query_ref();
        if let Some(ch) = after.pop() {
            before.push(ch);
        }
        self.sync_content();
    }

    pub fn act_beginning_of_line(&mut self) {
        let (before, after) = self.get_query_ref();
        while let Some(ch) = before.pop() {
            after.push(ch);
        }
        self.sync_content();
    }

    pub fn act_end_of_line(&mut self) {
        let (before, after) = self.get_query_ref();
        while let Some(ch) = after.pop() {
            before.push(ch);
        }
        self.sync_content();
    }

    pub fn act_unix_word_rubout(&mut self) {
        let mut yank = Vec::new();

        {
            let (before, _) = self.get_query_ref();
            // kill things other than whitespace
            while !before.is_empty() && before[before.len() - 1].is_whitespace() {
                yank.push(before.pop().unwrap());
            }

            // kill word until whitespace
            while !before.is_empty() && !before[before.len() - 1].is_whitespace() {
                yank.push(before.pop().unwrap());
            }
        }

        self.save_yank(yank, true);
        self.sync_content();
    }

    pub fn act_backward_kill_word(&mut self) {
        let mut yank = Vec::new();

        {
            let (before, _) = self.get_query_ref();
            // kill things other than alphanumeric
            while !before.is_empty() && !before[before.len() - 1].is_alphanumeric() {
                yank.push(before.pop().unwrap());
            }

            // kill word until whitespace (not alphanumeric)
            while !before.is_empty() && before[before.len() - 1].is_alphanumeric() {
                yank.push(before.pop().unwrap());
            }
        }

        self.save_yank(yank, true);
        self.sync_content();
    }

    pub fn act_kill_word(&mut self) {
        let mut yank = Vec::new();

        {
            let (_, after) = self.get_query_ref();

            // kill non alphanumeric
            while !after.is_empty() && !after[after.len() - 1].is_alphanumeric() {
                yank.push(after.pop().unwrap());
            }
            // kill alphanumeric
            while !after.is_empty() && after[after.len() - 1].is_alphanumeric() {
                yank.push(after.pop().unwrap());
            }
        }
        self.save_yank(yank, false);
        self.sync_content();
    }

    pub fn act_kill_line(&mut self) {
        let mut yank = Vec::new();
        let (_, after) = self.get_query_ref();
        
        while let Some(ch) = after.pop() {
            yank.push(ch);
        }
        
        self.save_yank(yank, false);
        self.sync_content();
    }

    pub fn act_yank(&mut self) {
        let yank_content = self.yank.clone();
        let (before, _) = self.get_query_ref();
        before.extend_from_slice(&yank_content);
        self.sync_content();
    }

    pub fn act_backward_word(&mut self) {
        let (before, after) = self.get_query_ref();
        // skip whitespace
        while !before.is_empty() && !before[before.len() - 1].is_alphanumeric() {
            if let Some(ch) = before.pop() {
                after.push(ch);
            }
        }

        // backward char until whitespace
        while !before.is_empty() && before[before.len() - 1].is_alphanumeric() {
            if let Some(ch) = before.pop() {
                after.push(ch);
            }
        }
        self.sync_content();
    }

    pub fn act_forward_word(&mut self) {
        let (before, after) = self.get_query_ref();
        // skip whitespace
        while !after.is_empty() && after[after.len() - 1].is_whitespace() {
            if let Some(ch) = after.pop() {
                before.push(ch);
            }
        }

        // forward char until whitespace
        while !after.is_empty() && after[after.len() - 1].is_alphanumeric() {
            if let Some(ch) = after.pop() {
                before.push(ch);
            }
        }
        self.sync_content();
    }

    pub fn act_clear_screen(&mut self) {
        // For query component, clear screen would typically mean clear the query
        let (before, after) = self.get_query_ref();
        before.clear();
        after.clear();
        self.sync_content();
    }
}

/// Handle query-specific events
pub fn handle_query_event(state: &mut QueryState, event: &SkimEvent) -> Option<SkimMessage> {
    match event {
        SkimEvent::Key(key) => {
            use crossterm::event::{KeyCode, KeyModifiers};
            
            match (key.code, key.modifiers) {
                // Basic character input
                (KeyCode::Char(c), KeyModifiers::NONE) => {
                    state.act_add_char(c);
                    Some(SkimMessage::UpdateQuery(state.get_query()))
                }
                (KeyCode::Char(c), KeyModifiers::SHIFT) => {
                    state.act_add_char(c);
                    Some(SkimMessage::UpdateQuery(state.get_query()))
                }
                
                // Navigation
                (KeyCode::Left, KeyModifiers::NONE) => {
                    state.act_backward_char();
                    None
                }
                (KeyCode::Right, KeyModifiers::NONE) => {
                    state.act_forward_char();
                    None
                }
                (KeyCode::Home, KeyModifiers::NONE) => {
                    state.act_beginning_of_line();
                    None
                }
                (KeyCode::End, KeyModifiers::NONE) => {
                    state.act_end_of_line();
                    None
                }
                
                // Word navigation
                (KeyCode::Left, KeyModifiers::ALT) => {
                    state.act_backward_word();
                    None
                }
                (KeyCode::Right, KeyModifiers::ALT) => {
                    state.act_forward_word();
                    None
                }
                
                // Deletion
                (KeyCode::Backspace, KeyModifiers::NONE) => {
                    state.act_backward_delete_char();
                    Some(SkimMessage::UpdateQuery(state.get_query()))
                }
                (KeyCode::Delete, KeyModifiers::NONE) => {
                    state.act_delete_char();
                    Some(SkimMessage::UpdateQuery(state.get_query()))
                }
                
                // Ctrl operations
                (KeyCode::Char('a'), KeyModifiers::CONTROL) => {
                    state.act_beginning_of_line();
                    None
                }
                (KeyCode::Char('e'), KeyModifiers::CONTROL) => {
                    state.act_end_of_line();
                    None
                }
                (KeyCode::Char('b'), KeyModifiers::CONTROL) => {
                    state.act_backward_char();
                    None
                }
                (KeyCode::Char('f'), KeyModifiers::CONTROL) => {
                    state.act_forward_char();
                    None
                }
                (KeyCode::Char('h'), KeyModifiers::CONTROL) => {
                    state.act_backward_delete_char();
                    Some(SkimMessage::UpdateQuery(state.get_query()))
                }
                (KeyCode::Char('d'), KeyModifiers::CONTROL) => {
                    state.act_delete_char();
                    Some(SkimMessage::UpdateQuery(state.get_query()))
                }
                (KeyCode::Char('k'), KeyModifiers::CONTROL) => {
                    state.act_kill_line();
                    Some(SkimMessage::UpdateQuery(state.get_query()))
                }
                (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
                    state.act_unix_word_rubout();
                    Some(SkimMessage::UpdateQuery(state.get_query()))
                }
                (KeyCode::Char('w'), KeyModifiers::CONTROL) => {
                    state.act_backward_kill_word();
                    Some(SkimMessage::UpdateQuery(state.get_query()))
                }
                (KeyCode::Char('y'), KeyModifiers::CONTROL) => {
                    state.act_yank();
                    Some(SkimMessage::UpdateQuery(state.get_query()))
                }
                (KeyCode::Char('l'), KeyModifiers::CONTROL) => {
                    state.act_clear_screen();
                    Some(SkimMessage::UpdateQuery(state.get_query()))
                }
                (KeyCode::Char('q'), KeyModifiers::CONTROL) => {
                    state.act_query_toggle_interactive();
                    Some(SkimMessage::UpdateQuery(state.get_query()))
                }
                
                // Alt operations  
                (KeyCode::Backspace, KeyModifiers::ALT) => {
                    state.act_backward_kill_word();
                    Some(SkimMessage::UpdateQuery(state.get_query()))
                }
                (KeyCode::Char('d'), KeyModifiers::ALT) => {
                    state.act_kill_word();
                    Some(SkimMessage::UpdateQuery(state.get_query()))
                }
                
                _ => None,
            }
        }
        _ => None,
    }
}

/// Render the query component with ratatui
pub fn render_query(state: &QueryState, frame: &mut Frame, area: Rect) {
    let query_text = format!("{}{}", state.get_prompt(), state.get_query());
    
    // Create spans for syntax highlighting
    let spans = vec![
        Span::styled(state.get_prompt(), Style::default().fg(Color::Yellow)),
        Span::styled(state.get_query(), Style::default().fg(Color::White)),
    ];
    
    let input = Paragraph::new(Line::from(spans))
        .style(Style::default())
        .block(Block::default().borders(Borders::NONE));
    
    frame.render_widget(input, area);
    
    // Calculate and set cursor position
    let prompt_width = state.get_prompt().width();
    let cursor_x = area.x + prompt_width as u16 + state.cursor_pos as u16;
    if cursor_x < area.x + area.width {
        frame.set_cursor(cursor_x, area.y);
    }
}