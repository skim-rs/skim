use crossterm::event::{KeyEvent, MouseEvent};

/// Events that can occur during skim's execution
#[derive(Clone)]
pub enum Event {
    /// Quit the application
    Quit,
    /// An error occurred
    Error(String),
    /// Close the application
    Close,
    /// Timer tick event
    Tick,
    /// Render the UI
    Render,
    /// A key was pressed
    Key(KeyEvent),
    /// A mouse event occurred
    Mouse(MouseEvent),
    /// Preview content is ready to display
    PreviewReady(Vec<u8>),
    /// Invalid input received
    InvalidInput,
    /// An action was triggered
    Action(Action),
    /// Clear all items
    ClearItems,
    /// Clear the screen
    Clear,
    /// Heartbeat event
    Heartbeat,
    /// Run the preview command
    RunPreview,
    /// Redraw the screen
    Redraw,
    /// Reload with a new command
    Reload(String),
}

/// Actions that can be performed in skim
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum Action {
    /// Abort and exit with error
    Abort,
    /// Accept selection and exit with optional key
    Accept(Option<String>),
    /// Add a character to the query
    AddChar(char),
    /// Append to selection and select
    AppendAndSelect,
    /// Move cursor backward one character
    BackwardChar,
    /// Delete character before cursor
    BackwardDeleteChar,
    /// Delete word before cursor
    BackwardKillWord,
    /// Move cursor backward one word
    BackwardWord,
    /// Move cursor to beginning of line
    BeginningOfLine,
    /// Cancel current operation
    Cancel,
    /// Clear the screen
    ClearScreen,
    /// Delete character under cursor
    DeleteChar,
    /// Delete character or exit if empty
    DeleteCharEOF,
    /// Deselect all items
    DeselectAll,
    /// Move selection down by N items
    Down(u16),
    /// Move cursor to end of line
    EndOfLine,
    /// Execute a command
    Execute(String),
    /// Execute a command silently
    ExecuteSilent(String),
    /// Jump to first item in list
    First,
    /// Move cursor forward one character
    ForwardChar,
    /// Move cursor forward one word
    ForwardWord,
    /// Execute action if query is empty
    IfQueryEmpty(String, Option<String>),
    /// Execute action if query is not empty
    IfQueryNotEmpty(String, Option<String>),
    /// Execute action if no items match
    IfNonMatched(String, Option<String>),
    /// Ignore the action
    Ignore,
    /// Delete from cursor to end of line
    KillLine,
    /// Delete word after cursor
    KillWord,
    /// Jump to last item in list
    Last,
    /// Move to next history entry
    NextHistory,
    /// Scroll down by half a page
    HalfPageDown(i32),
    /// Scroll up by half a page
    HalfPageUp(i32),
    /// Scroll down by a page
    PageDown(i32),
    /// Scroll up by a page
    PageUp(i32),
    /// Scroll preview up
    PreviewUp(i32),
    /// Scroll preview down
    PreviewDown(i32),
    /// Scroll preview left
    PreviewLeft(i32),
    /// Scroll preview right
    PreviewRight(i32),
    /// Scroll preview up by a page
    PreviewPageUp(i32),
    /// Scroll preview down by a page
    PreviewPageDown(i32),
    /// Move to previous history entry
    PreviousHistory,
    /// Redraw the screen
    Redraw,
    /// Reload with optional new command
    Reload(Option<String>),
    /// Refresh the command
    RefreshCmd,
    /// Refresh the preview
    RefreshPreview,
    /// Restart the matcher
    RestartMatcher,
    /// Rotate through matching modes
    RotateMode,
    /// Scroll item list left
    ScrollLeft(i32),
    /// Scroll item list right
    ScrollRight(i32),
    /// Select all items
    SelectAll,
    /// Select a specific row
    SelectRow(usize),
    /// Select current item
    Select,
    /// Toggle selection of current item
    Toggle,
    /// Toggle selection of all items
    ToggleAll,
    /// Toggle and move in
    ToggleIn,
    /// Toggle interactive mode
    ToggleInteractive,
    /// Toggle and move out
    ToggleOut,
    /// Toggle preview visibility
    TogglePreview,
    /// Toggle preview line wrapping
    TogglePreviewWrap,
    /// Toggle sorting
    ToggleSort,
    /// Jump to first item in list (alias for First)
    Top,
    /// Discard line (unix-style)
    UnixLineDiscard,
    /// Delete word backward (unix-style)
    UnixWordRubout,
    /// Move selection up by N items
    Up(u16),
    /// Yank (paste)
    Yank,
}

/// Parses an action string into an Action enum
#[rustfmt::skip]
pub fn parse_action(raw_action: &str) -> Option<Action> {
  let parts = raw_action.split_once([':', '(', ')']);
  let action;
  let mut arg = None;
  match parts {
    None => { action = raw_action }
    Some((act, "")) => { action = act }
    Some((act, a)) => { action = act; arg = Some(a.trim_end_matches(")").to_string()) }
  }
  debug!("parse_action: action={action}, arg={arg:?}");

  // Parse `if` chains
  if action.starts_with("if-") {
    let then_arg;
    let mut otherwise_arg = None;

    let if_arg = arg.unwrap_or_else(|| panic!("no arg specified for event {action}"));
    if if_arg.contains("+") {
      let split = if_arg.split_once("+");
      match split {
        Some((a, "")) => { then_arg = a.to_string(); }
        Some((a, b)) => {
          then_arg = a.to_string();
          otherwise_arg = Some(b.to_string());
        }
        None => unreachable!()
      }
    } else {
      then_arg = if_arg.to_string();
    }
    match action {
      "if-non-matched"       =>   Some(Action::IfNonMatched(then_arg, otherwise_arg)),
      "if-query-empty"       =>   Some(Action::IfQueryEmpty(then_arg, otherwise_arg)),
      "if-query-not-empty"   =>   Some(Action::IfQueryNotEmpty(then_arg, otherwise_arg)),
      _ => None
    }
  } else {
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
          "first"                =>   Some(Action::First),
          "forward-char"         =>   Some(Action::ForwardChar),
          "forward-word"         =>   Some(Action::ForwardWord),
          "ignore"               =>   Some(Action::Ignore),
          "kill-line"            =>   Some(Action::KillLine),
          "kill-word"            =>   Some(Action::KillWord),
          "last"                 =>   Some(Action::Last),
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
          "top"                  =>   Some(Action::Top),
          "unix-line-discard"    =>   Some(Action::UnixLineDiscard),
          "unix-word-rubout"     =>   Some(Action::UnixWordRubout),
          "up"                   =>   Some(Action::Up(arg.and_then(|s|s.parse().ok()).unwrap_or(1))),
          "yank"                 =>   Some(Action::Yank),
          _ => None
  }

    }
}
