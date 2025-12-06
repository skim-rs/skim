use crate::field::{FieldRange, parse_matching_fields, parse_transform_fields};
use crate::{DisplayContext, SkimItem};
use ratatui::text::Line;
use regex::Regex;
use std::borrow::Cow;

//------------------------------------------------------------------------------
/// An item will store everything that one line input will need to be operated and displayed.
///
/// What's special about an item?
/// The simplest version of an item is a line of string, but things are getting more complex:
/// - The conversion of lower/upper case is slow in rust, because it involds unicode.
/// - We may need to interpret the ANSI codes in the text.
/// - The text can be transformed and limited while searching.
///
/// About the ANSI, we made assumption that it is linewise, that means no ANSI codes will affect
/// more than one line.
#[derive(Debug)]
pub struct DefaultSkimItem {
    /// The text that will be output when user press `enter`
    /// `Some(..)` => the original input is transformed, could not output `text` directly
    /// `None` => that it is safe to output `text` directly
    orig_text: Option<String>,

    /// The text that will be shown on screen and matched.
    text: String,

    // Option<Box<_>> to reduce memory use in normal cases where no matching ranges are specified.
    #[allow(clippy::box_collection)]
    matching_ranges: Option<Box<Vec<(usize, usize)>>>,
    /// The index, for use in matching
    index: usize,
}

impl DefaultSkimItem {
    pub fn new(
        orig_text: String,
        ansi_enabled: bool, // TODO
        trans_fields: &[FieldRange],
        matching_fields: &[FieldRange],
        delimiter: &Regex,
        index: usize,
    ) -> Self {
        let using_transform_fields = !trans_fields.is_empty();

        //        transformed | ANSI             | output
        //------------------------------------------------------
        //                    +- T -> trans+ANSI | ANSI
        //                    |                  |
        //      +- T -> trans +- F -> trans      | orig
        // orig |                                |
        //      +- F -> orig  +- T -> ANSI     ==| ANSI
        //                    |                  |
        //                    +- F -> orig       | orig

        let (orig_text, text) = if using_transform_fields {
            let transformed = parse_transform_fields(delimiter, &orig_text, trans_fields);
            (Some(orig_text), transformed)
        } else {
            // Normal case: no transformation needed, save memory by not duplicating
            (None, orig_text)
        };

        let matching_ranges = if !matching_fields.is_empty() {
            Some(Box::new(parse_matching_fields(delimiter, &text, matching_fields)))
        } else {
            None
        };

        DefaultSkimItem {
            orig_text,
            text,
            matching_ranges,
            index,
        }
    }
}

impl SkimItem for DefaultSkimItem {
    #[inline]
    fn text(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.text)
    }

    fn output(&self) -> Cow<'_, str> {
        if let Some(ref orig) = self.orig_text {
            Cow::Borrowed(orig)
        } else {
            Cow::Borrowed(&self.text)
        }
    }

    fn get_matching_ranges(&self) -> Option<&[(usize, usize)]> {
        self.matching_ranges.as_ref().map(|vec| vec as &[(usize, usize)])
    }

    fn display<'a>(&'a self, context: DisplayContext) -> Line<'a> {
        context.to_line(Cow::Borrowed(&self.text))
    }

    fn get_index(&self) -> usize {
        self.index
    }

    fn set_index(&mut self, index: usize) {
        self.index = index;
    }
}
