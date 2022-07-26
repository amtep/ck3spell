use druid::widget::prelude::*;
use druid::{Point, WidgetPod};
use std::rc::Rc;

use crate::commands::{CURSOR_CHANGED, DICTIONARY_UPDATED};
use crate::LineInfo;

pub struct SyntaxHighlighter<W> {
    child: WidgetPod<LineInfo, W>,
    old_line: Option<Rc<String>>,
}

impl<W: Widget<LineInfo>> SyntaxHighlighter<W> {
    pub fn new(child: W) -> SyntaxHighlighter<W> {
        SyntaxHighlighter {
            child: WidgetPod::new(child),
            old_line: None,
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
            if let Some(cursor) = command.get(CURSOR_CHANGED) {
                if data.line.line_nr == cursor.linenr {
                    data.highlight_word_nr = cursor.wordnr;
                    force_update = true;
                } else {
                    force_update = data.highlight_word_nr != 0;
                    data.highlight_word_nr = 0;
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
