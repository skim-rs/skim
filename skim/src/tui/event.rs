use std::sync::Arc;

use crossterm::event::KeyEvent;

use crate::SkimItem;
// use std::sync::mpsc::{Receiver, Sender};

// pub type EventReceiver = Receiver<(Key, Event)>;
// pub type EventSender = Sender<(Key, Event)>;

#[derive(Clone)]
pub enum Event {
    Quit,
    Error(String),
    Close,
    Tick,
    Render,
    Key(KeyEvent),
    PreviewReady(Vec<u8>),
    InvalidInput,
    Action(Action),
    NewItem(Arc<dyn SkimItem>),
    ClearItems,
    Clear,
    Heartbeat,
    RunPreview,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum Action {
    Abort,
    Accept(Option<String>),
    AddChar(char),
    AppendAndSelect,
    BackwardChar,
    BackwardDeleteChar,
    BackwardKillWord,
    BackwardWord,
    BeginningOfLine,
    Cancel,
    ClearScreen,
    DeleteChar,
    DeleteCharEOF,
    DeselectAll,
    Down(u16),
    EndOfLine,
    Execute(String),
    ExecuteSilent(String),
    ForwardChar,
    ForwardWord,
    IfQueryEmpty(String),
    IfQueryNotEmpty(String),
    IfNonMatched(String),
    Ignore,
    KillLine,
    KillWord,
    NextHistory,
    HalfPageDown(i32),
    HalfPageUp(i32),
    PageDown(i32),
    PageUp(i32),
    PreviewUp(i32),
    PreviewDown(i32),
    PreviewLeft(i32),
    PreviewRight(i32),
    PreviewPageUp(i32),
    PreviewPageDown(i32),
    PreviousHistory,
    Redraw,
    Reload(Option<String>),
    RefreshCmd,
    RefreshPreview,
    RestartMatcher,
    RotateMode,
    ScrollLeft(i32),
    ScrollRight(i32),
    SelectAll,
    SelectRow(usize),
    Toggle,
    ToggleAll,
    ToggleIn,
    ToggleInteractive,
    ToggleOut,
    TogglePreview,
    TogglePreviewWrap,
    ToggleSort,
    UnixLineDiscard,
    UnixWordRubout,
    Up(u16),
    Yank,
}

/// `Effect` is the effect of a text
pub enum UpdateScreen {
    Redraw,
    DontRedraw,
}

pub trait EventHandler {
    /// handle event, return whether
    fn handle(&mut self, event: &Event) -> UpdateScreen;
}

#[rustfmt::skip]
pub fn parse_action(raw_action: &str) -> Option<Action> {
  let mut parts = raw_action.split(&[':', '(', ')']);
  let action = parts.next().unwrap();
  let arg = parts.next().map(|s| s.to_string());

  match action {
        "abort"                =>   Some(Action::Abort),
        "accept"               =>   Some(Action::Accept(arg)),
        "append-and-select"    =>   Some(Action::AppendAndSelect),
        "backward-char"        =>   Some(Action::BackwardChar),
        "backward-delete-char" =>   Some(Action::BackwardDeleteChar),
        "backward-kill-word"   =>   Some(Action::BackwardKillWord),
        "backward-word"        =>   Some(Action::BackwardWord),
        "beginning-of-line"    =>   Some(Action::BeginningOfLine),
        "cancel"               =>   Some(Action::Cancel),
        "clear-screen"         =>   Some(Action::ClearScreen),
        "delete-char"          =>   Some(Action::DeleteChar),
        "delete-charEOF"       =>   Some(Action::DeleteCharEOF),
        "deselect-all"         =>   Some(Action::DeselectAll),
        "down"                 =>   Some(Action::Down(arg.and_then(|s|s.parse().ok()).unwrap_or(1))),
        "end-of-line"          =>   Some(Action::EndOfLine),
        "execute"              =>   Some(Action::Execute(arg.expect("execute event should have argument"))),
        "execute-silent"       =>   Some(Action::ExecuteSilent(arg.expect("execute-silent event should have argument"))),
        "forward-char"         =>   Some(Action::ForwardChar),
        "forward-word"         =>   Some(Action::ForwardWord),
        "if-non-matched"       =>   Some(Action::IfNonMatched(arg.expect("no arg specified for event if-non-matched"))),
        "if-query-empty"       =>   Some(Action::IfQueryEmpty(arg.expect("no arg specified for event if-query-empty"))),
        "if-query-not-empty"   =>   Some(Action::IfQueryNotEmpty(arg.expect("no arg specified for event if-query-not-empty"))),
        "ignore"               =>   Some(Action::Ignore),
        "kill-line"            =>   Some(Action::KillLine),
        "kill-word"            =>   Some(Action::KillWord),
        "next-history"         =>   Some(Action::NextHistory),
        "half-page-down"       =>   Some(Action::HalfPageDown(arg.and_then(|s|s.parse().ok()).unwrap_or(1))),
        "half-page-up"         =>   Some(Action::HalfPageUp(arg.and_then(|s|s.parse().ok()).unwrap_or(1))),
        "page-down"            =>   Some(Action::PageDown(arg.and_then(|s|s.parse().ok()).unwrap_or(1))),
        "page-up"              =>   Some(Action::PageUp(arg.and_then(|s|s.parse().ok()).unwrap_or(1))),
        "preview-up"           =>   Some(Action::PreviewUp(arg.and_then(|s|s.parse().ok()).unwrap_or(1))),
        "preview-down"         =>   Some(Action::PreviewDown(arg.and_then(|s|s.parse().ok()).unwrap_or(1))),
        "preview-left"         =>   Some(Action::PreviewLeft(arg.and_then(|s|s.parse().ok()).unwrap_or(1))),
        "preview-right"        =>   Some(Action::PreviewRight(arg.and_then(|s|s.parse().ok()).unwrap_or(1))),
        "preview-page-up"      =>   Some(Action::PreviewPageUp(arg.and_then(|s|s.parse().ok()).unwrap_or(1))),
        "preview-page-down"    =>   Some(Action::PreviewPageDown(arg.and_then(|s|s.parse().ok()).unwrap_or(1))),
        "previous-history"     =>   Some(Action::PreviousHistory),
        "refresh-cmd"          =>   Some(Action::RefreshCmd),
        "refresh-preview"      =>   Some(Action::RefreshPreview),
        "reload"               =>   Some(Action::Reload(arg.clone())),
        "scroll-left"          =>   Some(Action::ScrollLeft(arg.and_then(|s|s.parse().ok()).unwrap_or(1))),
        "scroll-right"         =>   Some(Action::ScrollRight(arg.and_then(|s|s.parse().ok()).unwrap_or(1))),
        "select-all"           =>   Some(Action::SelectAll),
        "toggle"               =>   Some(Action::Toggle),
        "toggle-all"           =>   Some(Action::ToggleAll),
        "toggle-in"            =>   Some(Action::ToggleIn),
        "toggle-interactive"   =>   Some(Action::ToggleInteractive),
        "toggle-out"           =>   Some(Action::ToggleOut),
        "toggle-preview"       =>   Some(Action::TogglePreview),
        "toggle-preview-wrap"  =>   Some(Action::TogglePreviewWrap),
        "toggle-sort"          =>   Some(Action::ToggleSort),
        "unix-line-discard"    =>   Some(Action::UnixLineDiscard),
        "unix-word-rubout"     =>   Some(Action::UnixWordRubout),
        "up"                   =>   Some(Action::Up(arg.and_then(|s|s.parse().ok()).unwrap_or(1))),
        "yank"                 =>   Some(Action::Yank),
        _ => None
    }
}
