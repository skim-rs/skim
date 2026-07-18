//! Key binding configuration and parsing.
//!
//! This module provides utilities for parsing and managing keyboard shortcuts
//! and their associated actions in skim.

use std::collections::HashMap;
use std::ops::{Deref, DerefMut};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use eyre::{Result, eyre};

use crate::tui::event::{self, Action};

/// Synthetic events that skim fires internally and that can be bound to actions
/// via the keymap, exactly like a real key press.
///
/// The keymap is keyed by crossterm's [`KeyEvent`], which cannot express
/// "the query changed" or "reading finished" directly. Each variant is
/// therefore represented *transparently* as a reserved function-key code in the
/// high-`F` range (`F(249)`–`F(255)`) that no real terminal ever emits. The
/// seven variants are `change`, `start`, `load`, `result`, `focus`, `zero`, and
/// `one`. Giving these reserved codes named variants keeps them in one place
/// instead of scattering magic function-key literals across the codebase, and
/// lets [`parse_key`] accept every friendly event name.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum SkimEvent {
    /// Fired once, when skim has started up and entered its event loop.
    Start,
    /// Fired when the reader finishes producing items (once per read; a
    /// `reload` starts a new read and fires it again).
    Load,
    /// Fired whenever the query changes.
    Change,
    /// Fired when filtering for the current query completes and the result
    /// list is ready.
    Result,
    /// Fired when the focused item changes (cursor movement or a result update).
    Focus,
    /// Fired when a completed search yields no matches.
    Zero,
    /// Fired when a completed search yields exactly one match.
    One,
}

impl SkimEvent {
    /// The reserved [`KeyCode`] used to route this event through the keymap.
    #[must_use]
    pub const fn key_code(self) -> KeyCode {
        match self {
            SkimEvent::Change => KeyCode::F(255),
            SkimEvent::Start => KeyCode::F(254),
            SkimEvent::Load => KeyCode::F(253),
            SkimEvent::Result => KeyCode::F(252),
            SkimEvent::Focus => KeyCode::F(251),
            SkimEvent::Zero => KeyCode::F(250),
            SkimEvent::One => KeyCode::F(249),
        }
    }

    /// The reserved [`KeyEvent`] used to route this event through the keymap.
    #[must_use]
    pub const fn key_event(self) -> KeyEvent {
        KeyEvent::new(self.key_code(), KeyModifiers::NONE)
    }

    /// Parses an event name (`start`, `load`, `change`) into a [`SkimEvent`].
    ///
    /// Returns `None` if the name is not a recognised event.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "start" => Some(SkimEvent::Start),
            "load" => Some(SkimEvent::Load),
            "change" => Some(SkimEvent::Change),
            "result" => Some(SkimEvent::Result),
            "focus" => Some(SkimEvent::Focus),
            "zero" => Some(SkimEvent::Zero),
            "one" => Some(SkimEvent::One),
            _ => None,
        }
    }
}

impl From<SkimEvent> for KeyEvent {
    fn from(event: SkimEvent) -> Self {
        event.key_event()
    }
}

/// A map of key events to their associated actions
#[derive(Clone, Debug)]
pub struct KeyMap(pub HashMap<KeyEvent, Vec<Action>>);

impl Deref for KeyMap {
    type Target = HashMap<KeyEvent, Vec<Action>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for KeyMap {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<&str> for KeyMap {
    fn from(value: &str) -> Self {
        parse_keymaps(split_top_level(value, ',').into_iter())
    }
}

impl Default for KeyMap {
    fn default() -> Self {
        get_default_key_map()
    }
}

impl KeyMap {
    /// Adds keymaps from a comma-separated string.
    pub(crate) fn add_keymaps_str(&mut self, source: &str) {
        self.add_keymaps(split_top_level(source, ',').into_iter());
    }

    /// Adds keymaps from the source, parsing them using `parse_keymap`
    pub fn add_keymaps<'a, T>(&mut self, source: T)
    where
        T: Iterator<Item = &'a str>,
    {
        for map in source {
            if let Ok((key, action_chain)) = parse_keymap(map) {
                self.bind(key, action_chain)
                    .unwrap_or_else(|err| debug!("Failed to bind key {map}: {err}"));
            } else {
                debug!("Failed to parse key: {map}");
            }
        }
    }
    fn bind(&mut self, key: &str, action_chain: Vec<Action>) -> Result<()> {
        let key = parse_key(key)?;

        // remove the key for existing keymap;
        let _ = self.remove(&key);
        self.entry(key).or_insert(action_chain);
        Ok(())
    }
}

/// Returns the default key bindings for skim
#[rustfmt::skip]
#[must_use]
pub fn get_default_key_map() -> KeyMap {
    let mut ret = HashMap::new();

    ret.insert(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), vec![Action::Down(1)]);
    ret.insert(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE), vec![Action::Up(1)]);
    ret.insert(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE), vec![Action::PageUp(1)]);
    ret.insert(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE), vec![Action::PageDown(1)]);
    ret.insert(KeyEvent::new(KeyCode::End, KeyModifiers::NONE), vec![Action::EndOfLine]);
    ret.insert(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE), vec![Action::BeginningOfLine]);
    ret.insert(KeyEvent::new(KeyCode::Delete, KeyModifiers::NONE), vec![Action::DeleteChar]);
    ret.insert(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE), vec![Action::Toggle, Action::Down(1)]);
    ret.insert(KeyEvent::new(KeyCode::BackTab, KeyModifiers::all()), vec![Action::Toggle, Action::Up(1)]);
    ret.insert(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), vec![Action::Abort]);
    ret.insert(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), vec![Action::Accept(None)]);
    ret.insert(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE), vec![Action::BackwardChar]);
    ret.insert(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), vec![Action::ForwardChar]);
    ret.insert(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE), vec![Action::BackwardDeleteChar]);


    ret.insert(KeyEvent::new(KeyCode::Left, KeyModifiers::SHIFT), vec![Action::BackwardWord]);
    ret.insert(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT), vec![Action::ForwardWord]);
    ret.insert(KeyEvent::new(KeyCode::Up, KeyModifiers::SHIFT), vec![Action::PreviewUp(1)]);
    ret.insert(KeyEvent::new(KeyCode::Down, KeyModifiers::SHIFT), vec![Action::PreviewDown(1)]);
    ret.insert(KeyEvent::new(KeyCode::Tab, KeyModifiers::SHIFT), vec![Action::Toggle, Action::Up(1)]);
    ret.insert(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT), vec![Action::Toggle, Action::Up(1)]);
    ret.insert(KeyEvent::new(KeyCode::Home, KeyModifiers::SHIFT), vec![Action::BeginningOfLine]);


    ret.insert(KeyEvent::new(KeyCode::Left, KeyModifiers::CONTROL), vec![Action::BackwardWord]);
    ret.insert(KeyEvent::new(KeyCode::Right, KeyModifiers::CONTROL), vec![Action::ForwardWord]);

    ret.insert(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL), vec![Action::BeginningOfLine]);
    ret.insert(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::CONTROL), vec![Action::BackwardChar]);
    ret.insert(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL), vec![Action::Abort]);
    ret.insert(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL), vec![Action::Abort]);
    ret.insert(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL), vec![Action::EndOfLine]);
    ret.insert(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL), vec![Action::ForwardChar]);
    ret.insert(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL), vec![Action::Abort]);
    ret.insert(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::CONTROL), vec![Action::BackwardDeleteChar]);
    ret.insert(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::CONTROL), vec![Action::Down(1)]);
    ret.insert(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL), vec![Action::Up(1)]);
    ret.insert(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL), vec![Action::ClearScreen]);
    ret.insert(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL), vec![Action::Down(1)]);
    ret.insert(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL), vec![Action::Up(1)]);
    ret.insert(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL), vec![Action::ToggleInteractive]);
    ret.insert(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL), vec![Action::RotateMode]);
    ret.insert(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL), vec![Action::UnixLineDiscard]);
    ret.insert(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL), vec![Action::UnixWordRubout]);
    ret.insert(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::CONTROL), vec![Action::Yank]);


    ret.insert(KeyEvent::new(KeyCode::Backspace, KeyModifiers::ALT), vec![Action::BackwardKillWord]);

    ret.insert(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::ALT), vec![Action::BackwardWord]);
    ret.insert(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::ALT), vec![Action::KillWord]);
    ret.insert(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::ALT), vec![Action::ForwardWord]);
    ret.insert(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::ALT), vec![Action::ScrollLeft(1)]);
    ret.insert(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::ALT), vec![Action::ScrollRight(1)]);

    KeyMap(ret)
}

/// Parses a key str into a crossterm `KeyEvent`.
///
/// In addition to keyboard names, accepts all seven names recognized by
/// [`SkimEvent::from_name`]: `change`, `start`, `load`, `result`, `focus`,
/// `zero`, and `one`.
///
/// # Errors
/// Returns an error if the key string is empty, contains an unknown modifier,
/// or does not correspond to a recognised key name.
pub fn parse_key(key: &str) -> Result<KeyEvent> {
    if key.is_empty() {
        return Err(eyre!("Cannot parse empty key"));
    }
    let parts = key.split('-').collect::<Vec<&str>>();
    let mut mods = KeyModifiers::NONE;

    if parts.len() > 1 {
        let mod_strs = &parts[..parts.len() - 1];
        for mod_str in mod_strs {
            mods |= match *mod_str {
                "ctrl" => KeyModifiers::CONTROL,
                "alt" => KeyModifiers::ALT,
                "shift" => KeyModifiers::SHIFT,
                s => return Err(eyre!("Failed to parse {} as key modifier", s)),
            }
        }
    }
    let key = parts.last().unwrap_or(&"").to_string();

    let keycode: KeyCode;
    if key.len() == 1 {
        let char = key.chars().next().unwrap_or_default();
        if char.is_uppercase() {
            mods |= KeyModifiers::SHIFT;
            keycode = KeyCode::Char(char.to_ascii_lowercase());
        } else {
            keycode = KeyCode::Char(char);
        }
    } else if let Some(f) = key.strip_prefix('f')
        && let Ok(f_index) = f.parse::<u8>()
    {
        // A function key like `f10`. If the suffix isn't numeric (e.g. `focus`,
        // `first`), fall through to the named-key / event matching below.
        keycode = KeyCode::F(f_index);
    } else {
        keycode = match key.as_str() {
            "space" => KeyCode::Char(' '),
            "enter" => KeyCode::Enter,
            "bspace" | "bs" => KeyCode::Backspace,
            "up" => KeyCode::Up,
            "down" => KeyCode::Down,
            "left" => KeyCode::Left,
            "right" => KeyCode::Right,
            "tab" => KeyCode::Tab,
            "btab" => KeyCode::BackTab,
            "esc" => KeyCode::Esc,
            "home" => KeyCode::Home,
            "end" => KeyCode::End,
            "pgup" => KeyCode::PageUp,
            "pgdown" => KeyCode::PageDown,
            s => match SkimEvent::from_name(s) {
                Some(event) => event.key_code(),
                None => return Err(eyre!("Unknown key {}", s)),
            },
        }
    }

    debug!("parsed key {keycode:?} and mods {mods:?}");

    Ok(KeyEvent::new(keycode, mods))
}

/// Parse an iterator of keymaps into a `KeyMap`
pub fn parse_keymaps<'a, T>(maps: T) -> KeyMap
where
    T: Iterator<Item = &'a str>,
{
    let mut res = KeyMap::default();
    res.add_keymaps(maps);
    res
}

fn split_top_level(value: &str, separator: char) -> Vec<&str> {
    let mut depth = 0_u32;
    let mut start = 0;
    let mut parts = Vec::new();

    for (index, ch) in value.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ if ch == separator && depth == 0 => {
                parts.push(&value[start..index]);
                start = index + ch.len_utf8();
            }
            _ => {}
        }
    }
    parts.push(&value[start..]);
    parts
}

/// Parses follow-up action bindings from raw `--bind` specs.
///
/// Any action can be bound as if it were an event: when the "key" of a bind is
/// not a real key but is a known action name, the bound chain becomes a
/// *follow-up* that runs right after that action. For example `reload:first`
/// queues `first` immediately after a `reload`. The returned map is keyed by the
/// action's canonical name (see [`Action::name`](crate::tui::event::Action::name)),
/// so it can be looked up directly from the action that just ran.
///
/// Keys take precedence: if the "key" resolves to a real key it is left to the
/// key map, so a name shared by a key and an action (e.g. `up`) always binds the
/// key. To target the action in that case, prefix it with `act-` (`act-up`).
#[must_use]
pub fn parse_action_binds<'a, T>(maps: T) -> HashMap<String, Vec<Action>>
where
    T: Iterator<Item = &'a str>,
{
    let mut res = HashMap::new();
    for map in maps {
        let Some((key, chain)) = map.split_once(':') else {
            continue;
        };
        // Keys win: anything that parses as a real key is not an action trigger.
        if parse_key(key).is_ok() {
            continue;
        }
        // `act-<name>` explicitly targets the action `<name>`, even when `<name>`
        // is also a key. Without the prefix, a bare action name still works as
        // long as it isn't a key.
        let action_name = key.strip_prefix("act-").unwrap_or(key);
        // Some actions require an argument when executed, but their canonical
        // name is still valid as a trigger. `()` supplies the parser's empty
        // placeholder solely for name validation.
        let action = event::parse_action(action_name).or_else(|| event::parse_action(&format!("{action_name}()")));
        if let Some(action) = action
            && let Ok(actions) = parse_action_chain(chain)
        {
            res.insert(action.name().to_string(), actions);
        }
    }
    res
}

/// Parses an action chain, separated by '+'s into the corresponding actions
///
/// # Errors
/// Returns an error if the action chain is empty or contains only unknown actions.
pub fn parse_action_chain(action_chain: &str) -> Result<Vec<Action>> {
    let mut actions: Vec<Action> = vec![];
    let mut split = split_top_level(action_chain, '+').into_iter();

    while let Some(mut s) = split.next().map(String::from) {
        if (s.starts_with("if-") || s.ends_with('{'))
            && let Some(otherwise) = split.next()
        {
            s += &(String::from("+") + otherwise);
        }
        if let Some(act) = event::parse_action(&s) {
            actions.push(act);
        }
    }
    if actions.is_empty() {
        Err(eyre!("Empty action chain or unknown action `{}`", action_chain))
    } else {
        Ok(actions)
    }
}

/// Parse a single keymap and return the key and action(s)
///
/// # Errors
/// Returns an error if the string is empty, missing a colon separator, or the
/// action chain cannot be parsed.
pub fn parse_keymap(key_action: &str) -> Result<(&str, Vec<Action>)> {
    if key_action.is_empty() {
        return Err(eyre!("Got an empty keybind, skipping"));
    }
    debug!("got key_action: {key_action:?}");
    let (key, action_chain) = key_action
        .split_once(':')
        .ok_or(eyre!("Failed to parse {} as key and action", key_action))?;
    debug!("parsed key_action: {key:?}: {action_chain:?}");
    Ok((key, parse_action_chain(action_chain)?))
}

#[cfg(test)]
#[path = "binds_tests.rs"]
mod tests;
