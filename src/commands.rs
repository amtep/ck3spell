use druid::{Rect, Selector};
use std::rc::Rc;

use crate::Cursor;

pub const QUERY_LINE_LAYOUT_REGION: Selector<usize> =
    Selector::new("query_line_layout_region");
pub const REPLY_LINE_LAYOUT_REGION: Selector<Rect> =
    Selector::new("reply_line_layout_region");

pub const HIGHLIGHT_WORD: Selector<Cursor> = Selector::new("highlight_word");

pub const DICTIONARY_UPDATED: Selector<()> =
    Selector::new("dictionary_updated");

pub const APPLY_SUGGESTION: Selector<Rc<String>> =
    Selector::new("apply_suggestion");

pub const APPLY_EDIT: Selector<()> = Selector::new("apply_edit");
