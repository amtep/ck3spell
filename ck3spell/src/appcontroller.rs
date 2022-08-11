use druid::widget::prelude::*;
use druid::widget::Controller;
use druid::{Command, KbKey, Target};
use std::rc::Rc;
use std::sync::Arc;

use crate::commands::{
    APPLY_EDIT, APPLY_SUGGESTION, CURSOR_CHANGED, GOTO_LINE,
};
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
                let wordnr = data.cursor.wordnr;
                if wordnr > 0 {
                    data.change_line(data.cursor.linenr, |lineinfo| {
                        if let Some(range) = lineinfo.bad_words.get(wordnr - 1)
                        {
                            let mut linetext = (*lineinfo.line.line).clone();
                            linetext.replace_range(range.clone(), word);
                            lineinfo.line.line = Rc::new(linetext);
                            lineinfo.highlight(env);
                        }
                    });
                    if data.cursor_word().is_none() {
                        data.cursor_next();
                    } else {
                        data.update_suggestions();
                    }
                }
            } else if command.is(APPLY_EDIT) && data.editing_linenr > 0 {
                let new_text = data.editing_text.clone();
                data.change_line(data.editing_linenr, |lineinfo| {
                    lineinfo.line.line = Rc::new(new_text.to_string());
                    lineinfo.highlight(env);
                });
                data.editing_linenr = 0;
                data.editing_text = Arc::new(String::new());
                if data.cursor_word().is_none() {
                    data.cursor_next();
                } else {
                    data.update_suggestions();
                }
            } else if let Some(&linenr) = command.get(GOTO_LINE) {
                data.cursor.linenr = linenr;
                data.cursor.wordnr = 0;
                data.update_suggestions();
                ctx.submit_command(Command::new(
                    CURSOR_CHANGED,
                    data.cursor,
                    Target::Auto,
                ));
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
