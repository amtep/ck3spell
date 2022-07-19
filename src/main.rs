use anyhow::{Context, Result};
use clap::Parser;
use druid::text::{Attribute, RichText};
use druid::widget::{
    Controller, CrossAxisAlignment, Flex, Label, LineBreaking, List, RawLabel,
    Scroll,
};
use druid::{
    AppLauncher, Color, Data, Env, Event, EventCtx, Key, Lens, Widget,
    WidgetExt, WindowDesc,
};
use std::ffi::CString;
use std::os::raw::c_int;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

#[derive(Parser)]
struct Cli {
    /// File to spell check.
    pathname: PathBuf,
}

const WINDOW_TITLE: &str = "CK3 spellcheck";

const LOC_KEY_COLOR: Key<Color> = Key::new("ck3spell.loc-key-color");
const WORD_COLOR: Key<Color> = Key::new("ck3spell.word-color");
const MISSPELLED_COLOR: Key<Color> = Key::new("ck3spell.misspelled-color");
const CODE_COLOR: Key<Color> = Key::new("ck3spell.code-color");
const KEYWORD_COLOR: Key<Color> = Key::new("ck3spell.keyword-color");

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
    line: Rc<String>,
    line_end: LineEnd,
    rendered: RichText,
    /// Handle to the hunspell library object. Should be in Env but can't.
    hunspell: Rc<Hunspell>,
}

#[derive(Clone, Data, Lens)]
struct AppState {
    /// File to spell check.
    pathname: Rc<PathBuf>,
    /// Name of file to spell check, for display.
    filename: Rc<String>,
    lines: Arc<Vec<Line>>,
}

/// Opaque type representing a Hunhandle in C
#[repr(C)]
struct Hunhandle {
    _data: [u8; 0],
    _marker: core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
}

#[link(name = "hunspell")]
extern "C" {
    fn Hunspell_create(affpath: *const i8, dpath: *const i8) -> *mut Hunhandle;
    fn Hunspell_destroy(pHunspell: *mut Hunhandle);
    fn Hunspell_spell(pHunspell: *mut Hunhandle, word: *const i8) -> c_int;
}

struct Hunspell {
    handle: *mut Hunhandle,
}

impl Hunspell {
    fn _path_helper(path: &Path, locale: &str, ext: &str) -> CString {
        let mut p = path.to_path_buf();
        p.push(format!("{}.{}", locale, ext));
        CString::new(p.as_os_str().to_str().unwrap()).unwrap()
    }

    fn new(path: &Path, locale: &str) -> Hunspell {
        let c_affpath = Hunspell::_path_helper(path, locale, "aff");
        let c_dpath = Hunspell::_path_helper(path, locale, "dic");

        unsafe {
            let handle = Hunspell_create(c_affpath.as_ptr(), c_dpath.as_ptr());
            Hunspell { handle }
        }
    }

    fn spellcheck(&self, word: &str) -> bool {
        let c_word = if let Ok(c_word) = CString::new(word) {
            c_word
        } else {
            return true;
        };
        unsafe {
            let result = Hunspell_spell(self.handle, c_word.as_ptr());
            result != 0
        }
    }
}

impl Drop for Hunspell {
    fn drop(&mut self) {
        unsafe {
            Hunspell_destroy(self.handle);
        }
    }
}

fn is_word_char(c: char) -> bool {
    // 2019 is the unicode apostrophe
    c.is_alphabetic() || c == '\'' || c == '\u{2019}'
}

fn highlight_syntax(
    line: &Rc<String>,
    env: &Env,
    hunspell: &Rc<Hunspell>,
) -> RichText {
    let mut text = RichText::new((*line.as_str()).into());

    enum State {
        Init,
        AwaitingSpaceOrQuote,
        NormalText,
        InWord(usize),
        InKeyword(usize),
        InCode(usize),
    }

    let mut state: State = State::Init;
    let mut word: String = String::new();

    for (pos, c) in line.chars().enumerate() {
        match state {
            State::Init => {
                if c == ':' {
                    state = State::AwaitingSpaceOrQuote;
                }
            }
            State::AwaitingSpaceOrQuote => {
                if c == ' ' || c == '"' {
                    text.add_attribute(
                        0..pos,
                        Attribute::text_color(env.get(LOC_KEY_COLOR)),
                    );
                    state = State::NormalText;
                }
            }
            State::NormalText => {
                if c == '$' {
                    state = State::InKeyword(pos);
                } else if c == '[' {
                    state = State::InCode(pos);
                } else if is_word_char(c) {
                    word = String::new();
                    word.push(c);
                    state = State::InWord(pos);
                }
            }
            State::InWord(from) => {
                // TODO: checking of complex words that contain [ ] parts
                if c == '$' {
                    state = State::InKeyword(pos);
                } else if c == '[' {
                    state = State::InCode(pos);
                } else if is_word_char(c) {
                    word.push(c);
                } else if !is_word_char(c) {
                    if hunspell.spellcheck(&word) {
                        text.add_attribute(
                            from..pos,
                            Attribute::text_color(env.get(WORD_COLOR)),
                        );
                    } else {
                        text.add_attribute(
                            from..pos,
                            Attribute::text_color(env.get(MISSPELLED_COLOR)),
                        );
                    }
                    state = State::NormalText;
                }
            }
            State::InKeyword(from) => {
                if c == '$' {
                    text.add_attribute(
                        from..pos + 1,
                        Attribute::text_color(env.get(KEYWORD_COLOR)),
                    );
                    state = State::NormalText;
                }
            }
            State::InCode(from) => {
                if c == ']' {
                    text.add_attribute(
                        from..pos + 1,
                        Attribute::text_color(env.get(CODE_COLOR)),
                    );
                    state = State::NormalText;
                }
            }
        }
    }
    text
}

struct SyntaxHighlighter;

impl<W: Widget<Line>> Controller<Line, W> for SyntaxHighlighter {
    fn event(
        &mut self,
        child: &mut W,
        ctx: &mut EventCtx,
        event: &Event,
        data: &mut Line,
        env: &Env,
    ) {
        let pre_data = data.line.to_owned();
        child.event(ctx, event, data, env);
        if !data.line.same(&pre_data)
            || (data.rendered.is_empty() && !data.line.is_empty())
        {
            data.rendered = highlight_syntax(&data.line, env, &data.hunspell);
        }
    }
}

fn split_lines(contents: &str, hunspell: &Rc<Hunspell>) -> Vec<Line> {
    let mut lines: Vec<Line> = Vec::new();
    let mut line_iter = contents.split('\n').enumerate().peekable();
    while let Some((nr, line)) = line_iter.next() {
        if line_iter.peek().is_none() {
            if !line.is_empty() {
                lines.push(Line {
                    line_nr: nr + 1,
                    line: Rc::new(line.to_string()),
                    line_end: LineEnd::Nothing,
                    rendered: RichText::new("".into()),
                    hunspell: Rc::clone(hunspell),
                });
            }
        } else if line.ends_with('\r') {
            lines.push(Line {
                line_nr: nr + 1,
                line: Rc::new(line.strip_suffix('\r').unwrap().to_string()),
                line_end: LineEnd::CRLF,
                rendered: RichText::new("".into()),
                hunspell: Rc::clone(hunspell),
            });
        } else {
            lines.push(Line {
                line_nr: nr + 1,
                line: Rc::new(line.to_string()),
                line_end: LineEnd::NL,
                rendered: RichText::new("".into()),
                hunspell: Rc::clone(hunspell),
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

    let hunspell = Hunspell::new(Path::new("/usr/share/hunspell"), "en_US");

    let data = AppState {
        pathname: Rc::new(args.pathname),
        filename: Rc::new(filename),
        lines: Arc::new(split_lines(&contents, &Rc::new(hunspell))),
    };
    let main_window = WindowDesc::new(ui_builder())
        .title(WINDOW_TITLE.to_owned() + " " + data.filename.as_ref());
    AppLauncher::with_window(main_window)
        .log_to_console()
        .configure_env(|env, _| {
            env.set(LOC_KEY_COLOR, Color::rgb8(0xff, 0xa5, 0x00));
            env.set(WORD_COLOR, Color::rgb8(0xFF, 0xFF, 0xFF));
            env.set(MISSPELLED_COLOR, Color::rgb8(0xFF, 0x40, 0x40));
            env.set(CODE_COLOR, Color::rgb8(0x40, 0x40, 0xFF));
            env.set(KEYWORD_COLOR, Color::rgb8(0xc0, 0xa0, 0x00));
        })
        .launch(data)
        .with_context(|| "could not launch application")
}

fn make_line_item() -> impl Widget<Line> {
    let linenr = Label::dynamic(|line: &Line, _| line.line_nr.to_string())
        .with_text_color(Color::grey8(160))
        .fix_width(30.0);
    let line = RawLabel::new()
        .with_line_break_mode(LineBreaking::WordWrap)
        .lens(Line::rendered)
        .controller(SyntaxHighlighter);
    Flex::row()
        .with_child(Flex::column().with_child(linenr))
        .with_flex_child(line, 1.0)
        .cross_axis_alignment(CrossAxisAlignment::Start)
}

fn ui_builder() -> impl Widget<AppState> {
    let lines = List::new(make_line_item).lens(AppState::lines);
    Scroll::new(Flex::column().with_child(lines)).vertical()
}
