use anyhow::{anyhow, bail, Context, Result};
use clap::Parser;
use druid::text::{Attribute, RichText};
use druid::widget::prelude::*;
use druid::{AppLauncher, Color, Key, Lens, WindowDesc};
use home::home_dir;
use std::cell::RefCell;
use std::collections::HashMap;
use std::env::current_exe;
use std::ffi::OsStr;
use std::fs::File;
use std::io::Write;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use speller::{Speller, SpellerHunspellDict};

mod appcontroller;
mod commands;
mod editorcontroller;
mod linelist;
mod linescroller;
mod syntax;
mod syntaxhighlighter;
mod ui;

use crate::syntax::{parse_line, TokenType};
use crate::ui::ui_builder;

#[derive(Parser)]
struct Cli {
    /// Files to spell check.
    #[clap(required(true))]
    pathnames: Vec<PathBuf>,
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
const ICON_TAG_COLOR: Key<Color> = Key::new("ck3spell.icon-tag-color");
const LINE_COLOR: Key<Color> = Key::new("ck3spell-line-color");

const DICTIONARY_SEARCH_PATH: [&str; 5] =
    ["./dicts", ".", "/usr/share/hunspell", "$EXE/dicts", "$EXE"];

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
    speller: Rc<RefCell<dyn Speller>>, // Should be in Env but can't.
}

impl LineInfo {
    fn highlight(&mut self, env: &Env) {
        (self.rendered, self.bad_words) =
            highlight_syntax(&self.line.line, env, &self.speller);
        if let Some(range) = self.marked_word() {
            self.rendered
                .add_attribute(range, Attribute::underline(true));
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
#[derive(Clone, Copy, Data)]
pub struct Cursor {
    linenr: usize,
    wordnr: usize,
}

impl Default for Cursor {
    fn default() -> Cursor {
        Cursor {
            linenr: 1,
            wordnr: 0,
        }
    }
}

#[derive(Clone, Data)]
pub struct Suggestion {
    suggestion_nr: usize, // 1-based
    suggestion: Rc<String>,
}

#[derive(Clone, Data, Lens)]
pub struct FileState {
    /// File to spell check.
    pathname: Rc<PathBuf>,
    /// Name of file to spell check, for display.
    filename: Rc<String>,
    lines: Arc<Vec<LineInfo>>,
    speller: Rc<RefCell<dyn Speller>>,
}

impl FileState {
    fn new(
        pathname: &Path,
        contents: &str,
        speller: Rc<RefCell<dyn Speller>>,
    ) -> Self {
        let filename = if let Some(name) = pathname.file_name() {
            name.to_string_lossy().to_string()
        } else {
            "".to_string()
        };
        FileState {
            pathname: Rc::new(pathname.to_path_buf()),
            filename: Rc::new(filename),
            lines: Arc::new(split_lines(contents, &speller.clone())),
            speller,
        }
    }

    fn save(&self) -> Result<()> {
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
}

#[derive(Clone, Data, Lens)]
pub struct AppState {
    /// Currently shown and edited file.
    file: FileState,
    files: Rc<Vec<FileState>>,
    file_idx: usize, // 0-based
    cursor: Cursor,
    suggestions: Arc<Vec<Suggestion>>,
    editing_linenr: usize, // 1-based
    editing_text: Arc<String>,
}

impl AppState {
    fn new(files: Rc<Vec<FileState>>) -> Self {
        AppState {
            file: files[0].clone(),
            files: files.clone(),
            file_idx: 0,
            cursor: Cursor::default(),
            suggestions: Arc::new(Vec::new()),
            editing_linenr: 0,
            editing_text: Arc::new(String::new()),
        }
    }

    fn file_prev(&mut self) {
        if self.file_idx == 0 {
            return;
        }

        self.update_cursor(Cursor::default());
        self.update_suggestions();

        self.file_idx -= 1;
        self.file = self.files[self.file_idx].clone();
    }

    fn file_next(&mut self) {
        if self.file_idx == self.files.len() - 1 {
            return;
        }

        self.update_cursor(Cursor::default());
        self.update_suggestions();

        self.file_idx += 1;
        self.file = self.files[self.file_idx].clone();
    }

    fn cursor_prev(&mut self) {
        let mut cursor = self.cursor;
        if cursor.wordnr > 1 {
            cursor.wordnr -= 1;
        } else {
            cursor.wordnr = 0;
            while cursor.linenr > 1 {
                cursor.linenr -= 1;
                let nwords = self.file.lines[cursor.linenr - 1].bad_words.len();
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
        let mut cursor = self.cursor;
        let nwords = self.file.lines[cursor.linenr - 1].bad_words.len();
        let nlines = self.file.lines.len();
        if cursor.wordnr < nwords {
            cursor.wordnr += 1;
        } else {
            cursor.wordnr = 0;
            while cursor.linenr < nlines {
                cursor.linenr += 1;
                let nwords = self.file.lines[cursor.linenr - 1].bad_words.len();
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
        if let Some(range) = self.file.lines[self.cursor.linenr - 1]
            .bad_words
            .get(self.cursor.wordnr - 1)
        {
            Some(
                &self.file.lines[self.cursor.linenr - 1].line.line
                    [range.clone()],
            )
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
                self.file
                    .speller
                    .borrow()
                    .suggestions(word, 9)
                    .iter()
                    .take(9)
                    .enumerate()
                    .map(|(i, s)| Suggestion {
                        suggestion_nr: i + 1,
                        suggestion: Rc::new(s.to_string()),
                    })
                    .collect(),
            )
        } else {
            Arc::new(Vec::new())
        };
    }

    fn save_file(&self) -> Result<()> {
        self.file.save()
    }

    fn drop_file(&mut self) {
        self.update_cursor(Cursor::default());
        self.update_suggestions();

        let mut files = (*self.files).clone();
        files.remove(self.file_idx);
        self.files = Rc::new(files);
        self.file = self.files[self.file_idx].clone();
    }

    fn change_line(&mut self, linenr: usize, f: impl Fn(&mut LineInfo)) {
        // This takes the self.file version of the file as authoritative,
        // and copies it into the self.files vec.
        let mut files = (*self.files).clone();
        let mut lines = (*self.file.lines).clone();
        if let Some(lineinfo) = lines.get_mut(linenr - 1) {
            f(lineinfo);
            files[self.file_idx].lines = Arc::new(lines);
            self.files = Rc::new(files);
            self.file = self.files[self.file_idx].clone();
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
    speller: &Rc<RefCell<dyn Speller>>,
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
            TokenType::IconTag => env.get(ICON_TAG_COLOR),
        };

        if let TokenType::Word = token.ttype {
            let word = &line[token.range.clone()];
            if word.chars().count() > 1 && !speller.borrow().spellcheck(word) {
                color = env.get(MISSPELLED_COLOR);
                bad_words.push(token.range.clone());
            }
        }

        text.add_attribute(token.range.clone(), Attribute::text_color(color));
    }
    (text, Rc::new(bad_words))
}

fn split_lines(
    contents: &str,
    speller: &Rc<RefCell<dyn Speller>>,
) -> Vec<LineInfo> {
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
            speller: Rc::clone(speller),
        });
    }
    lines
}

/// Look for paths starting with $HOME or $EXE and fill in the user's
/// home directory or the ck3spell executable's directory, respectively.
fn expand_dir(dir: &Path) -> Option<PathBuf> {
    if let Ok(path) = dir.strip_prefix("$HOME") {
        Some(home_dir()?.join(path))
    } else if let Ok(path) = dir.strip_prefix("$EXE") {
        match current_exe() {
            Ok(exe) => Some(exe.parent()?.join(path)),
            Err(_) => None,
        }
    } else {
        Some(dir.to_path_buf())
    }
}

/// Look for Hunspell-format dictionaries for the given `locale` in the
/// provided directory search path. Return a tuple of paths to the
/// dictionary file and the affix file.
pub fn find_dictionary(
    search_path: Vec<&str>,
    locale: &str,
) -> Option<(PathBuf, PathBuf)> {
    for dir in search_path {
        let dir = match expand_dir(&PathBuf::from(dir)) {
            Some(dir) => dir,
            None => {
                eprintln!("Could not expand path {}", dir);
                continue;
            }
        };

        eprint!("Looking for dictionary in {}", dir.display());

        let pdic = dir.join(format!("{}.dic", locale));
        let paff = dir.join(format!("{}.aff", locale));

        if Path::exists(&pdic) && Path::exists(&paff) {
            eprintln!(" ... found");
            return Some((pdic, paff));
        }
        eprintln!();
    }
    None
}

fn load_file(
    pathname: &Path,
    local_dict: Option<&PathBuf>,
    dicts: &mut HashMap<String, Rc<RefCell<dyn Speller>>>,
) -> Result<FileState> {
    let mut contents =
        std::fs::read_to_string(pathname).with_context(|| {
            format!("Could not read file {}", pathname.display())
        })?;
    if contents.starts_with('\u{feff}') {
        contents.remove(0); // Remove BOM
    }

    let locale = locale_from_filename(pathname)?;
    let speller = if dicts.contains_key(locale) {
        dicts[locale].clone()
    } else {
        eprintln!("Using locale {}", locale);
        let mut speller =
            match find_dictionary(DICTIONARY_SEARCH_PATH.to_vec(), locale) {
                Some((dictpath, affixpath)) => {
                    SpellerHunspellDict::new(&dictpath, &affixpath)
                }
                None => Err(anyhow!("Dictionary not found")),
            }?;
        for e in speller.get_errors() {
            eprintln!("Dictionary error: {}", e);
        }
        if let Some(local_dict) = local_dict {
            eprint!("Using local dictionary {} ...", local_dict.display());
            let added = speller.set_user_dict(local_dict)?;
            eprintln!("loaded {} words", added);
        }
        let speller = Rc::new(RefCell::new(speller));
        dicts.insert(locale.to_string(), speller.clone());
        speller
    };

    Ok(FileState::new(pathname, &contents, speller))
}

fn main() -> Result<()> {
    let args = Cli::parse();
    let mut dicts = HashMap::new();
    let mut files = Vec::new();

    for pathname in args.pathnames.iter() {
        match load_file(pathname, args.local_dict.as_ref(), &mut dicts) {
            Ok(file) => files.push(file),
            Err(err) => eprintln!("{:#}", err),
        }
    }
    if files.is_empty() {
        bail!("No files could be spellchecked.");
    }

    let data = AppState::new(Rc::new(files));
    let main_window = WindowDesc::new(ui_builder())
        .title(WINDOW_TITLE.to_owned() + " " + data.file.filename.as_ref())
        .window_size((1000.0, 500.0));
    AppLauncher::with_window(main_window)
        .log_to_console()
        .configure_env(|env, _| {
            env.set(LOC_KEY_COLOR, Color::rgb8(0xff, 0xa5, 0x00));
            env.set(WORD_COLOR, Color::rgb8(0xFF, 0xFF, 0xFF));
            env.set(MISSPELLED_COLOR, Color::rgb8(0xFF, 0x40, 0x40));
            env.set(CODE_COLOR, Color::rgb8(0x60, 0x60, 0xFF));
            env.set(KEYWORD_COLOR, Color::rgb8(0xc0, 0xa0, 0x00));
            env.set(ESCAPE_COLOR, Color::rgb8(0xc0, 0xa0, 0x00));
            env.set(COMMENT_COLOR, Color::rgb8(0xc0, 0xa0, 0x50));
            env.set(MARKUP_COLOR, Color::rgb8(0x80, 0x80, 0xc0));
            env.set(ICON_TAG_COLOR, Color::rgb8(0xff, 0xd7, 0x00));
            env.set(LINE_COLOR, Color::rgba8(0x80, 0x80, 0x80, 0x20));
        })
        .launch(data)
        .with_context(|| "Could not launch application")
}
