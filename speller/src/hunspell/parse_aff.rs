/// Parser for hunspell-format .aff files
use anyhow::{anyhow, Error, Result};
use nom::branch::alt;
use nom::bytes::complete::{tag, take_till1};
use nom::character::complete::{
    anychar, char, i32, line_ending, not_line_ending, space0, space1, u8,
};
use nom::combinator::{cut, eof, map, opt, success, value};
use nom::error::{Error as NomError, ErrorKind, ParseError};
use nom::multi::many0;
use nom::sequence::{delimited, preceded, separated_pair, terminated, tuple};
use nom::{Compare, Err, Finish, IResult, InputLength, Parser};

use crate::hunspell::affixdata::FlagMode;
use crate::hunspell::AffixData;

type Input<'a> = &'a str;

const BYTE_ORDER_MARK: char = '\u{FEFF}';

struct AffError {
    message: String,
}

impl AffError {
    fn new(message: &str) -> Self {
        AffError {
            message: message.to_string(),
        }
    }

    fn wrapped(message: &str) -> Err<Self> {
        Err::Error(Self::new(message))
    }

    fn from_nom(e: Err<NomError<Input>>) -> Err<Self> {
        Err::Error(Self::new(&e.to_string()))
    }
}

impl<'a> ParseError<Input<'a>> for AffError {
    fn from_error_kind(input: Input, kind: ErrorKind) -> Self {
        let message = format!("{:?}:\t{}\n", kind, input);
        AffError { message }
    }

    fn append(_input: Input, _kind: ErrorKind, other: Self) -> Self {
        other
    }
}

impl ToString for AffError {
    fn to_string(&self) -> String {
        self.message.to_string()
    }
}

fn from_anyhow(e: Error) -> Err<AffError> {
    AffError::wrapped(&e.to_string())
}

#[derive(Clone)]
enum AffixLine<'a> {
    Empty,
    SetEncoding(&'a str),
    SetFlagMode(FlagMode),
    SetKeyboardString(&'a str),
    SetTryString(&'a str),
    SetExtraWordString(&'a str),
    SetFlag(&'a str, &'a str),
    SetCompoundMin(u8),
    AddIconv(char, char),
    AddOconv(char, char),
}

/// Parse a line starting with a keyword and then a value.
/// Takes the tag for the keyword, and a parser for the value.
/// Returns the result of the value parser.
fn keyword<'a, T, O, E: ParseError<Input<'a>>, F>(
    key: T,
    mut value: F,
) -> impl FnMut(Input<'a>) -> IResult<Input<'a>, O, E>
where
    F: Parser<Input<'a>, O, E>,
    Input<'a>: Compare<T>,
    T: InputLength + Copy,
{
    move |s: Input<'a>| {
        let (s, _) = tag(key).parse(s)?;
        let (s, _) = space1.parse(s)?;
        // re-implement cut() because I don't know how to pass cut(value)
        // without errors about copying value.
        match value.parse(s) {
            Err(Err::Error(e)) => Err(Err::Failure(e)),
            rest => rest,
        }
    }
}

fn comment(s: &str) -> IResult<&str, ()> {
    value((), preceded(char('#'), not_line_ending))(s)
}

fn ending(s: &str) -> IResult<&str, (), AffError> {
    value((), delimited(space0, opt(comment), line_ending))(s)
        .map_err(AffError::from_nom)
}

fn value_string(s: &str) -> IResult<&str, &str, AffError> {
    take_till1(|c: char| c.is_whitespace())(s)
}

const FLAG_NAMES: [&str; 9] = [
    "COMPOUNDBEGIN",
    "COMPOUNDMIDDLE",
    "COMPOUNDEND",
    "COMPOUNDPERMITFLAG",
    "ONLYINCOMPOUND",
    "NOSUGGEST",
    "CIRCUMFIX",
    "NEEDAFFIX",
    "FORBIDDENWORD",
];

fn assign_flag(s: &str) -> IResult<&str, AffixLine, AffError> {
    let (s, key) = value_string(s)?;
    if !FLAG_NAMES.contains(&key) {
        return Err(AffError::wrapped("Keyword not a known flag"));
    }
    let (s, _) = space1(s)?;
    let (s, v) = cut(value_string)(s)?;
    Ok((s, AffixLine::SetFlag(key, v)))
}

fn set_encoding(s: &str) -> IResult<&str, AffixLine, AffError> {
    map(keyword("SET", value_string), AffixLine::SetEncoding)(s)
}

fn flag_mode(s: &str) -> IResult<&str, FlagMode, AffError> {
    alt((
        value(FlagMode::DoubleCharFlags, tag("long")),
        value(FlagMode::NumericFlags, tag("num")),
        value(FlagMode::Utf8Flags, tag("UTF-8")),
    ))(s)
}

fn set_flag_mode(s: &str) -> IResult<&str, AffixLine, AffError> {
    map(keyword("FLAG", flag_mode), AffixLine::SetFlagMode)(s)
}

fn set_keyboard_string(s: &str) -> IResult<&str, AffixLine, AffError> {
    map(keyword("KEY", value_string), AffixLine::SetKeyboardString)(s)
}

fn set_try_string(s: &str) -> IResult<&str, AffixLine, AffError> {
    map(keyword("TRY", value_string), AffixLine::SetTryString)(s)
}

fn set_extra_word_string(s: &str) -> IResult<&str, AffixLine, AffError> {
    map(
        keyword("WORDCHARS", value_string),
        AffixLine::SetExtraWordString,
    )(s)
}

fn set_compound_min(s: &str) -> IResult<&str, AffixLine, AffError> {
    map(keyword("COMPOUNDMIN", u8), AffixLine::SetCompoundMin)(s)
}

fn conv(s: &str) -> IResult<&str, (char, char), AffError> {
    separated_pair(anychar, space1, anychar)(s)
}

fn add_iconv(s: &str) -> IResult<&str, AffixLine, AffError> {
    alt((
        value(AffixLine::Empty, tuple((tag("ICONV"), space1, i32))),
        map(keyword("ICONV", conv), |(c1, c2)| {
            AffixLine::AddIconv(c1, c2)
        }),
    ))(s)
}

fn add_oconv(s: &str) -> IResult<&str, AffixLine, AffError> {
    alt((
        value(AffixLine::Empty, tuple((tag("OCONV"), space1, i32))),
        map(keyword("OCONV", conv), |(c1, c2)| {
            AffixLine::AddOconv(c1, c2)
        }),
    ))(s)
}

fn line(s: &str) -> IResult<&str, AffixLine, AffError> {
    alt((
        set_encoding,
        set_flag_mode,
        set_keyboard_string,
        set_try_string,
        set_extra_word_string,
        assign_flag,
        set_compound_min,
        add_iconv,
        add_oconv,
        success(AffixLine::Empty),
    ))(s)
}

fn affix_file(s: &str) -> IResult<&str, AffixData, AffError> {
    let (s, _) = opt(char(BYTE_ORDER_MARK)).parse(s)?; // discard BOM

    let mut d = AffixData::new();
    let (s, lines) = many0(terminated(line, ending))(s)?;
    for l in lines.iter() {
        match l {
            AffixLine::Empty => (),
            AffixLine::SetEncoding(enc) => {
                if enc != &"UTF-8" {
                    return Err(AffError::wrapped(&format!(
                        "Unsupported encoding {}",
                        enc
                    )));
                }
            }
            AffixLine::SetFlagMode(fm) => d.flag_mode = *fm,
            AffixLine::SetKeyboardString(k) => {
                d.keyboard_string = Some(k.to_string())
            }
            AffixLine::SetTryString(t) => d.try_string = Some(t.to_string()),
            AffixLine::SetExtraWordString(t) => {
                d.extra_word_string = Some(t.to_string())
            }
            AffixLine::SetFlag(f, v) => {
                let fflag = d.parse_flags(v).map_err(from_anyhow)?;
                if fflag.len() != 1 {
                    return Err(AffError::wrapped(&format!(
                        "Could not parse {}",
                        f
                    )));
                }
                let v = Some(fflag[0]);
                match *f {
                    "FORBIDDEN" => d.forbidden = fflag[0],
                    "COMPOUNDBEGIN" => d.compound_begin = v,
                    "COMPOUNDMIDDLE" => d.compound_middle = v,
                    "COMPOUNDEND" => d.compound_end = v,
                    "COMPOUNDPERMITFLAG" => d.compound_permit = v,
                    "ONLYINCOMPOUND" => d.only_in_compound = v,
                    "NOSUGGEST" => d.no_suggest = v,
                    "CIRCUMFIX" => d.circumfix = v,
                    "NEEDAFFIX" => d.need_affix = v,
                    _ => panic!("Unhandled flag"),
                }
            }
            AffixLine::SetCompoundMin(v) => d.compound_min = *v,
            AffixLine::AddIconv(c1, c2) => {
                d.iconv.insert(*c1, *c2);
            }
            AffixLine::AddOconv(c1, c2) => {
                d.oconv.insert(*c1, *c2);
            }
        };
    }
    let (s, _) = eof(s)?;
    Ok((s, d))
}

pub fn parse_affix_data(text: &str) -> Result<AffixData> {
    match delimited(opt(char(BYTE_ORDER_MARK)), affix_file, eof)
        .parse(text)
        .finish()
    {
        Ok((_, d)) => Ok(d),
        Err(e) => Err(anyhow!(e.to_string())),
    }
}
