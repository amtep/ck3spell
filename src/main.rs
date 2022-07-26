use anyhow::{anyhow, Context, Result};
use clap::Parser;
use druid::text::{Attribute, RichText};
use druid::widget::prelude::*;
use druid::{AppLauncher, Color, Key, Lens, WindowDesc};
use std::ffi::OsStr;
use std::fs::File;
use std::io::Write;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

mod appcontroller;
mod commands;
mod editorcontroller;
mod hunspell;
mod linelist;
mod linescroller;
mod syntax;
mod syntaxhighlighter;
mod ui;

use crate::hunspell::Hunspell;
use crate::syntax::{parse_line, TokenType};
use crate::ui::ui_builder;

#[derive(Parser)]
struct Cli {
    /// File to spell check.
    pathname: PathBuf,
    /// Dictionary for accepted words.
    #[clap(short, long)]
    local_dict: Option<PathBuf>,
}

const WINDOW_TITLE: &str = "CK3 spellcheck";

const LOC_KEY_COLOR: Key<Color> = Key::new("ck3spell.loc-key-color");
const WORD_COLOR: Key<Color> = Key::new("ck3spell.word-color");
const MISSPELLED_COLOR: Key<Color> = Key::new("ck3spell.misspelled-color");
const CODE_COLOR: Key<Color> = Key::new("ck3spell.code-color");
const KEYWORD_COLOR: Key<Color> = Key::new("ck3spell.keyword-color");
const ESCAPE_COLOR: Key<Color> = Key::new("ck3spell.escape-color");
const COMMENT_COLOR: Key<Color> = Key::new("ck3spell.comment-color");
const MARKUP_COLOR: Key<Color> = Key::new("ck3spell.markup-color");

const DICTIONARY_SEARCH_PATH: [&str; 2] = [".", "/usr/share/hunspell"];

#[derive(Clone, Data, PartialEq)]
#[allow(clippy::upper_case_acronyms)]
enum LineEnd {
    NL,
    CRLF,
    Nothing,
}

impl LineEnd {
    fn to_str(&self) -> &str {
        match self {
            LineEnd::NL => "\n",
            LineEnd::CRLF => "\r\n",
            LineEnd::Nothing => "",
        }
    }
}

#[derive(Clone, Data, Lens)]
struct Line {
    line_nr: usize,
    line: Rc<String>,
    line_end: LineEnd,
}

#[derive(Clone, Data, Lens)]
pub struct LineInfo {
    line: Line,
    rendered: RichText,
    bad_words: Rc<Vec<Range<usize>>>,
    highlight_word_nr: usize,
    /// Handle to the hunspell library object. Should be in Env but can't.
    hunspell: Rc<Hunspell>,
}

impl LineInfo {
    fn highlight(&mut self, env: &Env) {
        (self.rendered, self.bad_words) =
            highlight_syntax(&self.line.line, env, &self.hunspell);
        if let Some(range) = self.marked_word() {
            self.rendered
                .add_attribute(range.clone(), Attribute::underline(true));
        }
    }

    fn marked_word(&self) -> Option<Range<usize>> {
        if self.highlight_word_nr > 0 {
            self.bad_words.get(self.highlight_word_nr - 1).cloned()
        } else {
            None
        }
    }
}

/// Current highlighted bad word, as 1-based line and word number.
/// If the word number is 0 then no word is highlighted.
#[derive(Clone, Data)]
pub struct Cursor {
    linenr: usize,
    wordnr: usize,
}

#[derive(Clone, Data)]
pub struct Suggestion {
    suggestion_nr: usize, // 1-based
    suggestion: Rc<String>,
}

#[derive(Clone, Data, Lens)]
pub struct AppState {
    /// File to spell check.
    pathname: Rc<PathBuf>,
    /// Name of file to spell check, for display.
    filename: Rc<String>,
    lines: Arc<Vec<LineInfo>>,
    cursor: Cursor,
    suggestions: Arc<Vec<Suggestion>>,
    editing_linenr: usize, // 1-based
    editing_text: Arc<String>,
    /// Handle to the hunspell library object. Should be in Env but can't.
    hunspell: Rc<Hunspell>,
}

impl AppState {
    fn new(pathname: &Path, contents: &str, hunspell: Rc<Hunspell>) -> Self {
        let filename = if let Some(name) = pathname.file_name() {
            name.to_string_lossy().to_string()
        } else {
            "".to_string()
        };
        AppState {
            pathname: Rc::new(pathname.to_path_buf()),
            filename: Rc::new(filename),
            lines: Arc::new(split_lines(contents, &hunspell.clone())),
            cursor: Cursor {
                linenr: 1,
                wordnr: 0,
            },
            suggestions: Arc::new(Vec::new()),
            editing_linenr: 0,
            editing_text: Arc::new(String::new()),
            hunspell,
        }
    }

    fn cursor_prev(&mut self) {
        let mut cursor = self.cursor.clone();
        if cursor.wordnr > 1 {
            cursor.wordnr -= 1;
        } else {
            cursor.wordnr = 0;
            while cursor.linenr > 1 {
                cursor.linenr -= 1;
                let nwords = self.lines[cursor.linenr - 1].bad_words.len();
                if nwords > 0 {
                    cursor.wordnr = nwords;
                    break;
                }
            }
        }
        self.update_cursor(cursor);
        self.update_suggestions();
    }

    fn cursor_next(&mut self) {
        let mut cursor = self.cursor.clone();
        let nwords = self.lines[cursor.linenr - 1].bad_words.len();
        let nlines = self.lines.len();
        if cursor.wordnr < nwords {
            cursor.wordnr += 1;
        } else {
            cursor.wordnr = 0;
            while cursor.linenr < nlines {
                cursor.linenr += 1;
                let nwords = self.lines[cursor.linenr - 1].bad_words.len();
                if nwords > 0 {
                    cursor.wordnr = 1;
                    break;
                }
            }
        }
        self.update_cursor(cursor);
        self.update_suggestions();
    }

    fn cursor_word(&self) -> Option<&str> {
        if self.cursor.wordnr == 0 {
            return None;
        }
        if let Some(range) = self.lines[self.cursor.linenr - 1]
            .bad_words
            .get(self.cursor.wordnr - 1)
        {
            Some(&self.lines[self.cursor.linenr - 1].line.line[range.clone()])
        } else {
            None
        }
    }

    fn update_cursor(&mut self, cursor: Cursor) {
        if self.cursor.linenr != cursor.linenr {
            self.change_line(self.cursor.linenr, |lineinfo| {
                lineinfo.highlight_word_nr = 0
            });
        }
        self.change_line(cursor.linenr, |lineinfo| {
            lineinfo.highlight_word_nr = cursor.wordnr
        });
        self.cursor = cursor;
    }

    fn update_suggestions(&mut self) {
        self.suggestions = if let Some(word) = self.cursor_word() {
            Arc::new(
                self.hunspell
                    .suggestions(word)
                    .iter()
                    .enumerate()
                    .map(|(i, s)| Suggestion {
                        suggestion_nr: i + 1,
                        suggestion: s.clone(),
                    })
                    .take(9)
                    .collect(),
            )
        } else {
            Arc::new(Vec::new())
        };
    }

    fn save_file(&self) -> Result<()> {
        let mut file = File::create(&*self.pathname).with_context(|| {
            format!("Could not write to {}", self.pathname.display())
        })?;
        file.write_all("\u{FEFF}".as_bytes())?; // Unicode BOM
        for lineinfo in self.lines.iter() {
            file.write_all(lineinfo.line.line.as_bytes())?;
            file.write_all(lineinfo.line.line_end.to_str().as_bytes())?;
        }
        Ok(())
    }

    fn change_line(&mut self, linenr: usize, f: impl Fn(&mut LineInfo)) {
        let mut lines = (*self.lines).clone();
        if let Some(lineinfo) = lines.get_mut(linenr - 1) {
            f(lineinfo);
            self.lines = Arc::new(lines);
        }
    }
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

fn highlight_syntax(
    line: &Rc<String>,
    env: &Env,
    hunspell: &Rc<Hunspell>,
) -> (RichText, Rc<Vec<Range<usize>>>) {
    let mut text = RichText::new((*line.as_str()).into());
    let mut bad_words = Vec::new();

    for token in parse_line(line) {
        let mut color = match token.ttype {
            TokenType::Comment => env.get(COMMENT_COLOR),
            TokenType::LocKey => env.get(LOC_KEY_COLOR),
            TokenType::KeyReference => env.get(KEYWORD_COLOR),
            TokenType::Word => env.get(WORD_COLOR),
            TokenType::WordPart => env.get(WORD_COLOR),
            TokenType::Escape => env.get(ESCAPE_COLOR),
            TokenType::Code => env.get(CODE_COLOR),
            TokenType::Markup => env.get(MARKUP_COLOR),
        };

        if let TokenType::Word = token.ttype {
            let word = &line[token.range.clone()];
            if word.chars().count() > 1 && !hunspell.spellcheck(word) {
                color = env.get(MISSPELLED_COLOR);
                bad_words.push(token.range.clone());
            }
        }

        text.add_attribute(token.range.clone(), Attribute::text_color(color));
    }
    (text, Rc::new(bad_words))
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
            highlight_word_nr: 0,
            hunspell: Rc::clone(hunspell),
        });
    }
    lines
}

fn main() -> Result<()> {
    let args = Cli::parse();

    let mut contents =
        std::fs::read_to_string(&args.pathname).with_context(|| {
            format!("Could not read file {}", args.pathname.display())
        })?;
    if contents.starts_with('\u{feff}') {
        contents.remove(0); // Remove BOM
    }

    let locale = locale_from_filename(&args.pathname)?;
    eprintln!("Using locale {}", locale);
    let dictpath = Hunspell::find_dictionary(&DICTIONARY_SEARCH_PATH, locale)?;
    let mut hunspell = Hunspell::new(Path::new(dictpath), locale)?;
    if let Some(local_dict) = args.local_dict {
        eprint!("Using local dictionary {} ...", local_dict.display());
        let added = hunspell.set_user_dict(&local_dict)?;
        eprintln!("loaded {} words", added);
    }

    let data = AppState::new(&args.pathname, &contents, Rc::new(hunspell));
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
            env.set(MARKUP_COLOR, Color::rgb8(0x80, 0x80, 0xc0));
        })
        .launch(data)
        .with_context(|| "Could not launch application")
}
