use anyhow::{Context, Result};
use clap::Parser;
use druid::widget::{Flex, Scroll, TextBox};
use druid::{AppLauncher, Data, Lens, Widget, WidgetExt, WindowDesc};
use std::borrow::Cow;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

#[derive(Parser)]
struct Cli {
    #[clap(parse(from_os_str))]
    pathname: PathBuf,
}

const WINDOW_TITLE: &str = "CK3 spellcheck";

#[derive(Clone, Data, Lens)]
struct AppState {
    pathname: Rc<PathBuf>,
    filename: Rc<String>,
    text: Arc<String>,
}

fn main() -> Result<()> {
    let args = Cli::parse();
    let filename = match args.pathname.file_name() {
        Some(name) => name.to_string_lossy(),
        None => Cow::from(""),
    }
    .to_string();
    let contents =
        std::fs::read_to_string(&args.pathname).with_context(|| {
            format!("could not read file {}", args.pathname.display())
        })?;
    let data = AppState {
        pathname: Rc::new(args.pathname),
        filename: Rc::new(filename),
        text: Arc::new(contents),
    };
    let main_window = WindowDesc::new(ui_builder)
        .title(WINDOW_TITLE.to_owned() + " " + data.filename.as_ref());
    AppLauncher::with_window(main_window)
        .use_simple_logger()
        .launch(data)
        .with_context(|| "could not launch application")
}

fn ui_builder() -> impl Widget<AppState> {
    let textbox = TextBox::multiline().lens(AppState::text).expand_width();

    Scroll::new(Flex::column().with_child(textbox)).vertical()
}
