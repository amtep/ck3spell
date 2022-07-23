use druid::widget::prelude::*;
use druid::widget::Controller;
use druid::{Command, KbKey, Target};
use std::rc::Rc;
use std::sync::Arc;

use crate::commands::{APPLY_EDIT, APPLY_SUGGESTION};
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
        if data.editing_linenr == 0 && !ctx.has_focus() {
            ctx.request_focus();
        }
        if let Event::Command(command) = event {
            if let Some(word) = command.get(APPLY_SUGGESTION) {
                if data.cursor.wordnr > 0 {
                    let mut lines = (*data.lines).clone();
                    if let Some(line) = lines.get_mut(data.cursor.linenr - 1) {
                        if let Some(range) =
                            line.bad_words.get(data.cursor.wordnr - 1)
                        {
                            let mut linetext = (*line.line.line).clone();
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
            } else if command.is(APPLY_EDIT) && data.editing_linenr > 0 {
                let mut lines = (*data.lines).clone();
                let mut line = &mut lines[data.editing_linenr - 1];
                line.line.line = Rc::new(data.editing_text.to_string());
                line.highlight(env);
                data.lines = Arc::new(lines);
                data.editing_linenr = 0;
                data.editing_text = Arc::new(String::new());
                if data.cursor_word().is_none() {
                    data.cursor_next();
                } else {
                    data.update_suggestions();
                }
            }
        } else if let Event::KeyDown(key_event) = event {
            if let KbKey::Character(k) = &key_event.key {
                if let Ok(d) = k.parse::<usize>() {
                    if d > 0 && d <= data.suggestions.len() {
                        let s = &data.suggestions[d - 1];
                        ctx.submit_command(Command::new(
                            APPLY_SUGGESTION,
                            s.suggestion.clone(),
                            Target::Auto,
                        ));
                    }
                }
            }
        }
        child.event(ctx, event, data, env);
    }
}
