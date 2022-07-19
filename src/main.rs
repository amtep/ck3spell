use anyhow::{Context, Result};
use clap::Parser;
use druid::widget::{
    CrossAxisAlignment, Flex, Label, LineBreaking, List, Scroll,
};
use druid::{
    AppLauncher, Color, Data, Key, Lens, Widget, WidgetExt, WindowDesc,
};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

#[derive(Parser)]
struct Cli {
    /// File to spell check.
    pathname: PathBuf,
}

const WINDOW_TITLE: &str = "CK3 spellcheck";

const LOC_KEY_COLOR: Key<Color> = Key::new("ck3spell.loc-key-color");
const NORMAL_TEXT_COLOR: Key<Color> = Key::new("ck3spell.normal-text-color");
const CODE_COLOR: Key<Color> = Key::new("ck3spell.code-color");

#[derive(Clone, Data, PartialEq)]
#[allow(clippy::upper_case_acronyms)]
enum LineEnd {
    NL,
    CRLF,
    Nothing,
}

#[derive(Clone, Data, Lens)]
struct Line {
    line_nr: usize,
    line: String,
    line_end: LineEnd,
}

#[derive(Clone, Data, Lens)]
struct AppState {
    /// File to spell check.
    pathname: Rc<PathBuf>,
    /// Name of file to spell check, for display.
    filename: Rc<String>,
    lines: Arc<Vec<Line>>,
}

fn split_lines(contents: &str) -> Vec<Line> {
    let mut lines: Vec<Line> = Vec::new();
    let mut line_iter = contents.split('\n').enumerate().peekable();
    while let Some((nr, line)) = line_iter.next() {
        if line_iter.peek().is_none() {
            if !line.is_empty() {
                lines.push(Line {
                    line_nr: nr + 1,
                    line: line.to_string(),
                    line_end: LineEnd::Nothing,
                });
            }
        } else if line.ends_with('\r') {
            lines.push(Line {
                line_nr: nr + 1,
                line: line.strip_suffix('\r').unwrap().to_string(),
                line_end: LineEnd::CRLF,
            });
        } else {
            lines.push(Line {
                line_nr: nr + 1,
                line: line.to_string(),
                line_end: LineEnd::NL,
            });
        }
    }
    lines
}

fn main() -> Result<()> {
    let args = Cli::parse();
    let filename = if let Some(name) = args.pathname.file_name() {
        name.to_string_lossy().to_string()
    } else {
        "".to_string()
    };

    let mut contents =
        std::fs::read_to_string(&args.pathname).with_context(|| {
            format!("could not read file {}", args.pathname.display())
        })?;
    if contents.starts_with('\u{feff}') {
        contents.remove(0); // Remove BOM
    }

    let data = AppState {
        pathname: Rc::new(args.pathname),
        filename: Rc::new(filename),
        lines: Arc::new(split_lines(&contents)),
    };
    let main_window = WindowDesc::new(ui_builder())
        .title(WINDOW_TITLE.to_owned() + " " + data.filename.as_ref());
    AppLauncher::with_window(main_window)
        .log_to_console()
        .configure_env(|env, _| {
            env.set(LOC_KEY_COLOR, Color::rgb8(0xA5, 0x2A, 0x2A));
            env.set(NORMAL_TEXT_COLOR, Color::rgb8(0xFF, 0xFF, 0xFF));
            env.set(CODE_COLOR, Color::rgb8(0x2A, 0x2A, 0xA5));
        })
        .launch(data)
        .with_context(|| "could not launch application")
}

fn make_line_item() -> impl Widget<Line> {
    let linenr = Label::dynamic(|line: &Line, _| line.line_nr.to_string())
        .with_text_color(Color::grey8(160))
        .fix_width(30.0);
    let line = Label::dynamic(|line: &Line, _| line.line.to_string())
        .with_line_break_mode(LineBreaking::WordWrap);
    Flex::row()
        .with_child(Flex::column().with_child(linenr))
        .with_flex_child(line, 1.0)
        .cross_axis_alignment(CrossAxisAlignment::Start)
}

fn ui_builder() -> impl Widget<AppState> {
    let lines = List::new(make_line_item).lens(AppState::lines);
    Scroll::new(Flex::column().with_child(lines)).vertical()
}
