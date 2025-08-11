use crate::field::get_string_by_field;
use crate::field::FieldRange;
use crate::SkimItem;
use regex::Regex;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::prelude::v1::*;
use std::sync::Arc;

pub fn read_file_lines(filename: &str) -> std::result::Result<Vec<String>, std::io::Error> {
    let file = File::open(filename)?;
    let ret = BufReader::new(file).lines().collect();
    debug!("file content: {:?}", ret);
    ret
}

lazy_static! {
    static ref RE_ITEMS: Regex = Regex::new(r"\\?(\{ *-?[0-9.+]*? *})").unwrap();
    static ref RE_FIELDS: Regex = Regex::new(r"\\?(\{ *-?[0-9.,cq+n]*? *})").unwrap();
}

/// Check if a command depends on item
/// e.g. contains `{}`, `{1..}`, `{+}`
pub fn depends_on_items(cmd: &str) -> bool {
    RE_ITEMS.is_match(cmd)
}

/// Replace the fields in `pattern` with the items, expanding {...} patterns
///
/// Replaces:
/// - `{}` -> currently selected item
/// - `{1..}` etc -> fields of currently selected item, whose index is `selected`
/// - `{+}` -> all items
/// - `{q}` -> current query
/// - `{cq}` -> current command query
///
pub fn printf<'a>(
    pattern: String,
    delimiter: &Regex,
    items: impl Iterator<Item = Arc<dyn SkimItem>>,
    selected: Option<Arc<dyn SkimItem>>,
    query: &str,
    command_query: &str,
) -> String {
    let item_text = match selected {
        Some(s) => s.text().into_owned(),
        None => String::default(),
    };
    // Replace static fields first
    let mut res = pattern.replace("{}", &item_text);

    res = res.replace(
        "{+}",
        &items.map(|i| i.text().into_owned()).collect::<Vec<_>>().join(" "),
    );
    res = res.replace("{q}", query);
    res = res.replace("{cq}", command_query);

    let mut inside = false;
    let mut pattern = String::new();
    let mut replaced = String::new();
    for c in res.chars() {
        if inside {
            if c == '}' {
                let range = FieldRange::from_str(&pattern).unwrap(); // TODO
                let replacement = get_string_by_field(delimiter, &item_text, &range).unwrap();
                replaced.push_str(replacement);

                pattern = String::new();
                inside = false;
            } else {
                pattern.push(c);
            }
        } else if c == '{' {
            inside = true;
        } else {
            replaced.push(c);
        }
    }

    replaced
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::SkimItem;
    use regex::Regex;
    #[test]
    fn test_printf() {
        let pattern = String::from("[1] {} [2] {..2} [3] {2..} [4] {+} [5] {q} [6] {cq}");
        let items: Vec<Arc<dyn SkimItem>> = vec![
            Arc::new("item 1"),
            Arc::new("item 2"),
            Arc::new("item 3"),
            Arc::new("item 4"),
        ];
        let delimiter = Regex::new(" ").unwrap();
        assert_eq!(
            printf(
                pattern,
                &delimiter,
                items.iter().map(|x| x.clone()),
                Some(Arc::new("item 2")),
                "query",
                "cmd query"
            ),
            String::from("[1] item 2 [2] item 2 [3] 2 [4] item 1\nitem 2\nitem 3\nitem 4 [5] query [6] cmd query")
        );
    }
}
