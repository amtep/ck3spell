use clap::Parser;
use druid::widget::{ Button, Flex, Label };
use druid::{
    AppLauncher, Data, LocalizedString, PlatformError,
    Widget, WidgetExt, WindowDesc
};
use std::path::PathBuf;
use std::rc::Rc;

#[derive(Parser)]
struct Cli {
    #[clap(parse(from_os_str))]
    pathname: PathBuf,
}

const WINDOW_TITLE: LocalizedString<AppState>
    = LocalizedString::new("CK3 spellcheck");

#[derive(Clone, Data)]
struct AppState {
    pathname: Rc<PathBuf>,
}

fn main() -> Result<(), PlatformError> {
    let args = Cli::parse();
    let data = AppState {
        pathname: Rc::new(args.pathname),
    };
    let main_window = WindowDesc::new(ui_builder)
        .title(WINDOW_TITLE);
    AppLauncher::with_window(main_window)
        .use_simple_logger()
        .launch(data)
}

fn ui_builder() -> impl Widget<AppState> {
    let text = LocalizedString::new("hello-counter");
    let label = Label::new(text).padding(5.0).center();
    let button = Button::new("increment")
        .padding(5.0);

    Flex::column().with_child(label).with_child(button)
}
