use crossterm::event::KeyEvent;
// use std::sync::mpsc::{Receiver, Sender};

// pub type EventReceiver = Receiver<(Key, Event)>;
// pub type EventSender = Sender<(Key, Event)>;

#[derive(Clone, Debug)]
pub enum Event {
    Quit,
    Error(String),
    Close,
    Tick,
    Render,
    Key(KeyEvent),
    PreviewReady(Vec<u8>),
    InvalidInput,
    Action(Action)
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
    Down(i32),
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
    Up(i32),
    Yank,

    #[doc(hidden)]
    __Nonexhaustive,
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
pub fn parse_event(action: &str, arg: Option<String>) -> Option<Event> {
    match action {
        "abort"                =>   Some(Event::Action(Action::Abort)),
        "accept"               =>   Some(Event::Action(Action::Accept(arg))),
        "append-and-select"    =>   Some(Event::Action(Action::AppendAndSelect)),
        "backward-char"        =>   Some(Event::Action(Action::BackwardChar)),
        "backward-delete-char" =>   Some(Event::Action(Action::BackwardDeleteChar)),
        "backward-kill-word"   =>   Some(Event::Action(Action::BackwardKillWord)),
        "backward-word"        =>   Some(Event::Action(Action::BackwardWord)),
        "beginning-of-line"    =>   Some(Event::Action(Action::BeginningOfLine)),
        "cancel"               =>   Some(Event::Action(Action::Cancel)),
        "clear-screen"         =>   Some(Event::Action(Action::ClearScreen)),
        "delete-char"          =>   Some(Event::Action(Action::DeleteChar)),
        "delete-charEOF"       =>   Some(Event::Action(Action::DeleteCharEOF)),
        "deselect-all"         =>   Some(Event::Action(Action::DeselectAll)),
        "down"                 =>   Some(Event::Action(Action::Down(arg.and_then(|s|s.parse().ok()).unwrap_or(1)))),
        "end-of-line"          =>   Some(Event::Action(Action::EndOfLine)),
        "execute"              =>   Some(Event::Action(Action::Execute(arg.expect("execute event should have argument")))),
        "execute-silent"       =>   Some(Event::Action(Action::ExecuteSilent(arg.expect("execute-silent event should have argument")))),
        "forward-char"         =>   Some(Event::Action(Action::ForwardChar)),
        "forward-word"         =>   Some(Event::Action(Action::ForwardWord)),
        "if-non-matched"       =>   Some(Event::Action(Action::IfNonMatched(arg.expect("no arg specified for event if-non-matched")))),
        "if-query-empty"       =>   Some(Event::Action(Action::IfQueryEmpty(arg.expect("no arg specified for event if-query-empty")))),
        "if-query-not-empty"   =>   Some(Event::Action(Action::IfQueryNotEmpty(arg.expect("no arg specified for event if-query-not-empty")))),
        "ignore"               =>   Some(Event::Action(Action::Ignore)),
        "kill-line"            =>   Some(Event::Action(Action::KillLine)),
        "kill-word"            =>   Some(Event::Action(Action::KillWord)),
        "next-history"         =>   Some(Event::Action(Action::NextHistory)),
        "half-page-down"       =>   Some(Event::Action(Action::HalfPageDown(arg.and_then(|s|s.parse().ok()).unwrap_or(1)))),
        "half-page-up"         =>   Some(Event::Action(Action::HalfPageUp(arg.and_then(|s|s.parse().ok()).unwrap_or(1)))),
        "page-down"            =>   Some(Event::Action(Action::PageDown(arg.and_then(|s|s.parse().ok()).unwrap_or(1)))),
        "page-up"              =>   Some(Event::Action(Action::PageUp(arg.and_then(|s|s.parse().ok()).unwrap_or(1)))),
        "preview-up"           =>   Some(Event::Action(Action::PreviewUp(arg.and_then(|s|s.parse().ok()).unwrap_or(1)))),
        "preview-down"         =>   Some(Event::Action(Action::PreviewDown(arg.and_then(|s|s.parse().ok()).unwrap_or(1)))),
        "preview-left"         =>   Some(Event::Action(Action::PreviewLeft(arg.and_then(|s|s.parse().ok()).unwrap_or(1)))),
        "preview-right"        =>   Some(Event::Action(Action::PreviewRight(arg.and_then(|s|s.parse().ok()).unwrap_or(1)))),
        "preview-page-up"      =>   Some(Event::Action(Action::PreviewPageUp(arg.and_then(|s|s.parse().ok()).unwrap_or(1)))),
        "preview-page-down"    =>   Some(Event::Action(Action::PreviewPageDown(arg.and_then(|s|s.parse().ok()).unwrap_or(1)))),
        "previous-history"     =>   Some(Event::Action(Action::PreviousHistory)),
        "refresh-cmd"          =>   Some(Event::Action(Action::RefreshCmd)),
        "refresh-preview"      =>   Some(Event::Action(Action::RefreshPreview)),
        "reload"               =>   Some(Event::Action(Action::Reload(arg.clone()))),
        "scroll-left"          =>   Some(Event::Action(Action::ScrollLeft(arg.and_then(|s|s.parse().ok()).unwrap_or(1)))),
        "scroll-right"         =>   Some(Event::Action(Action::ScrollRight(arg.and_then(|s|s.parse().ok()).unwrap_or(1)))),
        "select-all"           =>   Some(Event::Action(Action::SelectAll)),
        "toggle"               =>   Some(Event::Action(Action::Toggle)),
        "toggle-all"           =>   Some(Event::Action(Action::ToggleAll)),
        "toggle-in"            =>   Some(Event::Action(Action::ToggleIn)),
        "toggle-interactive"   =>   Some(Event::Action(Action::ToggleInteractive)),
        "toggle-out"           =>   Some(Event::Action(Action::ToggleOut)),
        "toggle-preview"       =>   Some(Event::Action(Action::TogglePreview)),
        "toggle-preview-wrap"  =>   Some(Event::Action(Action::TogglePreviewWrap)),
        "toggle-sort"          =>   Some(Event::Action(Action::ToggleSort)),
        "unix-line-discard"    =>   Some(Event::Action(Action::UnixLineDiscard)),
        "unix-word-rubout"     =>   Some(Event::Action(Action::UnixWordRubout)),
        "up"                   =>   Some(Event::Action(Action::Up(arg.and_then(|s|s.parse().ok()).unwrap_or(1)))),
        "yank"                 =>   Some(Event::Action(Action::Yank)),
        _ => None
    }
}
