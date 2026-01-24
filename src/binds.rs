//! Key binding configuration and parsing.
//!
//! This module provides utilities for parsing and managing keyboard shortcuts
//! and their associated actions in skim.

use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
};

use color_eyre::Result;
use color_eyre::eyre::eyre;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::tui::event::{self, Action};

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
        parse_keymaps(value.split(','))
    }
}

impl Default for KeyMap {
    fn default() -> Self {
        get_default_key_map()
    }
}

impl KeyMap {
    /// Adds keymaps from the source, parsing them using parse_keymap
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
    ret.insert(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL), vec![Action::DeleteCharEof]);
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

/// Parses a key str into a crossterm KeyEvent
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
    let key = parts.last().unwrap_or(&"").to_string().to_lowercase();

    let keycode: KeyCode;
    if key.len() == 1 {
        let char = key.chars().next().unwrap();
        if char.is_uppercase() {
            mods |= KeyModifiers::SHIFT;
            keycode = KeyCode::Char(char.to_lowercase().next().unwrap());
        } else {
            keycode = KeyCode::Char(char);
        }
    } else if key.starts_with("f") {
        let f_index = key.strip_prefix("f").unwrap().parse::<u8>()?;
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
            "pgup" | "page-up" => KeyCode::PageUp,
            "pgdown" | "page-down" => KeyCode::PageDown,
            "change" => KeyCode::F(255),
            s => return Err(eyre!("Unknown key {}", s)),
        }
    }

    Ok(KeyEvent::new(keycode, mods))
}

/// Parse an iterator of keymaps into a KeyMap
pub fn parse_keymaps<'a, T>(maps: T) -> KeyMap
where
    T: Iterator<Item = &'a str>,
{
    let mut res = KeyMap::default();
    res.add_keymaps(maps);
    res
}

/// Parses an action chain, separated by '+'s into the corresponding actions
pub fn parse_action_chain(action_chain: &str) -> Result<Vec<Action>> {
    let mut actions: Vec<Action> = vec![];
    let mut split = action_chain.split("+");
    loop {
        let Some(mut s) = split.next().map(String::from) else {
            break;
        };
        if (s.starts_with("if-") || s.ends_with("{"))
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
pub fn parse_keymap(key_action: &str) -> Result<(&str, Vec<Action>)> {
    if key_action.is_empty() {
        return Err(eyre!("Got an empty keybind, skipping"));
    }
    debug!("got key_action: {:?}", key_action);
    let (key, action_chain) = key_action
        .split_once(':')
        .ok_or(eyre!("Failed to parse {} as key and action", key_action))?;
    debug!("parsed key_action: {:?}: {:?}", key, action_chain);
    Ok((key, parse_action_chain(action_chain)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use event::Action::*;
    #[test]
    fn test_parse_action_chain() {
        let parsed = parse_action_chain(
            "execute-silent:1 {}+execute-silent:2 {+}+execute-silent:3 {+n}+reload+if-query-empty:reload+up",
        );
        assert!(parsed.is_ok());
        let res = parsed.unwrap();
        assert_eq!(
            res,
            vec![
                ExecuteSilent("1 {}".into()),
                ExecuteSilent("2 {+}".into()),
                ExecuteSilent("3 {+n}".into()),
                Reload(None),
                IfQueryEmpty("reload".into(), Some("up".into())),
            ]
        );
    }
}
