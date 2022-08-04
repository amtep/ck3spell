use druid::widget::prelude::*;
use druid::widget::Scroll;
use druid::{Command, Point, Rect, Target, WidgetPod};

use crate::commands::{
    CURSOR_CHANGED, DICTIONARY_UPDATED, QUERY_LINE_LAYOUT_REGION,
    REPLY_LINE_LAYOUT_REGION,
};
use crate::{AppState, Cursor};

pub struct LineScroller<W> {
    scroll: WidgetPod<AppState, Scroll<AppState, W>>,
    old_cursor: Cursor,
}

impl<W: Widget<AppState>> LineScroller<W> {
    pub fn new(child: W) -> LineScroller<W> {
        LineScroller {
            scroll: WidgetPod::new(Scroll::new(child).vertical()),
            old_cursor: Cursor::default(),
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
            if let Some(&region) = notification.get(REPLY_LINE_LAYOUT_REGION) {
                let port_size = self.scroll.layout_rect().size();
                let port_pad = port_size.height / 2.0;
                let centerline = region.center().y;
                let rect = Rect::new(
                    region.x0,
                    (centerline - port_pad).max(0.0),
                    region.x1,
                    centerline + port_pad,
                );
                let moved = self.scroll.widget_mut().scroll_to(rect);
                ctx.set_handled();
                if moved {
                    ctx.request_paint();
                }
            }
        } else if let Event::Command(command) = event {
            if command.is(DICTIONARY_UPDATED) {
                if data.cursor_word().is_none() {
                    data.cursor_next();
                    ctx.submit_command(Command::new(
                        CURSOR_CHANGED,
                        data.cursor,
                        Target::Auto,
                    ));
                } else {
                    data.update_suggestions();
                }
            }
        }
        if !data.cursor.same(&self.old_cursor) {
            self.old_cursor = data.cursor;
            let command = Command::new(
                QUERY_LINE_LAYOUT_REGION,
                data.cursor.linenr,
                Target::Auto,
            );
            ctx.submit_command(command);
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
