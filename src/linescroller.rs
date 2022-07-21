use druid::widget::prelude::*;
use druid::widget::Scroll;
use druid::{Command, Point, Target, WidgetPod};

use crate::commands::{QUERY_LINE_LAYOUT_REGION, REPLY_LINE_LAYOUT_REGION};
use crate::{AppState, Cursor};

pub struct LineScroller<W> {
    scroll: WidgetPod<AppState, Scroll<AppState, W>>,
    old_cursor: Cursor,
}

impl<W: Widget<AppState>> LineScroller<W> {
    pub fn new(child: W) -> LineScroller<W> {
        LineScroller {
            scroll: WidgetPod::new(Scroll::new(child).vertical()),
            old_cursor: Cursor {
                linenr: 1,
                wordnr: 0,
            },
        }
    }
}

impl<W: Widget<AppState>> Widget<AppState> for LineScroller<W> {
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        event: &Event,
        data: &mut AppState,
        env: &Env,
    ) {
        self.scroll.event(ctx, event, data, env);
        if let Event::Notification(notification) = event {
            if let Some(&rect) = notification.get(REPLY_LINE_LAYOUT_REGION) {
                self.scroll.widget_mut().scroll_to(rect);
                ctx.set_handled();
                ctx.request_paint();
            }
        }
    }

    fn lifecycle(
        &mut self,
        ctx: &mut LifeCycleCtx,
        event: &LifeCycle,
        data: &AppState,
        env: &Env,
    ) {
        self.scroll.lifecycle(ctx, event, data, env);
    }

    fn update(
        &mut self,
        ctx: &mut UpdateCtx,
        _old_data: &AppState,
        data: &AppState,
        env: &Env,
    ) {
        self.scroll.update(ctx, data, env);
        if !data.cursor.same(&self.old_cursor) {
            self.old_cursor = data.cursor.clone();
            let command = Command::new(
                QUERY_LINE_LAYOUT_REGION,
                data.cursor.linenr,
                Target::Auto,
            );
            ctx.submit_command(command);
        }
    }

    fn layout(
        &mut self,
        ctx: &mut LayoutCtx,
        bc: &BoxConstraints,
        data: &AppState,
        env: &Env,
    ) -> Size {
        bc.debug_check("LineScroller");
        let size = self.scroll.layout(ctx, bc, data, env);
        self.scroll.set_origin(ctx, data, env, Point::ZERO);
        size
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &AppState, env: &Env) {
        self.scroll.paint(ctx, data, env);
    }
}
