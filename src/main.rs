use anyhow::{Context, Result};
use clap::Parser;
use druid::widget::{Flex, Label, LineBreaking, List, Scroll};
use druid::{AppLauncher, Data, Lens, Widget, WidgetExt, WindowDesc};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

#[derive(Parser)]
struct Cli {
    /// File to spell check.
    pathname: PathBuf,
}

const WINDOW_TITLE: &str = "CK3 spellcheck";

#[derive(Clone, Data, PartialEq)]
#[allow(clippy::upper_case_acronyms)]
enum LineEnd {
    NL,
    CRLF,
    Nothing,
}

#[derive(Clone, Data, Lens)]
struct Line {
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

    let mut lines: Vec<Line> = Vec::new();
    let mut line_iter = contents.split('\n').peekable();
    while let Some(line) = line_iter.next() {
        if line_iter.peek().is_none() {
            if !line.is_empty() {
                lines.push(Line {
                    line: line.to_string(),
                    line_end: LineEnd::Nothing,
                });
            }
        } else if line.ends_with('\r') {
            lines.push(Line {
                line: line.strip_suffix('\r').unwrap().to_string(),
                line_end: LineEnd::CRLF,
            });
        } else {
            lines.push(Line {
                line: line.to_string(),
                line_end: LineEnd::NL,
            });
        }
    }

    let data = AppState {
        pathname: Rc::new(args.pathname),
        filename: Rc::new(filename),
        lines: Arc::new(lines),
    };
    let main_window = WindowDesc::new(ui_builder())
        .title(WINDOW_TITLE.to_owned() + " " + data.filename.as_ref());
    AppLauncher::with_window(main_window)
        .log_to_console()
        .launch(data)
        .with_context(|| "could not launch application")
}

fn make_line_item() -> impl Widget<Line> {
    Label::dynamic(|line: &Line, _| line.line.to_string())
        .with_line_break_mode(LineBreaking::WordWrap)
}

fn ui_builder() -> impl Widget<AppState> {
    let lines = List::new(make_line_item).lens(AppState::lines);
    Scroll::new(Flex::column().with_child(lines)).vertical()
}
