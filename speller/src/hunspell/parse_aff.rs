/// Parser for hunspell-format .aff files
use anyhow::{anyhow, Error, Result};
use nom::branch::alt;
use nom::bytes::complete::{tag, take_till1};
use nom::character::complete::{
    char, line_ending, not_line_ending, space0, space1,
};
use nom::combinator::{eof, map, opt, success, value};
use nom::error::{Error as NomError, ErrorKind, ParseError};
use nom::multi::many0;
use nom::sequence::{delimited, preceded, terminated};
use nom::{Compare, Err, Finish, IResult, InputLength, Parser};

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

    fn append(input: Input, kind: ErrorKind, other: Self) -> Self {
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
    SetForbidden(&'a str),
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

fn set_encoding_line(s: &str) -> IResult<&str, AffixLine, AffError> {
    map(keyword("SET", value_string), AffixLine::SetEncoding)(s)
}

fn set_forbidden_line(s: &str) -> IResult<&str, AffixLine, AffError> {
    map(
        keyword("FORBIDDENWORD", value_string),
        AffixLine::SetForbidden,
    )(s)
}

fn line(s: &str) -> IResult<&str, AffixLine, AffError> {
    alt((
        set_encoding_line,
        set_forbidden_line,
        success(AffixLine::Empty),
    ))(s)
}

fn affix_file(s: &str) -> IResult<&str, AffixData, AffError> {
    let (s, _) = opt(char(BYTE_ORDER_MARK)).parse(s)?; // discard BOM

    let mut d = AffixData::new();
    let mut forbidden_seen = false;
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
            AffixLine::SetForbidden(f) => {
                let fflag = d.parse_flags(f).map_err(from_anyhow)?;
                if fflag.len() != 1 || forbidden_seen {
                    return Err(AffError::wrapped(
                        "Found multiple flags for FORBIDDENWORD",
                    ));
                }
                d.forbidden = fflag[0];
                forbidden_seen = true;
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
