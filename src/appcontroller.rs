use druid::widget::prelude::*;
use druid::widget::Controller;
use std::rc::Rc;
use std::sync::Arc;

use crate::commands::APPLY_SUGGESTION;
use crate::AppState;

pub struct AppController;

impl<W: Widget<AppState>> Controller<AppState, W> for AppController {
    fn event(
        &mut self,
        child: &mut W,
        ctx: &mut EventCtx,
        event: &Event,
        data: &mut AppState,
        env: &Env,
    ) {
        if let Event::Command(command) = event {
            if let Some(word) = command.get(APPLY_SUGGESTION) {
                if data.cursor.wordnr > 0 {
                    let mut lines = (*data.lines).clone();
                    if let Some(line) = lines.get_mut(data.cursor.linenr - 1) {
                        if let Some(range) =
                            line.bad_words.get(data.cursor.wordnr - 1)
                        {
                            let mut linetext = (&(*(line.line.line))).clone();
                            linetext.replace_range((*range).clone(), word);
                            (*line).line.line = Rc::new(linetext);
                            (*line).highlight(env);
                            data.lines = Arc::new(lines);
                            if data.cursor_word().is_none() {
                                data.cursor_next();
                            } else {
                                data.update_suggestions();
                            }
                        }
                    }
                }
            }
        }
        child.event(ctx, event, data, env);
    }
}
