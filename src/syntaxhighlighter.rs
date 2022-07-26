use druid::widget::prelude::*;
use druid::{Point, WidgetPod};
use std::ops::Range;
use std::rc::Rc;

use crate::commands::{CURSOR_CHANGED, DICTIONARY_UPDATED};
use crate::LineInfo;

pub struct SyntaxHighlighter<W> {
    child: WidgetPod<LineInfo, W>,
    old_line: Option<Rc<String>>,
    old_highlight: Option<Range<usize>>,
}

impl<W: Widget<LineInfo>> SyntaxHighlighter<W> {
    pub fn new(child: W) -> SyntaxHighlighter<W> {
        SyntaxHighlighter {
            child: WidgetPod::new(child),
            old_line: None,
            old_highlight: None,
        }
    }
}

impl<W: Widget<LineInfo>> Widget<LineInfo> for SyntaxHighlighter<W> {
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        event: &Event,
        data: &mut LineInfo,
        env: &Env,
    ) {
        let mut force_update = false;
        if let Event::Command(command) = event {
            if command.is(CURSOR_CHANGED) {
                if self.old_highlight != data.marked_word() {
                    force_update = true;
                }
            } else if command.is(DICTIONARY_UPDATED) {
                if !data.bad_words.is_empty() {
                    force_update = true;
                }
            }
        }
        if self.old_line.is_none()
            || force_update
            || !data.line.line.same(self.old_line.as_ref().unwrap())
        {
            data.highlight(env);
            self.old_line = Some(data.line.line.clone());
            self.old_highlight = data.marked_word();
            ctx.request_paint();
        }
        self.child.event(ctx, event, data, env);
    }

    fn lifecycle(
        &mut self,
        ctx: &mut LifeCycleCtx,
        event: &LifeCycle,
        data: &LineInfo,
        env: &Env,
    ) {
        self.child.lifecycle(ctx, event, data, env);
    }

    fn update(
        &mut self,
        ctx: &mut UpdateCtx,
        _old_data: &LineInfo,
        data: &LineInfo,
        env: &Env,
    ) {
        self.child.update(ctx, data, env);
    }

    fn layout(
        &mut self,
        ctx: &mut LayoutCtx,
        bc: &BoxConstraints,
        data: &LineInfo,
        env: &Env,
    ) -> Size {
        bc.debug_check("SyntaxHighlighter");
        let size = self.child.layout(ctx, bc, data, env);
        self.child.set_origin(ctx, data, env, Point::ZERO);
        size
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &LineInfo, env: &Env) {
        self.child.paint(ctx, data, env);
    }
}
