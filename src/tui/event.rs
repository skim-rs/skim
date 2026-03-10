use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use crate::exhaustive_match;
use crossterm::event::{KeyEvent, MouseEvent};
use derive_more::{Debug, Eq, PartialEq};

type BoxError = Box<dyn std::error::Error + Sync + Send>;
type BoxFuture<'a> = Pin<Box<dyn Future<Output = Result<Vec<Event>, BoxError>> + Send + 'a>>;

/// Trait object stored inside [`ActionCallback`].
///
/// Having an explicit trait (rather than a bare `dyn Fn` type alias) allows
/// Rust to correctly resolve the higher-ranked lifetime in the return type.
trait AsyncCallbackFn: Send {
    fn call<'a>(&'a self, app: &'a mut crate::tui::App) -> BoxFuture<'a>;
}

/// Adapter that stores a concrete async closure and implements [`AsyncCallbackFn`].
struct AsyncFnWrapper<F>(F);

impl<F, Fut> AsyncCallbackFn for AsyncFnWrapper<F>
where
    F: for<'a> Fn(&'a mut crate::tui::App) -> Fut + Send,
    Fut: Future<Output = Result<Vec<Event>, BoxError>> + Send + 'static,
{
    fn call<'a>(&'a self, app: &'a mut crate::tui::App) -> BoxFuture<'a> {
        Box::pin((self.0)(app))
    }
}

/// Adapter that stores a plain synchronous closure and implements [`AsyncCallbackFn`].
struct SyncFnWrapper<F>(F);

impl<F> AsyncCallbackFn for SyncFnWrapper<F>
where
    F: Fn(&mut crate::tui::App) -> Result<Vec<Event>, BoxError> + Send,
{
    fn call<'a>(&'a self, app: &'a mut crate::tui::App) -> BoxFuture<'a> {
        Box::pin(std::future::ready((self.0)(app)))
    }
}

/// A custom action callback that receives a mutable reference to the App.
///
/// The closure will be called with a mutable reference to App and should return
/// a vec of events that will be processed after the callback completes.
///
/// Both sync and async closures are supported:
/// - Use [`ActionCallback::new`] to wrap an **async** closure or block.
/// - Use [`ActionCallback::new_sync`] to wrap a plain synchronous closure.
#[derive(Clone)]
pub struct ActionCallback(Arc<Mutex<dyn AsyncCallbackFn>>);

impl std::fmt::Debug for ActionCallback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ActionCallback").finish()
    }
}

impl ActionCallback {
    /// Create a new action callback from an **async** closure or block.
    ///
    /// ```rust,ignore
    /// ActionCallback::new(|app| async move {
    ///     // async work here …
    ///     Ok(vec![])
    /// });
    /// ```
    pub fn new<F, Fut>(f: F) -> Self
    where
        F: for<'a> Fn(&'a mut crate::tui::App) -> Fut + Send + 'static,
        Fut: Future<Output = Result<Vec<Event>, BoxError>> + Send + 'static,
    {
        Self(Arc::new(Mutex::new(AsyncFnWrapper(f))))
    }

    /// Create a new action callback from a plain **synchronous** closure.
    ///
    /// This is a convenience wrapper; the closure is lifted into an immediately-
    /// resolving future so it integrates with the same async call site.
    ///
    /// ```rust,ignore
    /// ActionCallback::new_sync(|app| {
    ///     Ok(vec![Event::Action(Action::SelectAll)])
    /// });
    /// ```
    pub fn new_sync<F>(f: F) -> Self
    where
        F: Fn(&mut crate::tui::App) -> Result<Vec<Event>, BoxError> + Send + 'static,
    {
        Self(Arc::new(Mutex::new(SyncFnWrapper(f))))
    }

    /// Call the callback with an App reference, driving the returned future to completion.
    ///
    /// Must be called from within a Tokio multi-thread runtime context.
    pub(crate) fn call(&self, app: &mut crate::tui::App) -> Result<Vec<Event>, BoxError> {
        let callback = self.0.lock().unwrap();
        let fut = callback.call(app);
        // We are inside a synchronous call stack that originates from an async
        // tokio context.  `block_in_place` moves the current thread out of the
        // async worker pool temporarily so we can block on the future without
        // starving the runtime.
        tokio::task::block_in_place(|| tokio::runtime::Handle::current().block_on(fut))
    }
}

/// Events that can occur during skim's execution
#[derive(Clone, Debug)]
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
    /// Text was pasted (bracketed paste)
    Paste(String),
    /// A mouse event occurred
    Mouse(MouseEvent),
    /// Preview content is ready to display
    PreviewReady,
    /// Invalid input received
    InvalidInput,
    /// An action was triggered
    Action(Action),
    /// Append items to the pool
    AppendItems(Vec<Arc<dyn crate::SkimItem>>),
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
    /// Terminal was resized to (columns, rows)
    Resize(u16, u16),
}

/// Actions that can be performed in skim
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
    /// Delete character before cursor or exit if the query is empty
    BackwardDeleteCharEof,
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
    DeleteCharEof,
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
    /// Refresh the command
    RefreshCmd,
    /// Refresh the preview
    RefreshPreview,
    /// Restart the matcher
    RestartMatcher,
    /// Reload with optional new command
    Reload(Option<String>),
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
    /// Set the header (or disable it on an empty value)
    SetHeader(Option<String>),
    /// Set the preview cmd and rerun preview
    SetPreviewCmd(String),
    /// Set the query to the expanded value
    SetQuery(String),
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
    /// Custom action from lib
    #[debug("custom")]
    #[eq(skip)]
    #[partial_eq(skip)]
    #[serde(skip)]
    Custom(ActionCallback),
}

/// Parses an action string into an Action enum
pub fn parse_action(raw_action: &str) -> Option<Action> {
    let parts = raw_action.split_once([':', '(', ')']);
    let action;
    let mut arg = None;
    match parts {
        None => action = raw_action,
        Some((act, "")) => action = act,
        Some((act, a)) => {
            action = act;
            arg = Some(a.trim_end_matches(")").to_string())
        }
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
                Some((a, "")) => {
                    then_arg = a.to_string();
                }
                Some((a, b)) => {
                    then_arg = a.to_string();
                    otherwise_arg = Some(b.to_string());
                }
                None => unreachable!(),
            }
        } else {
            then_arg = if_arg.to_string();
        }
        match action {
            "if-non-matched" => Some(Action::IfNonMatched(then_arg, otherwise_arg)),
            "if-query-empty" => Some(Action::IfQueryEmpty(then_arg, otherwise_arg)),
            "if-query-not-empty" => Some(Action::IfQueryNotEmpty(then_arg, otherwise_arg)),
            _ => None,
        }
    } else {
        exhaustive_match! {
            action => Option<Action>;
            {
                "abort" => Some(Abort),
                "accept" => Some(Accept(arg)),
                "add-char" => Some(AddChar(
                    arg.unwrap_or_default()
                        .chars()
                        .next()
                        .expect("add-char should have an argument"),
                )),
                "append-and-select" => Some(AppendAndSelect),
                "backward-char" => Some(BackwardChar),
                "backward-delete-char" => Some(BackwardDeleteChar),
                "backward-delete-char/eof" => Some(BackwardDeleteCharEof),
                "backward-kill-word" => Some(BackwardKillWord),
                "backward-word" => Some(BackwardWord),
                "beginning-of-line" => Some(BeginningOfLine),
                "cancel" => Some(Cancel),
                "clear-screen" => Some(ClearScreen),
                "delete-char" => Some(DeleteChar),
                "delete-char/eof" => Some(DeleteCharEof),
                "deselect-all" => Some(DeselectAll),
                "down" => Some(Down(arg.and_then(|s| s.parse().ok()).unwrap_or(1))),
                "end-of-line" => Some(EndOfLine),
                "execute" => Some(Execute(arg.expect("execute event should have argument"))),
                "execute-silent" => Some(ExecuteSilent(arg.expect("execute-silent event should have argument"))),
                "first" => Some(First),
                "forward-char" => Some(ForwardChar),
                "forward-word" => Some(ForwardWord),
                "ignore" => Some(Ignore),
                "kill-line" => Some(KillLine),
                "kill-word" => Some(KillWord),
                "last" => Some(Last),
                "next-history" => Some(NextHistory),
                "half-page-down" => Some(HalfPageDown(arg.and_then(|s| s.parse().ok()).unwrap_or(1))),
                "half-page-up" => Some(HalfPageUp(arg.and_then(|s| s.parse().ok()).unwrap_or(1))),
                "page-down" => Some(PageDown(arg.and_then(|s| s.parse().ok()).unwrap_or(1))),
                "page-up" => Some(PageUp(arg.and_then(|s| s.parse().ok()).unwrap_or(1))),
                "preview-up" => Some(PreviewUp(arg.and_then(|s| s.parse().ok()).unwrap_or(1))),
                "preview-down" => Some(PreviewDown(arg.and_then(|s| s.parse().ok()).unwrap_or(1))),
                "preview-left" => Some(PreviewLeft(arg.and_then(|s| s.parse().ok()).unwrap_or(1))),
                "preview-right" => Some(PreviewRight(arg.and_then(|s| s.parse().ok()).unwrap_or(1))),
                "preview-page-up" => Some(PreviewPageUp(arg.and_then(|s| s.parse().ok()).unwrap_or(1))),
                "preview-page-down" => Some(PreviewPageDown(arg.and_then(|s| s.parse().ok()).unwrap_or(1))),
                "previous-history" => Some(PreviousHistory),
                "redraw" => Some(Redraw),
                "refresh-cmd" => Some(RefreshCmd),
                "refresh-preview" => Some(RefreshPreview),
                "restart-matcher" => Some(RestartMatcher),
                "reload" => Some(Reload(arg.clone())),
                "rotate-mode" => Some(RotateMode),
                "scroll-left" => Some(ScrollLeft(arg.and_then(|s| s.parse().ok()).unwrap_or(1))),
                "scroll-right" => Some(ScrollRight(arg.and_then(|s| s.parse().ok()).unwrap_or(1))),
                "select" => Some(Select),
                "select-all" => Some(SelectAll),
                "select-row" => Some(SelectRow(arg.and_then(|s| s.parse().ok()).unwrap_or_default())),
                "set-header" => Some(SetHeader(arg)),
                "set-preview-cmd" => Some(SetPreviewCmd(arg.expect("set-preview-cmd action needs a value"))),
                "set-query" => Some(SetQuery(arg.expect("set-query action needs a value"))),
                "toggle" => Some(Toggle),
                "toggle-all" => Some(ToggleAll),
                "toggle-in" => Some(ToggleIn),
                "toggle-interactive" => Some(ToggleInteractive),
                "toggle-out" => Some(ToggleOut),
                "toggle-preview" => Some(TogglePreview),
                "toggle-preview-wrap" => Some(TogglePreviewWrap),
                "toggle-sort" => Some(ToggleSort),
                "top" => Some(Top),
                "unix-line-discard" => Some(UnixLineDiscard),
                "unix-word-rubout" => Some(UnixWordRubout),
                "up" => Some(Up(arg.and_then(|s| s.parse().ok()).unwrap_or(1))),
                "yank" => Some(Yank),
                "unreachable-if-non-matched" => Some(IfNonMatched(Default::default(), None)),
                "unreachable-if-query-empty" => Some(IfQueryEmpty(Default::default(), None)),
                "unreachable-if-query-not-empty" => Some(IfQueryNotEmpty(Default::default(), None)),
                "custom-do-not-use-from-cli" => Some(Custom(ActionCallback::new_sync(|_: &mut crate::tui::App| { Ok(Vec::new()) }))),
            }
            default _ => None
        }
    }
}
