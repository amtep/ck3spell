use druid::{Rect, Selector};
use std::rc::Rc;

use crate::Cursor;

pub const QUERY_LINE_LAYOUT_REGION: Selector<usize> = Selector::new("query_line_layout_region");
pub const REPLY_LINE_LAYOUT_REGION: Selector<Rect> = Selector::new("reply_line_layout_region");

pub const CURSOR_CHANGED: Selector<Cursor> = Selector::new("cursor_changed");

pub const DICTIONARY_UPDATED: Selector = Selector::new("dictionary_updated");

pub const FILE_CHANGED: Selector = Selector::new("file_changed");

pub const APPLY_SUGGESTION: Selector<Rc<String>> = Selector::new("apply_suggestion");

pub const APPLY_EDIT: Selector = Selector::new("apply_edit");

pub const GOTO_LINE: Selector<usize> = Selector::new("goto_line");

// Hotkeys for buttons
pub const ACCEPT_WORD: Selector = Selector::new("accept_word");
pub const CURSOR_NEXT: Selector = Selector::new("cursor_next");
pub const CURSOR_PREV: Selector = Selector::new("cursor_prev");
pub const EDIT_LINE: Selector = Selector::new("edit_line");
pub const SAVE_AND_CLOSE: Selector = Selector::new("save_and_close");

// Non-hotkey buttons
pub const CLOSE_GOOD_FILES: Selector = Selector::new("close_good_files");

pub const EDIT_TEXT_AT: Selector<usize> = Selector::new("edit_text_at");
