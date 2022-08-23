use anyhow::Context;
use druid::commands::QUIT_APP;
use druid::widget::prelude::*;
use druid::widget::Controller;
use druid::{Command, KbKey, Target};
use std::rc::Rc;
use std::sync::Arc;

use crate::commands::{
    ACCEPT_WORD, APPLY_EDIT, APPLY_SUGGESTION, CURSOR_CHANGED, CURSOR_NEXT, CURSOR_PREV,
    DICTIONARY_UPDATED, EDIT_LINE, FILE_CHANGED, GOTO_LINE, SAVE_AND_CLOSE,
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
                        if let Some(range) = lineinfo.bad_words_range.get(wordnr - 1) {
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
                let mut cursor = data.cursor;
                cursor.linenr = linenr;
                if !data.file.lines[cursor.linenr - 1]
                    .bad_words_range
                    .is_empty()
                {
                    cursor.wordnr = 1;
                } else {
                    cursor.wordnr = 0;
                }
                data.update_cursor(cursor);
                data.update_suggestions();
                ctx.submit_command(Command::new(CURSOR_CHANGED, data.cursor, Target::Auto));
            } else if command.is(ACCEPT_WORD) {
                if let Some(cursor_word) = data.cursor_word() {
                    if let Err(err) = data
                        .file
                        .speller
                        .borrow_mut()
                        .add_word_to_user_dict(cursor_word)
                    {
                        eprintln!("{:#}", err);
                    }
                    ctx.submit_command(DICTIONARY_UPDATED);
                }
            } else if command.is(EDIT_LINE) {
                data.editing_linenr = data.cursor.linenr;
                data.editing_text = Arc::new(
                    data.file.lines[data.cursor.linenr - 1]
                        .line
                        .line
                        .to_string(),
                );
            } else if command.is(SAVE_AND_CLOSE) {
                if let Err(err) = data.save_file().with_context(|| "Could not save file") {
                    eprintln!("{:#}", err);
                }
                if data.files.len() == 1 {
                    ctx.submit_command(QUIT_APP);
                } else {
                    data.drop_file();
                    ctx.submit_command(FILE_CHANGED);
                }
            } else if command.is(CURSOR_PREV) {
                data.cursor_prev();
                ctx.submit_command(Command::new(CURSOR_CHANGED, data.cursor, Target::Auto));
            } else if command.is(CURSOR_NEXT) {
                data.cursor_next();
                ctx.submit_command(Command::new(CURSOR_CHANGED, data.cursor, Target::Auto));
            }
        } else if let Event::KeyDown(key_event) = event {
            match &key_event.key {
                // Special: accept no hotkeys while editing a line
                _ if data.editing_linenr > 0 => (),
                KbKey::Character(a) if a == "a" => ctx.submit_command(ACCEPT_WORD),
                KbKey::Character(e) if e == "e" => ctx.submit_command(EDIT_LINE),
                KbKey::Character(c) if c == "c" => ctx.submit_command(SAVE_AND_CLOSE),
                KbKey::Character(k) => {
                    // Number keys select suggestions
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
                KbKey::ArrowUp => ctx.submit_command(CURSOR_PREV),
                KbKey::ArrowDown => ctx.submit_command(CURSOR_NEXT),
                _ => (),
            }
        }
        child.event(ctx, event, data, env);
    }
}
