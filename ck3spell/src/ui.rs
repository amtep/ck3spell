use anyhow::Context;
use druid::commands::QUIT_APP;
use druid::widget::prelude::*;
use druid::widget::{
    Button, CrossAxisAlignment, Either, Flex, Label, LineBreaking, List,
    RawLabel, Scroll, TextBox,
};
use druid::{Color, Command, Target, WidgetExt};
use std::sync::Arc;

use crate::appcontroller::AppController;
use crate::commands::{
    APPLY_SUGGESTION, CURSOR_CHANGED, DICTIONARY_UPDATED, FILE_CHANGED,
    GOTO_LINE,
};
use crate::editorcontroller::EditorController;
use crate::linelist::LineList;
use crate::linescroller::LineScroller;
use crate::syntaxhighlighter::SyntaxHighlighter;
use crate::{AppState, FileState, LineInfo, Suggestion};

fn make_file_header() -> impl Widget<AppState> {
    let prev = Button::new("Prev")
        .on_click(|ctx, data: &mut AppState, _| {
            data.file_prev();
            ctx.submit_command(FILE_CHANGED);
        })
        .disabled_if(|data: &AppState, _| data.file_idx == 0);
    let next = Button::new("Next")
        .on_click(|ctx, data: &mut AppState, _| {
            data.file_next();
            ctx.submit_command(FILE_CHANGED);
        })
        .disabled_if(|data: &AppState, _| {
            data.file_idx == data.files.len() - 1
        });
    let file_label = Label::dynamic(|data: &AppState, _| {
        format!(
            "{} ({} {})",
            data.file.filename,
            data.files.len(),
            if data.files.len() == 1 {
                "file"
            } else {
                "files"
            }
        )
    });
    Flex::row()
        .with_child(prev)
        .with_default_spacer()
        .with_child(next)
        .with_default_spacer()
        .with_flex_child(file_label, 1.0)
}

fn make_line_item() -> impl Widget<LineInfo> {
    let linenr =
        Label::dynamic(|line: &LineInfo, _| line.line.line_nr.to_string())
            .with_text_color(Color::grey8(160))
            .fix_width(30.0);
    let line = SyntaxHighlighter::new(
        RawLabel::new()
            .with_line_break_mode(LineBreaking::WordWrap)
            .lens(LineInfo::rendered)
            .on_click(|ctx, data: &mut LineInfo, _| {
                ctx.submit_command(Command::new(
                    GOTO_LINE,
                    data.line.line_nr,
                    Target::Auto,
                ));
            }),
    );
    Flex::row()
        .with_child(Flex::column().with_child(linenr))
        .with_flex_child(line, 1.0)
        .cross_axis_alignment(CrossAxisAlignment::Start)
}

fn buttons_builder() -> impl Widget<AppState> {
    let prev =
        Button::new("Previous").on_click(|ctx, data: &mut AppState, _| {
            data.cursor_prev();
            ctx.submit_command(Command::new(
                CURSOR_CHANGED,
                data.cursor,
                Target::Auto,
            ));
        });
    let next = Button::new("Next").on_click(|ctx, data: &mut AppState, _| {
        data.cursor_next();
        ctx.submit_command(Command::new(
            CURSOR_CHANGED,
            data.cursor,
            Target::Auto,
        ));
    });
    let accept = Button::new("Accept word")
        .on_click(|ctx, data: &mut AppState, _| {
            if let Some(cursor_word) = data.cursor_word() {
                if let Err(err) =
                    data.file.speller.add_word_to_user_dict(cursor_word)
                {
                    eprintln!("{:#}", err);
                }
                ctx.submit_command(DICTIONARY_UPDATED);
            }
        })
        .disabled_if(|data: &AppState, _| data.cursor_word().is_none());
    let edit =
        Button::new("Edit line").on_click(|_, data: &mut AppState, _| {
            data.editing_linenr = data.cursor.linenr;
            data.editing_text = Arc::new(
                data.file.lines[data.cursor.linenr - 1]
                    .line
                    .line
                    .to_string(),
            );
        });
    let save = Button::new("Save and Close").on_click(
        |ctx, data: &mut AppState, _| {
            if let Err(err) =
                data.save_file().with_context(|| "Could not save file")
            {
                eprintln!("{:#}", err);
            }
            if data.files.len() == 1 {
                ctx.submit_command(QUIT_APP);
            } else {
                data.drop_file();
                ctx.submit_command(FILE_CHANGED);
            }
        },
    );
    Flex::row()
        .with_child(prev)
        .with_default_spacer()
        .with_child(next)
        .with_default_spacer()
        .with_child(accept)
        .with_default_spacer()
        .with_child(edit)
        .with_default_spacer()
        .with_child(save)
}

fn make_suggestion() -> impl Widget<Suggestion> {
    let nr = Button::dynamic(|s: &Suggestion, _| s.suggestion_nr.to_string())
        .on_click(|ctx: &mut EventCtx, s: &mut Suggestion, _| {
            ctx.submit_command(Command::new(
                APPLY_SUGGESTION,
                s.suggestion.clone(),
                Target::Auto,
            ))
        })
        .fix_width(30.0);
    let word = Label::dynamic(|s: &Suggestion, _| s.suggestion.to_string());
    Flex::row().with_child(nr).with_flex_child(word, 1.0)
}

fn lower_box_builder() -> impl Widget<AppState> {
    let suggestions =
        Scroll::new(List::new(make_suggestion).lens(AppState::suggestions))
            .vertical();
    let editor = TextBox::multiline()
        .lens(AppState::editing_text)
        .controller(EditorController)
        .expand();
    Either::new(
        |data: &AppState, _| data.editing_linenr > 0,
        editor,
        suggestions,
    )
}

pub fn ui_builder() -> impl Widget<AppState> {
    let file_header = make_file_header().border(Color::WHITE, 2.0);
    let lines = LineList::new(make_line_item)
        .lens(FileState::lines)
        .lens(AppState::file);
    let display = LineScroller::new(lines);
    let word = Label::dynamic(|data: &AppState, _| {
        if let Some(cursor_word) = data.cursor_word() {
            format!("Word: {}", cursor_word)
        } else {
            String::new()
        }
    });
    let buttons_row = Flex::row()
        .with_default_spacer()
        .with_child(word)
        .with_flex_spacer(1.0)
        .with_child(buttons_builder())
        .with_default_spacer();
    Flex::column()
        .with_child(file_header)
        .with_spacer(2.0)
        .with_flex_child(display.border(Color::WHITE, 2.0), 1.0)
        .with_spacer(2.0)
        .with_child(buttons_row)
        .with_spacer(2.0)
        .with_flex_child(lower_box_builder(), 1.0)
        .controller(AppController)
}