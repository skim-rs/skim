use crate::SkimItem;
use crate::field::FieldRange;
use crate::field::get_string_by_field;
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

fn escape_arg(a: &str) -> String {
    format!("'{}'", a.replace('\0', "\\0").replace("'", "'\\''"))
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
pub fn printf(
    pattern: String,
    delimiter: &Regex,
    items: impl Iterator<Item = Arc<dyn SkimItem>>,
    selected: Option<Arc<dyn SkimItem>>,
    query: &str,
    command_query: &str,
) -> String {
    let (item_text, field_text) = match selected {
        Some(s) => (s.text().into_owned(), s.output().into_owned()),
        None => (String::default(), String::default()),
    };
    // Replace static fields first
    let mut res = pattern.replace("{}", &escape_arg(&item_text));

    res = res.replace(
        "{+}",
        &escape_arg(&items.map(|i| i.text().into_owned()).collect::<Vec<_>>().join(" ")),
    );
    res = res.replace("{q}", &escape_arg(query));
    res = res.replace("{cq}", &escape_arg(command_query));

    let mut inside = false;
    let mut pattern = String::new();
    let mut replaced = String::new();
    for c in res.chars() {
        if inside {
            if c == '}' {
                let range = FieldRange::from_str(&pattern).unwrap(); // TODO
                let replacement = get_string_by_field(delimiter, &field_text, &range).unwrap();
                replaced.push_str(&escape_arg(replacement));

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
            String::from(
                "[1] 'item 2' [2] 'item 2' [3] '2' [4] 'item 1 item 2 item 3 item 4' [5] 'query' [6] 'cmd query'"
            )
        );
    }
}
