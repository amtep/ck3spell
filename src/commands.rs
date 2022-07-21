use druid::{Rect, Selector};

pub const QUERY_LINE_LAYOUT_REGION: Selector<usize> =
    Selector::new("query_line_layout_region");
pub const REPLY_LINE_LAYOUT_REGION: Selector<Rect> =
    Selector::new("reply_line_layout_region");
