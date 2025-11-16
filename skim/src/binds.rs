use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
};

use color_eyre::Result;
use color_eyre::eyre::eyre;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::tui::event::{self, Action};

#[derive(Clone)]
pub struct KeyMap(HashMap<KeyEvent, Vec<Action>>);

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
        parse_keymaps(value.split(',')).expect("Failed to parse keymaps")
    }
}

impl Default for KeyMap {
    fn default() -> Self {
        get_default_key_map()
    }
}

#[rustfmt::skip]
pub fn get_default_key_map() -> KeyMap {
    let mut ret = HashMap::new();

    ret.insert(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), vec![Action::Down(1)]);
    ret.insert(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE), vec![Action::Up(1)]);
    ret.insert(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE), vec![Action::PageUp(1)]);
    ret.insert(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE), vec![Action::PageUp(1)]);
    ret.insert(KeyEvent::new(KeyCode::End, KeyModifiers::NONE), vec![Action::EndOfLine]);
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
    ret.insert(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL), vec![Action::DeleteCharEOF]);
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
    let key = parts.last().unwrap_or(&"").to_string();

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
            s => return Err(eyre!("Unknown key {}", s)),
        }
    }

    Ok(KeyEvent::new(keycode, mods))
}

/// Parses the key and creates the binding in the KeyMap
pub fn bind(keymap: &mut KeyMap, key: &str, action_chain: Vec<Action>) -> Result<()> {
    let key = parse_key(key)?;

    // remove the key for existing keymap;
    let _ = keymap.remove(&key);
    keymap.entry(key).or_insert(action_chain);
    Ok(())
}

/// Parse an iterator of keymaps into a KeyMap
pub fn parse_keymaps<'a, T>(maps: T) -> Result<KeyMap>
where
    T: Iterator<Item = &'a str>,
{
    let mut keymap = get_default_key_map();
    for map in maps {
        if !map.is_empty() {
            let (key, action_chain) = parse_keymap(map)?;
            bind(&mut keymap, key, action_chain)?;
        }
    }
    Ok(keymap)
}

/// Parses an action chain, separated by '+'s into the corresponding actions
pub fn parse_action_chain(action_chain: &str) -> Result<Vec<Action>> {
    let mut actions: Vec<Action> = vec![];
    let mut split = action_chain.split('+');
    loop {
        let opt_s = split.next();
        if opt_s.is_none() {
            break;
        }
        let mut s = opt_s.unwrap().to_string();
        if s.starts_with("if-")
            && let Some(otherwise) = split.next()
        {
            s += &(String::from("+") + otherwise);
        }
        if let Some(act) = event::parse_action(&s) {
            actions.push(act);
        }
    }
    Ok(actions)
}

/// Parse a single keymap and return the key and action(s)
pub fn parse_keymap(key_action: &str) -> Result<(&str, Vec<Action>)> {
    debug!("got key_action: {:?}", key_action);
    let (key, action_chain) = key_action
        .split_once(':')
        .ok_or(eyre!("Failed to parse {} as key and action", key_action))?;
    debug!("parsed key_action: {:?}: {:?}", key, action_chain);
    Ok((key, parse_action_chain(action_chain)?))
}

pub fn parse_expect_keys<'a, T>(keymap: &mut KeyMap, keys: T) -> Result<()>
where
    T: Iterator<Item = &'a str>,
{
    for key in keys {
        bind(keymap, key, vec![Action::Accept(Some(key.to_string()))])?;
    }
    Ok(())
}
