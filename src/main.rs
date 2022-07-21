use anyhow::{anyhow, Context, Result};
use clap::Parser;
use druid::text::{Attribute, RichText};
use druid::widget::prelude::*;
use druid::widget::{
    Button, CrossAxisAlignment, Flex, Label, LineBreaking, List, RawLabel,
    Scroll,
};
use druid::{
    AppLauncher, Color, Command, Key, Lens, Point, Rect, Selector, Target,
    WidgetExt, WidgetPod, WindowDesc,
};
use std::ffi::{CString, OsStr};
use std::fs::File;
use std::ops::Range;
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
const ESCAPE_COLOR: Key<Color> = Key::new("ck3spell.escape-color");
const COMMENT_COLOR: Key<Color> = Key::new("ck3spell.comment-color");

const DICTIONARY_SEARCH_PATH: [&str; 2] = [".", "/usr/share/hunspell"];

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
}

#[derive(Clone, Data, Lens)]
struct LineInfo {
    line: Line,
    rendered: RichText,
    bad_words: Rc<Vec<Range<usize>>>,
    /// Handle to the hunspell library object. Should be in Env but can't.
    hunspell: Rc<Hunspell>,
}

/// Current highlighted bad word, as 1-based line and word number.
/// If the word number is 0 then no word is highlighted.
#[derive(Clone, Data)]
struct Cursor {
    linenr: usize,
    wordnr: usize,
}

#[derive(Clone, Data, Lens)]
struct AppState {
    /// File to spell check.
    pathname: Rc<PathBuf>,
    /// Name of file to spell check, for display.
    filename: Rc<String>,
    lines: Arc<Vec<LineInfo>>,
    cursor: Cursor,
}

impl AppState {
    fn cursor_prev(&mut self) {
        if self.cursor.wordnr > 1 {
            self.cursor.wordnr -= 1;
        } else {
            self.cursor.wordnr = 0;
            while self.cursor.linenr > 1 {
                self.cursor.linenr -= 1;
                let words = self.lines[self.cursor.linenr - 1].bad_words.len();
                if words > 0 {
                    self.cursor.wordnr = words;
                    break;
                }
            }
        }
    }

    fn cursor_next(&mut self) {
        let words = self.lines[self.cursor.linenr - 1].bad_words.len();
        let lines = self.lines.len();
        if self.cursor.wordnr < words {
            self.cursor.wordnr += 1;
        } else {
            self.cursor.wordnr = 0;
            while self.cursor.linenr < lines {
                self.cursor.linenr += 1;
                let words = self.lines[self.cursor.linenr - 1].bad_words.len();
                if words > 0 {
                    self.cursor.wordnr = 1;
                    break;
                }
            }
        }
    }

    fn cursor_word(&self) -> Option<&str> {
        if self.cursor.wordnr == 0 {
            None
        } else {
            let range = self.lines[self.cursor.linenr - 1].bad_words
                [self.cursor.wordnr - 1]
                .clone();
            Some(&self.lines[self.cursor.linenr - 1].line.line[range])
        }
    }
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
    fn _path_helper(
        path: &Path,
        locale: &str,
        ext: &str,
        errname: &str,
    ) -> Result<CString> {
        let mut p = path.to_path_buf();
        p.push(format!("{}.{}", locale, ext));

        // Hunspell itself won't tell us if opening the dictionary fails,
        // so check it here.
        File::open(&p).with_context(|| {
            format!("Could not open {} file {}", errname, p.display())
        })?;

        // These unwraps won't panic because we have full control over the incoming pathname.
        Ok(CString::new(p.as_os_str().to_str().unwrap()).unwrap())
    }

    fn new(path: &Path, locale: &str) -> Result<Hunspell> {
        let c_dpath =
            Hunspell::_path_helper(path, locale, "dic", "dictionary")?;
        let c_affpath = Hunspell::_path_helper(path, locale, "aff", "affix")?;

        unsafe {
            let handle = Hunspell_create(c_affpath.as_ptr(), c_dpath.as_ptr());
            Ok(Hunspell { handle })
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

fn find_dictionary(locale: &str) -> Result<&str> {
    for dir in DICTIONARY_SEARCH_PATH {
        eprint!("Looking for dictionary in {}", dir);
        let filename = format!("{}.dic", locale);
        let mut p = PathBuf::from(dir);
        p.push(filename);
        if Path::exists(&p) {
            eprintln!(" ...found");
            return Ok(dir);
        }
        eprintln!();
    }
    Err(anyhow!("Dictionary not found"))
}

const LANGUAGES: [(&str, &str, &str); 7] = [
    ("l_english", "en_US", "English"),
    ("l_german", "de_DE", "German"),
    ("l_french", "fr_FR", "French"),
    ("l_spanish", "es_ES", "Spanish"),
    ("l_russian", "ru_RU", "Russian"),
    ("l_korean", "", "Korean"),
    ("l_simp_chinese", "", "Chinese"),
];

fn locale_from_filename(pathname: &Path) -> Result<&str> {
    let filename = pathname
        .file_name()
        .unwrap_or_else(|| OsStr::new(""))
        .to_str()
        .unwrap_or("");
    for (tag, locale, name) in LANGUAGES {
        if filename.ends_with(&format!("_{}.yml", tag)) {
            if !locale.is_empty() {
                return Ok(locale);
            } else {
                return Err(anyhow!("{} not supported", name));
            }
        }
    }
    Err(anyhow!("Could not determine language from filename"))
}

fn is_word_char(c: char) -> bool {
    // U+2019 is the unicode apostrophe
    // alphanumeric is accepted for words like "2nd"
    c.is_alphanumeric() || c == '\'' || c == '\u{2019}' || c == '-'
}

fn highlight_syntax(
    line: &Rc<String>,
    env: &Env,
    hunspell: &Rc<Hunspell>,
) -> (RichText, Rc<Vec<Range<usize>>>) {
    let mut text = RichText::new((*line.as_str()).into());
    let mut bad_words = Vec::new();

    enum State {
        Init,
        AwaitingSpaceOrQuote,
        NormalText,
        Escape(usize),
        InWord(usize),
        InKeyword(usize),
        InCode(usize),
    }

    let mut state: State = State::Init;
    let mut word: String = String::new();

    for (pos, c) in line.char_indices() {
        match state {
            State::Init => {
                if c == ':' {
                    state = State::AwaitingSpaceOrQuote;
                } else if c == '#' {
                    text.add_attribute(
                        pos..line.len(),
                        Attribute::text_color(env.get(COMMENT_COLOR)),
                    );
                    break;
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
                } else if c == '\\' {
                    state = State::Escape(pos);
                } else if is_word_char(c) {
                    word = String::new();
                    word.push(c);
                    state = State::InWord(pos);
                }
            }
            State::InWord(from) => {
                // TODO: checking of complex words that contain [ ] parts
                let mut paint_word = false;
                if c == '$' {
                    state = State::InKeyword(pos);
                } else if c == '[' {
                    state = State::InCode(pos);
                } else if c == '\\' {
                    paint_word = true;
                    state = State::Escape(pos);
                } else if is_word_char(c) {
                    if c == '\'' {
                        word.push('\u{2019}')
                    } else {
                        word.push(c);
                    }
                } else if !is_word_char(c) {
                    paint_word = true;
                    state = State::NormalText;
                }
                if paint_word {
                    if word.chars().count() <= 1 || hunspell.spellcheck(&word) {
                        text.add_attribute(
                            from..pos,
                            Attribute::text_color(env.get(WORD_COLOR)),
                        );
                    } else {
                        text.add_attribute(
                            from..pos,
                            Attribute::text_color(env.get(MISSPELLED_COLOR)),
                        );
                        bad_words.push(from..pos);
                    }
                }
            }
            State::Escape(from) => {
                text.add_attribute(
                    from..pos + c.len_utf8(),
                    Attribute::text_color(env.get(ESCAPE_COLOR)),
                );
                state = State::NormalText;
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
    (text, Rc::new(bad_words))
}

const QUERY_LINE_LAYOUT_REGION: Selector<usize> =
    Selector::new("query_line_layout_region");
const REPLY_LINE_LAYOUT_REGION: Selector<Rect> =
    Selector::new("reply_line_layout_region");

struct SyntaxHighlighter<W> {
    child: WidgetPod<LineInfo, W>,
    old_line: Option<Rc<String>>,
}

impl<W: Widget<LineInfo>> SyntaxHighlighter<W> {
    fn new(child: W) -> SyntaxHighlighter<W> {
        SyntaxHighlighter {
            child: WidgetPod::new(child),
            old_line: None,
        }
    }
}

impl<W: Widget<LineInfo>> Widget<LineInfo> for SyntaxHighlighter<W> {
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        event: &Event,
        data: &mut LineInfo,
        env: &Env,
    ) {
        if let Event::Command(command) = event {
            if let Some(&linenr) = command.get(QUERY_LINE_LAYOUT_REGION) {
                if linenr == data.line.line_nr {
                    let command = Command::new(
                        REPLY_LINE_LAYOUT_REGION,
                        self.child.layout_rect(),
                        Target::Auto,
                    );
                    ctx.submit_notification(command);
                }
            }
        }
        self.child.event(ctx, event, data, env);
        if self.old_line.is_none()
            || !data.line.line.same(self.old_line.as_ref().unwrap())
        {
            (data.rendered, data.bad_words) =
                highlight_syntax(&data.line.line, env, &data.hunspell);
            self.old_line = Some(data.line.line.clone());
        }
    }

    fn lifecycle(
        &mut self,
        ctx: &mut LifeCycleCtx,
        event: &LifeCycle,
        data: &LineInfo,
        env: &Env,
    ) {
        self.child.lifecycle(ctx, event, data, env);
    }

    fn update(
        &mut self,
        ctx: &mut UpdateCtx,
        _old_data: &LineInfo,
        data: &LineInfo,
        env: &Env,
    ) {
        self.child.update(ctx, data, env);
    }

    fn layout(
        &mut self,
        ctx: &mut LayoutCtx,
        bc: &BoxConstraints,
        data: &LineInfo,
        env: &Env,
    ) -> Size {
        bc.debug_check("SyntaxHighlighter");
        let size = self.child.layout(ctx, bc, data, env);
        self.child.set_origin(ctx, data, env, Point::ZERO);
        size
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &LineInfo, env: &Env) {
        self.child.paint(ctx, data, env);
    }
}

struct LineScroller<W> {
    scroll: WidgetPod<AppState, Scroll<AppState, W>>,
    old_cursor: Cursor,
}

impl<W: Widget<AppState>> LineScroller<W> {
    fn new(child: W) -> LineScroller<W> {
        LineScroller {
            scroll: WidgetPod::new(Scroll::new(child).vertical()),
            old_cursor: Cursor {
                linenr: 1,
                wordnr: 0,
            },
        }
    }
}

impl<W: Widget<AppState>> Widget<AppState> for LineScroller<W> {
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        event: &Event,
        data: &mut AppState,
        env: &Env,
    ) {
        self.scroll.event(ctx, event, data, env);
        if !data.cursor.same(&self.old_cursor) {
            self.old_cursor = data.cursor.clone();
            let command = Command::new(
                QUERY_LINE_LAYOUT_REGION,
                data.cursor.linenr,
                Target::Auto,
            );
            ctx.submit_command(command);
        }
    }

    fn lifecycle(
        &mut self,
        ctx: &mut LifeCycleCtx,
        event: &LifeCycle,
        data: &AppState,
        env: &Env,
    ) {
        self.scroll.lifecycle(ctx, event, data, env);
    }

    fn update(
        &mut self,
        ctx: &mut UpdateCtx,
        _old_data: &AppState,
        data: &AppState,
        env: &Env,
    ) {
        self.scroll.update(ctx, data, env);
    }

    fn layout(
        &mut self,
        ctx: &mut LayoutCtx,
        bc: &BoxConstraints,
        data: &AppState,
        env: &Env,
    ) -> Size {
        bc.debug_check("LineScroller");
        let size = self.scroll.layout(ctx, bc, data, env);
        self.scroll.set_origin(ctx, data, env, Point::ZERO);
        size
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &AppState, env: &Env) {
        self.scroll.paint(ctx, data, env);
    }
}

fn split_lines(contents: &str, hunspell: &Rc<Hunspell>) -> Vec<LineInfo> {
    let mut lines: Vec<LineInfo> = Vec::new();
    let mut line_iter = contents.split('\n').enumerate().peekable();
    while let Some((nr, line)) = line_iter.next() {
        let numbered_line = if line_iter.peek().is_none() {
            if !line.is_empty() {
                Line {
                    line_nr: nr + 1,
                    line: Rc::new(line.to_string()),
                    line_end: LineEnd::Nothing,
                }
            } else {
                continue;
            }
        } else if line.ends_with('\r') {
            Line {
                line_nr: nr + 1,
                line: Rc::new(line.strip_suffix('\r').unwrap().to_string()),
                line_end: LineEnd::CRLF,
            }
        } else {
            Line {
                line_nr: nr + 1,
                line: Rc::new(line.to_string()),
                line_end: LineEnd::NL,
            }
        };
        lines.push(LineInfo {
            line: numbered_line,
            rendered: RichText::new("".into()),
            bad_words: Rc::new(Vec::new()),
            hunspell: Rc::clone(hunspell),
        });
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
            format!("Could not read file {}", args.pathname.display())
        })?;
    if contents.starts_with('\u{feff}') {
        contents.remove(0); // Remove BOM
    }

    let locale = locale_from_filename(&args.pathname)?;
    eprintln!("Using locale {}", locale);
    let dictpath = find_dictionary(locale)?;
    let hunspell = Hunspell::new(Path::new(dictpath), locale)?;

    let data = AppState {
        pathname: Rc::new(args.pathname),
        filename: Rc::new(filename),
        lines: Arc::new(split_lines(&contents, &Rc::new(hunspell))),
        cursor: Cursor {
            linenr: 1,
            wordnr: 0,
        },
    };
    let main_window = WindowDesc::new(ui_builder())
        .title(WINDOW_TITLE.to_owned() + " " + data.filename.as_ref())
        .window_size((1000.0, 500.0));
    AppLauncher::with_window(main_window)
        .log_to_console()
        .configure_env(|env, _| {
            env.set(LOC_KEY_COLOR, Color::rgb8(0xff, 0xa5, 0x00));
            env.set(WORD_COLOR, Color::rgb8(0xFF, 0xFF, 0xFF));
            env.set(MISSPELLED_COLOR, Color::rgb8(0xFF, 0x40, 0x40));
            env.set(CODE_COLOR, Color::rgb8(0x40, 0x40, 0xFF));
            env.set(KEYWORD_COLOR, Color::rgb8(0xc0, 0xa0, 0x00));
            env.set(ESCAPE_COLOR, Color::rgb8(0xc0, 0xa0, 0x00));
            env.set(COMMENT_COLOR, Color::rgb8(0xc0, 0xa0, 0x50));
        })
        .launch(data)
        .with_context(|| "Could not launch application")
}

fn make_line_item() -> impl Widget<LineInfo> {
    let linenr =
        Label::dynamic(|line: &LineInfo, _| line.line.line_nr.to_string())
            .with_text_color(Color::grey8(160))
            .fix_width(30.0);
    let line = SyntaxHighlighter::new(
        RawLabel::new()
            .with_line_break_mode(LineBreaking::WordWrap)
            .lens(LineInfo::rendered),
    );
    Flex::row()
        .with_child(Flex::column().with_child(linenr))
        .with_flex_child(line, 1.0)
        .cross_axis_alignment(CrossAxisAlignment::Start)
}

fn buttons_builder() -> impl Widget<AppState> {
    let prev = Button::new("Previous").on_click(|_, data: &mut AppState, _| {
        data.cursor_prev();
    });
    let next = Button::new("Next").on_click(|_, data: &mut AppState, _| {
        data.cursor_next();
    });
    let accept = Button::new("Accept word");
    let edit = Button::new("Edit line");
    let save = Button::new("Save and Exit");
    let quit = Button::new("Quit without Saving");
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
        .with_default_spacer()
        .with_child(quit)
}

fn ui_builder() -> impl Widget<AppState> {
    let lines = List::new(make_line_item).lens(AppState::lines);
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
        .with_flex_child(display.border(Color::WHITE, 2.0), 1.0)
        .with_spacer(2.0)
        .with_child(buttons_row)
}
