use druid::widget::prelude::*;
use druid::widget::Controller;
use druid::{Command, KbKey, Target};

use crate::commands::{APPLY_EDIT, EDIT_LINE, EDIT_TEXT_AT};
use crate::AppState;

pub struct EditorController;

impl<W: Widget<AppState>> Controller<AppState, W> for EditorController {
    fn event(
        &mut self,
        child: &mut W,
        ctx: &mut EventCtx,
        event: &Event,
        data: &mut AppState,
        env: &Env,
    ) {
        if data.editing_linenr > 0 && !ctx.has_focus() {
            ctx.request_focus();
        }
        if data.editing_linenr == 0 && ctx.has_focus() {
            ctx.resign_focus();
        }
        if let Event::Command(command) = event {
            if command.is(EDIT_LINE) {
                if let Some(range) = data.file.lines[data.editing_linenr - 1].marked_word() {
                    ctx.submit_command(Command::new(EDIT_TEXT_AT, range.start, Target::Auto));
                }
            }
        } else if let Event::KeyDown(key_event) = event {
            if KbKey::Enter == key_event.key {
                ctx.submit_command(APPLY_EDIT);
                return; // Do not pass the ENTER down to the textbox
            }
        }
        child.event(ctx, event, data, env);
    }
}
