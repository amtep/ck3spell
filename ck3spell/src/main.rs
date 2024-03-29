use anyhow::{anyhow, bail, Context, Result};
use clap::Parser;
use druid::text::{Attribute, RichText};
use druid::widget::prelude::*;
use druid::{AppLauncher, Color, Key, Lens, WindowDesc};
use home::home_dir;
use nu_glob::glob;
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
mod custom;
mod edit;
mod editorcontroller;
mod linelist;
mod linescroller;
mod syntax;
mod syntaxhighlighter;
mod ui;

use crate::custom::CustomEndings;
use crate::syntax::{parse_line, TokenType};
use crate::ui::ui_builder;

#[derive(Parser)]
#[clap(author, version, about)]
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
const CUSTOM_COLOR: Key<Color> = Key::new("ck3spell.custom-color");
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
    // For highlighting
    bad_words_range: Rc<Vec<Range<usize>>>,
    // For spellchecking and for displaying the word. Usually the same as the highlighted range,
    // but can differ when custom endings are used.
    bad_words_text: Rc<Vec<String>>,
    highlight_word_nr: usize,
    speller: Rc<RefCell<dyn Speller>>, // Should be in Env but can't.
    custom: Rc<CustomEndings>,         // Should be in Env but can't.
}

impl LineInfo {
    fn highlight(&mut self, env: &Env) {
        highlight_syntax(self, env);
        if let Some(range) = self.marked_word() {
            self.rendered
                .add_attribute(range, Attribute::underline(true));
        }
    }

    fn marked_word(&self) -> Option<Range<usize>> {
        if self.highlight_word_nr > 0 {
            self.bad_words_range
                .get(self.highlight_word_nr - 1)
                .cloned()
        } else {
            None
        }
    }
}

/// Current highlighted bad word, as 1-based line and word number.
/// If the word number is 0 then no word is highlighted.
#[derive(Clone, Copy, Data, Debug)]
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
    custom: Rc<CustomEndings>,
}

impl FileState {
    fn new(
        pathname: &Path,
        contents: &str,
        speller: Rc<RefCell<dyn Speller>>,
        custom: Rc<CustomEndings>,
    ) -> Self {
        let filename = if let Some(name) = pathname.file_name() {
            name.to_string_lossy().to_string()
        } else {
            "".to_string()
        };
        FileState {
            pathname: Rc::new(pathname.to_path_buf()),
            filename: Rc::new(filename),
            lines: Arc::new(split_lines(contents, &speller, &custom)),
            speller,
            custom,
        }
    }

    fn save(&self) -> Result<()> {
        let mut file = File::create(&*self.pathname)
            .with_context(|| format!("Could not write to {}", self.pathname.display()))?;
        file.write_all("\u{FEFF}".as_bytes())?; // Unicode BOM
        for lineinfo in self.lines.iter() {
            file.write_all(lineinfo.line.line.as_bytes())?;
            file.write_all(lineinfo.line.line_end.to_str().as_bytes())?;
        }
        Ok(())
    }

    fn is_clean(&self) -> bool {
        self.lines
            .iter()
            .all(|lineinfo| lineinfo.bad_words_range.is_empty())
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
                let nwords = self.file.lines[cursor.linenr - 1].bad_words_range.len();
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
        let nwords = self.file.lines[cursor.linenr - 1].bad_words_range.len();
        let nlines = self.file.lines.len();
        if cursor.wordnr < nwords {
            cursor.wordnr += 1;
        } else {
            cursor.wordnr = 0;
            while cursor.linenr < nlines {
                cursor.linenr += 1;
                let nwords = self.file.lines[cursor.linenr - 1].bad_words_range.len();
                if nwords > 0 {
                    cursor.wordnr = 1;
                    break;
                }
            }
        }
        self.update_cursor(cursor);
        self.update_suggestions();
    }

    fn cursor_word(&self) -> Option<&String> {
        if self.cursor.wordnr == 0 {
            return None;
        }
        self.file.lines[self.cursor.linenr - 1]
            .bad_words_text
            .get(self.cursor.wordnr - 1)
    }

    // If the cursor word is from a WordPart + Custom, then the Custom part is fixed
    // and can't be changed by suggestions. This is a helper function for dealing with that.
    fn cursor_word_fixed_suffix(&self) -> Option<String> {
        if let Some(word) = self.cursor_word() {
            // These indexes are safe because cursor_word() succeeded so there's a word there.
            let lineinfo = &self.file.lines[self.cursor.linenr - 1];
            let range = &lineinfo.bad_words_range[self.cursor.wordnr - 1];
            let wordpart = &lineinfo.line.line[range.clone()];
            if let Some(suffix) = word.strip_prefix(wordpart) {
                if !suffix.is_empty() {
                    return Some(suffix.to_string());
                }
            }
        }
        None
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
            let opt_suffix = self.cursor_word_fixed_suffix();
            Arc::new(
                self.file
                    .speller
                    .borrow()
                    .suggestions(word, 9)
                    .iter()
                    .filter(|s| {
                        if let Some(suffix) = &opt_suffix {
                            s.ends_with(suffix)
                        } else {
                            true
                        }
                    })
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

const LANGUAGES: [(&str, &str, &str); 9] = [
    ("l_english", "en_US", "English"),
    ("l_german", "de_DE", "German"),
    ("l_french", "fr_FR", "French"),
    ("l_spanish", "es_ES", "Spanish"),
    ("l_russian", "ru_RU", "Russian"),
    ("l_korean", "", "Korean"),
    ("l_simp_chinese", "", "Chinese"),
    ("l_braz_por", "pt_BR", "Portuguese"), // for Stellaris
    ("l_polish", "pl_PL", "Polish"),       // for Stellaris
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

fn highlight_syntax(lineinfo: &mut LineInfo, env: &Env) {
    let line = &lineinfo.line.line;
    let mut text = RichText::new((*line.as_str()).into());
    let mut bad_words_range = Vec::new();
    let mut bad_words_text = Vec::new();

    let tokens = parse_line(line);
    for i in 0..tokens.len() {
        let token = &tokens[i];

        let mut color = match token.ttype {
            TokenType::Comment => env.get(COMMENT_COLOR),
            TokenType::LocKey => env.get(LOC_KEY_COLOR),
            TokenType::KeyReference => env.get(KEYWORD_COLOR),
            TokenType::Word => env.get(WORD_COLOR),
            TokenType::WordPart => env.get(WORD_COLOR),
            TokenType::Escape => env.get(ESCAPE_COLOR),
            TokenType::Code => env.get(CODE_COLOR),
            TokenType::Custom => env.get(CUSTOM_COLOR),
            TokenType::Markup => env.get(MARKUP_COLOR),
            TokenType::IconTag => env.get(ICON_TAG_COLOR),
        };

        if let TokenType::WordPart = token.ttype {
            // Look for a sequence WordPart, Code, Custom, Code (the last Code
            // is not checked), where the WordPart directly borders the Code.
            // For example: meilleur[bg_opponent.Custom('FR_E')]
            //              ^^^^^^^^ WordPart            ^^^^ Custom
            if i + 2 < tokens.len()
                && tokens[i + 2].ttype == TokenType::Custom
                && tokens[i + 1].ttype == TokenType::Code
                && token.range.end == tokens[i + 1].range.start
            {
                let custom = &line[tokens[i + 2].range.clone()];
                if let Some(endings) = lineinfo.custom.check(custom) {
                    for ending in endings {
                        let word = line[token.range.clone()].to_string() + ending;
                        if !lineinfo.speller.borrow().spellcheck(&word) {
                            color = env.get(MISSPELLED_COLOR);
                            bad_words_range.push(token.range.clone());
                            bad_words_text.push(word);
                            break;
                        }
                    }
                }
            }
        } else if let TokenType::Word = token.ttype {
            let word = &line[token.range.clone()];
            if word.chars().count() > 1 && !lineinfo.speller.borrow().spellcheck(word) {
                color = env.get(MISSPELLED_COLOR);
                bad_words_range.push(token.range.clone());
                bad_words_text.push(lineinfo.line.line[token.range.clone()].to_string())
            }
        }

        text.add_attribute(token.range.clone(), Attribute::text_color(color));
    }
    lineinfo.rendered = text;
    lineinfo.bad_words_range = Rc::new(bad_words_range);
    lineinfo.bad_words_text = Rc::new(bad_words_text);
}

fn split_lines(
    contents: &str,
    speller: &Rc<RefCell<dyn Speller>>,
    custom: &Rc<CustomEndings>,
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
            bad_words_range: Rc::new(Vec::new()),
            bad_words_text: Rc::new(Vec::new()),
            highlight_word_nr: 0,
            speller: Rc::clone(speller),
            custom: Rc::clone(custom),
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
pub fn find_dictionary(search_path: Vec<&str>, locale: &str) -> Option<(PathBuf, PathBuf)> {
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
    customs: &mut HashMap<String, Rc<CustomEndings>>,
) -> Result<FileState> {
    let mut contents = std::fs::read_to_string(pathname)
        .with_context(|| format!("Could not read file {}", pathname.display()))?;
    if contents.starts_with('\u{feff}') {
        contents.remove(0); // Remove BOM
    }

    let locale = locale_from_filename(pathname)?;
    let speller = if dicts.contains_key(locale) {
        dicts[locale].clone()
    } else {
        eprintln!("Using locale {}", locale);
        let mut speller = match find_dictionary(DICTIONARY_SEARCH_PATH.to_vec(), locale) {
            Some((dictpath, affixpath)) => SpellerHunspellDict::new(&dictpath, &affixpath),
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

    if !customs.contains_key(locale) {
        customs.insert(locale.to_string(), Rc::new(CustomEndings::new(locale)));
    }
    let custom = customs[locale].clone();

    Ok(FileState::new(pathname, &contents, speller, custom))
}

fn main() -> Result<()> {
    let args = Cli::parse();
    let mut dicts = HashMap::new();
    let mut customs = HashMap::new();
    let mut files = Vec::new();

    // Heuristic. Does the shell that invoked us do its own globbing?
    // Windows Powershell and CMD don't glob, and they also don't set SHELL.
    let needs_glob = std::env::var_os("SHELL").is_none();
    let local_dict = args.local_dict.as_ref();

    for pathname in args.pathnames.iter() {
        if needs_glob {
            for entry in
                glob(&pathname.to_string_lossy()).expect("could not understand filename pattern")
            {
                match entry {
                    Ok(path) => match load_file(&path, local_dict, &mut dicts, &mut customs) {
                        Ok(file) => files.push(file),
                        Err(err) => eprintln!("{:#}", err),
                    },
                    Err(err) => eprintln!("{:#}", err),
                }
            }
        } else {
            match load_file(pathname, local_dict, &mut dicts, &mut customs) {
                Ok(file) => files.push(file),
                Err(err) => eprintln!("{:#}", err),
            }
        }
    }
    if files.is_empty() {
        bail!("No files could be spellchecked.");
    }

    let data = AppState::new(Rc::new(files));
    let main_window = WindowDesc::new(ui_builder())
        .title(|data: &AppState, _: &Env| {
            format!("{} {}", WINDOW_TITLE, data.file.filename.as_ref())
        })
        .window_size((1000.0, 500.0));
    AppLauncher::with_window(main_window)
        .log_to_console()
        .configure_env(|env, _| {
            env.set(LOC_KEY_COLOR, Color::rgb8(0xff, 0xa5, 0x00));
            env.set(WORD_COLOR, Color::rgb8(0xFF, 0xFF, 0xFF));
            env.set(MISSPELLED_COLOR, Color::rgb8(0xFF, 0x40, 0x40));
            env.set(CODE_COLOR, Color::rgb8(0x60, 0x60, 0xFF));
            env.set(CUSTOM_COLOR, Color::rgb8(0x80, 0x80, 0xFF));
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
