use druid::text::Selection;
use druid::widget::prelude::*;
use druid::widget::TextBox;
use std::sync::Arc;

use crate::commands::EDIT_TEXT_AT;

pub struct EditLineBox {
    textbox: TextBox<Arc<String>>,
}

impl EditLineBox {
    pub fn multiline() -> Self {
        EditLineBox {
            textbox: TextBox::multiline(),
        }
    }
}

impl Widget<Arc<String>> for EditLineBox {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event, data: &mut Arc<String>, env: &Env) {
        if let Event::Command(command) = event {
            if let Some(&position) = command.get(EDIT_TEXT_AT) {
                if position > 0 {
                    let text_component = self.textbox.text();
                    if text_component.can_write() {
                        let mut session = text_component.borrow_mut();
                        if let Some(invalidation) =
                            session.set_selection(Selection::caret(position))
                        {
                            ctx.invalidate_text_input(invalidation);
                        }
                    }
                }
            }
        }
        self.textbox.event(ctx, event, data, env);
    }

    fn lifecycle(
        &mut self,
        ctx: &mut LifeCycleCtx,
        lifecycle: &LifeCycle,
        data: &Arc<String>,
        env: &Env,
    ) {
        self.textbox.lifecycle(ctx, lifecycle, data, env);
    }

    fn update(
        &mut self,
        ctx: &mut UpdateCtx,
        old_data: &Arc<String>,
        data: &Arc<String>,
        env: &Env,
    ) {
        self.textbox.update(ctx, old_data, data, env);
    }

    fn layout(
        &mut self,
        ctx: &mut LayoutCtx,
        bc: &BoxConstraints,
        data: &Arc<String>,
        env: &Env,
    ) -> Size {
        self.textbox.layout(ctx, bc, data, env)
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &Arc<String>, env: &Env) {
        self.textbox.paint(ctx, data, env);
    }
}
