use druid::widget::prelude::*;
use druid::{Command, Point, Target, WidgetPod};
use std::rc::Rc;

use crate::commands::{QUERY_LINE_LAYOUT_REGION, REPLY_LINE_LAYOUT_REGION};
use crate::{highlight_syntax, LineInfo};

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
        if let Event::Command(command) = event {
            if let Some(&linenr) = command.get(QUERY_LINE_LAYOUT_REGION) {
                if linenr == data.line.line_nr {
                    let command = Command::new(
                        REPLY_LINE_LAYOUT_REGION,
                        self.child.layout_rect(),
                        Target::Auto,
                    );
                    ctx.submit_notification(command);
                }
            }
        }
        self.child.event(ctx, event, data, env);
        if self.old_line.is_none()
            || !data.line.line.same(self.old_line.as_ref().unwrap())
        {
            (data.rendered, data.bad_words) =
                highlight_syntax(&data.line.line, env, &data.hunspell);
            self.old_line = Some(data.line.line.clone());
        }
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
