//! Action definitions, the action catalog, and action parsing.
//!
//! The [`Action`] enum, its canonical names and its parser are all generated
//! from a single source of truth: the [`define_action_catalog`] invocation at
//! the bottom of this module. Each entry declares the variant (with its doc
//! comment and payload types), the kebab-case name accepted by `--bind`, and
//! the expression that builds the variant from an optional argument.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use derive_more::{Debug, Eq, PartialEq};

use super::event::Event;

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

fn parse_conditional(arg: Option<String>, constructor: fn(String, Option<String>) -> Action) -> Option<Action> {
    let arg = arg?;
    let (then, otherwise) = match arg.split_once('+') {
        Some((then, "")) => (then, None),
        Some((then, otherwise)) => (then, Some(otherwise.to_string())),
        None => (arg.as_str(), None),
    };
    Some(constructor(then.to_string(), otherwise))
}

/// Documentation for a single entry of the action catalog.
///
/// Produced by `define_action_catalog!` and exposed through [`ACTION_CATALOG`];
/// used to generate the actions list of the manpage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionDoc {
    /// Canonical kebab-case name, as accepted by [`parse_action`].
    pub name: &'static str,
    /// Whether the action carries an argument (`name(...)` / `name:...`).
    pub takes_arg: bool,
    /// The action's rustdoc, one line per doc comment line.
    pub doc: &'static str,
}

impl ActionDoc {
    /// The action as it is spelled in a binding, with `(...)` for actions taking an argument.
    #[must_use]
    pub fn display_name(&self) -> String {
        if self.takes_arg {
            format!("{}(...)", self.name)
        } else {
            self.name.to_string()
        }
    }

    /// The action's documentation collapsed into a single line.
    #[must_use]
    pub fn summary(&self) -> String {
        self.doc
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Whether this action can be named in a binding.
    ///
    /// [`Action::Custom`] exists only for library users building actions in
    /// Rust, so it has no spelling [`parse_action`] accepts.
    #[must_use]
    pub fn is_bindable(&self) -> bool {
        // Actions that require an argument only parse with one, so retry with
        // the parser's empty placeholder before giving up.
        parse_action(self.name)
            .or_else(|| parse_action(&format!("{}()", self.name)))
            .is_some()
    }
}

/// Expands to the argument marker used in the manpage, ignoring the payload types it is handed.
macro_rules! action_arg_marker {
    ($($payload:tt)*) => {
        true
    };
}

/// Declares the whole action catalog in one place.
///
/// Every entry has the shape
///
/// ```text
/// /// doc comment
/// Variant(payload types…) => "canonical-name" => constructor expression
/// ```
///
/// Doc comments are captured (so they can be re-emitted on the variant *and*
/// rendered into the manpage), which means any other attribute has to be passed
/// through the optional `@attrs[…]` group instead — a bare `#[…]` would be
/// ambiguous with the doc comments:
///
/// ```text
/// /// doc comment
/// @attrs[debug("custom")]
/// Variant(payload) => "canonical-name" => constructor expression
/// ```
///
/// and the macro generates, from that single list:
/// - the [`Action`] enum (with the doc comments and payloads as written),
/// - [`Action::name`], mapping each variant to its canonical name,
/// - `parse_named_action`, mapping a name plus optional argument back to a variant,
/// - [`ACTION_CATALOG`], the name/argument/documentation list the manpage is generated from.
///
/// The identifier before the `;` is the name bound to the optional argument
/// (`Option<String>`) inside the constructor expressions.
macro_rules! define_action_catalog {
    (
        $arg:ident;
        $(
            $(#[doc = $doc:literal])*
            $(@attrs[$($attr:meta),+ $(,)?])?
            $variant:ident $(($($payload:ty),+ $(,)?))? => $name:literal => $parsed:expr
        ),+ $(,)?
    ) => {
        /// Actions that can be performed in skim
        #[derive(Debug, Clone, PartialEq, Eq)]
        #[cfg_attr(feature = "listen", derive(serde::Serialize, serde::Deserialize))]
        pub enum Action {
            $(
                $(#[doc = $doc])*
                $($(#[$attr])+)?
                $variant $(($($payload),+))?,
            )+
        }

        /// Every action, in declaration order, with its name, argument marker and documentation.
        ///
        /// This is the source the manpage's action list is generated from, so a
        /// new entry in the catalog is documented automatically.
        pub const ACTION_CATALOG: &[ActionDoc] = &[
            $(ActionDoc {
                name: $name,
                takes_arg: false $(|| action_arg_marker!($($payload)+))?,
                doc: concat!($($doc, "\n"),*),
            }),+
        ];

        impl Action {
            /// Returns the canonical kebab-case name of this action — the same spelling
            /// [`parse_action`] accepts.
            ///
            /// This lets an action be bound as if it were an event (e.g. `reload:first`):
            /// after the action runs, any follow-up chain keyed by this name is queued.
            /// The name ignores the action's arguments, so `down` matches `Down(1)` and
            /// `Down(5)` alike.
            #[must_use]
            pub fn name(&self) -> &'static str {
                match self {
                    $(Self::$variant { .. } => $name),+
                }
            }
        }

        fn parse_named_action(action: &str, $arg: Option<String>) -> Option<Action> {
            #[allow(clippy::enum_glob_use)]
            use Action::*;
            match action {
                $($name => $parsed),+,
                _ => None,
            }
        }
    };
}

define_action_catalog! {
    arg;
    /// Abort and exit with error
    Abort => "abort" => Some(Abort),
    /// Accept selection and exit with optional key.
    ///
    /// The argument is printed when the binding is triggered.
    Accept(Option<String>) => "accept" => Some(Accept(arg)),
    /// Add a character to the query
    AddChar(char) => "add-char" => arg.map(|s| AddChar(s.chars().next().unwrap_or_default())),
    /// Append to selection and select
    AppendAndSelect => "append-and-select" => Some(AppendAndSelect),
    /// Move cursor backward one character
    BackwardChar => "backward-char" => Some(BackwardChar),
    /// Delete character before cursor
    BackwardDeleteChar => "backward-delete-char" => Some(BackwardDeleteChar),
    /// Delete character before cursor or exit if the query is empty
    BackwardDeleteCharEof => "backward-delete-char/eof" => Some(BackwardDeleteCharEof),
    /// Delete word before cursor
    BackwardKillWord => "backward-kill-word" => Some(BackwardKillWord),
    /// Move cursor backward one word
    BackwardWord => "backward-word" => Some(BackwardWord),
    /// Move cursor to beginning of line
    BeginningOfLine => "beginning-of-line" => Some(BeginningOfLine),
    /// Bind one or more keys to action chains.
    ///
    /// The argument is a comma-separated list of `trigger:action[+action]` bindings to add,
    /// using the same syntax as `--bind`, including action triggers such as `act-up:last`.
    Bind(String) => "bind" => arg.map(Bind),
    /// Cancel current operation
    Cancel => "cancel" => Some(Cancel),
    /// Clear the screen
    ClearScreen => "clear-screen" => Some(ClearScreen),
    /// Delete character under cursor
    DeleteChar => "delete-char" => Some(DeleteChar),
    /// Delete character or exit if empty
    DeleteCharEof => "delete-char/eof" => Some(DeleteCharEof),
    /// Deselect all items
    DeselectAll => "deselect-all" => Some(DeselectAll),
    /// Move selection down by N items
    Down(u16) => "down" => Some(Down(arg.and_then(|s| s.parse().ok()).unwrap_or(1))),
    /// Move cursor to end of line
    EndOfLine => "end-of-line" => Some(EndOfLine),
    /// Execute a command.
    ///
    /// The argument is a command, see COMMAND EXPANSION for details.
    Execute(String) => "execute" => arg.map(Execute),
    /// Execute a command silently.
    ///
    /// The argument is a command, see COMMAND EXPANSION for details.
    ExecuteSilent(String) => "execute-silent" => arg.map(ExecuteSilent),
    /// Jump to first item in list
    First => "first" => Some(First),
    /// Move cursor forward one character
    ForwardChar => "forward-char" => Some(ForwardChar),
    /// Move cursor forward one word
    ForwardWord => "forward-word" => Some(ForwardWord),
    /// Execute action if query is empty
    IfQueryEmpty(String, Option<String>) => "if-query-empty" => parse_conditional(arg, IfQueryEmpty),
    /// Execute action if query is not empty
    IfQueryNotEmpty(String, Option<String>) => "if-query-not-empty" => parse_conditional(arg, IfQueryNotEmpty),
    /// Execute action if no items match
    IfNonMatched(String, Option<String>) => "if-non-matched" => parse_conditional(arg, IfNonMatched),
    /// Ignore the action
    Ignore => "ignore" => Some(Ignore),
    /// Delete from cursor to end of line
    KillLine => "kill-line" => Some(KillLine),
    /// Delete word after cursor
    KillWord => "kill-word" => Some(KillWord),
    /// Jump to last item in list
    Last => "last" => Some(Last),
    /// Move to next history entry (requires `--history` or `--cmd-history`)
    NextHistory => "next-history" => Some(NextHistory),
    /// Scroll down by half a page
    HalfPageDown(i32) => "half-page-down" => Some(HalfPageDown(arg.and_then(|s| s.parse().ok()).unwrap_or(1))),
    /// Scroll up by half a page
    HalfPageUp(i32) => "half-page-up" => Some(HalfPageUp(arg.and_then(|s| s.parse().ok()).unwrap_or(1))),
    /// Scroll down by a page
    PageDown(i32) => "page-down" => Some(PageDown(arg.and_then(|s| s.parse().ok()).unwrap_or(1))),
    /// Scroll up by a page
    PageUp(i32) => "page-up" => Some(PageUp(arg.and_then(|s| s.parse().ok()).unwrap_or(1))),
    /// Scroll preview up
    PreviewUp(i32) => "preview-up" => Some(PreviewUp(arg.and_then(|s| s.parse().ok()).unwrap_or(1))),
    /// Scroll preview down
    PreviewDown(i32) => "preview-down" => Some(PreviewDown(arg.and_then(|s| s.parse().ok()).unwrap_or(1))),
    /// Scroll preview left
    PreviewLeft(i32) => "preview-left" => Some(PreviewLeft(arg.and_then(|s| s.parse().ok()).unwrap_or(1))),
    /// Scroll preview right
    PreviewRight(i32) => "preview-right" => Some(PreviewRight(arg.and_then(|s| s.parse().ok()).unwrap_or(1))),
    /// Scroll preview up by a page
    PreviewPageUp(i32) => "preview-page-up" => Some(PreviewPageUp(arg.and_then(|s| s.parse().ok()).unwrap_or(1))),
    /// Scroll preview down by a page
    PreviewPageDown(i32) => "preview-page-down" => Some(PreviewPageDown(arg.and_then(|s| s.parse().ok()).unwrap_or(1))),
    /// Move to previous history entry (requires `--history` or `--cmd-history`)
    PreviousHistory => "previous-history" => Some(PreviousHistory),
    /// Redraw the screen
    Redraw => "redraw" => Some(Redraw),
    /// Refresh the command
    RefreshCmd => "refresh-cmd" => Some(RefreshCmd),
    /// Refresh the preview
    RefreshPreview => "refresh-preview" => Some(RefreshPreview),
    /// Restart the matcher
    RestartMatcher => "restart-matcher" => Some(RestartMatcher),
    /// Reload with optional new command
    Reload(Option<String>) => "reload" => Some(Reload(arg)),
    /// Rotate through matching modes
    RotateMode => "rotate-mode" => Some(RotateMode),
    /// Scroll item list left
    ScrollLeft(i32) => "scroll-left" => Some(ScrollLeft(arg.and_then(|s| s.parse().ok()).unwrap_or(1))),
    /// Scroll item list right
    ScrollRight(i32) => "scroll-right" => Some(ScrollRight(arg.and_then(|s| s.parse().ok()).unwrap_or(1))),
    /// Select all items
    SelectAll => "select-all" => Some(SelectAll),
    /// Select a specific row
    SelectRow(usize) => "select-row" => Some(SelectRow(arg.and_then(|s| s.parse().ok()).unwrap_or_default())),
    /// Select current item
    Select => "select" => Some(Select),
    /// Suppress the default behaviour of the action this is bound to.
    ///
    /// Only meaningful as a follow-up bound to an action (e.g. `act-up:suppress`):
    /// it cancels that action's own effect, so the remaining follow-up chain
    /// runs in its place. On its own it is a no-op (equivalent to `ignore`).
    Suppress => "suppress" => Some(Suppress),
    /// Set the interactive-mode command and rerun it.
    ///
    /// The argument is an expanded expression, see COMMAND EXPANSION for details.
    SetCmd(String) => "set-cmd" => arg.map(SetCmd),
    /// Set the header (or disable it on an empty value)
    SetHeader(Option<String>) => "set-header" => Some(SetHeader(arg)),
    /// Set the preview cmd and rerun preview.
    ///
    /// The argument is an expanded expression, see COMMAND EXPANSION for details.
    SetPreviewCmd(String) => "set-preview-cmd" => arg.map(SetPreviewCmd),
    /// Set the query to the expanded value.
    ///
    /// The argument is an expanded expression, see COMMAND EXPANSION for details.
    SetQuery(String) => "set-query" => arg.map(SetQuery),
    /// Toggle selection of current item
    Toggle => "toggle" => Some(Toggle),
    /// Toggle selection of all items
    ToggleAll => "toggle-all" => Some(ToggleAll),
    /// Toggle and move in
    ToggleIn => "toggle-in" => Some(ToggleIn),
    /// Toggle interactive mode
    ToggleInteractive => "toggle-interactive" => Some(ToggleInteractive),
    /// Toggle and move out
    ToggleOut => "toggle-out" => Some(ToggleOut),
    /// Toggle preview visibility
    TogglePreview => "toggle-preview" => Some(TogglePreview),
    /// Toggle preview line wrapping
    TogglePreviewWrap => "toggle-preview-wrap" => Some(TogglePreviewWrap),
    /// Toggle sorting
    ToggleSort => "toggle-sort" => Some(ToggleSort),
    /// Jump to first item in list (alias for First)
    Top => "top" => Some(Top),
    /// Unbind one or more keys.
    ///
    /// The argument is a comma-separated list of keys or action triggers (e.g. `act-up`) to unbind.
    Unbind(String) => "unbind" => arg.map(Unbind),
    /// Discard line (unix-style)
    UnixLineDiscard => "unix-line-discard" => Some(UnixLineDiscard),
    /// Delete word backward (unix-style)
    UnixWordRubout => "unix-word-rubout" => Some(UnixWordRubout),
    /// Move selection up by N items
    Up(u16) => "up" => Some(Up(arg.and_then(|s| s.parse().ok()).unwrap_or(1))),
    /// Yank (paste)
    Yank => "yank" => Some(Yank),
    /// Custom action from lib
    @attrs[
        debug("custom"),
        eq(skip),
        partial_eq(skip),
        cfg_attr(feature = "listen", serde(skip)),
    ]
    Custom(ActionCallback) => "custom" => None,
}

/// Parses an action string into an Action enum
///
/// Returns `None` if the action is unrecognized, or if it is specified without
/// an argument it requires (the `if-*` actions, `execute`, `set-query`, … — see
/// the `arg.map(…)` arms of the catalog).
#[must_use]
pub fn parse_action(raw_action: &str) -> Option<Action> {
    let parts = raw_action.split_once([':', '(', ')']);
    let action;
    let mut arg = None;
    match parts {
        None => action = raw_action,
        Some((act, "")) => action = act,
        Some((act, a)) => {
            action = act;
            arg = Some(a.trim_end_matches(')').to_string());
        }
    }
    debug!("parse_action: action={action}, arg={arg:?}");

    parse_named_action(action, arg)
}

#[cfg(test)]
#[path = "actions_tests.rs"]
mod tests;
