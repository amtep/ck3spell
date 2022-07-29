/// Parser for hunspell-format .aff files
use anyhow::{anyhow, Result};
use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::character::complete::{char, line_ending, space1};
use nom::combinator::{eof, map, opt};
use nom::error::ParseError;
use nom::multi::many0;
use nom::sequence::{delimited, terminated};
use nom::{Compare, Err, Finish, InputLength, IResult, Parser};

type Input<'a> = &'a str;

const BYTE_ORDER_MARK: char = '\u{FEFF}';

pub struct AffixData {
    
}

enum AffixLine<'a> {
    SetEncoding(&'a str),
}

/// Parse a line starting with a keyword and then a value.
/// Takes the tag for the keyword, and a parser for the value.
/// Returns the result of the value parser.
fn keyword<'a, T, O, E: ParseError<Input<'a>>, F>(key: T, mut value: F) ->
    impl FnMut(Input<'a>) -> IResult<Input<'a>, O, E>
    where F: Parser<Input<'a>, O, E>,
          Input<'a>: Compare<T>,
          T: InputLength + Copy {
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

fn set_encoding_line(s: &str) -> IResult<&str, AffixLine> {
    map(keyword("SET", tag("UTF-8")), AffixLine::SetEncoding)(s)
}

fn line(s: &str) -> IResult<&str, AffixLine> {
    alt((
        set_encoding_line,
    ))(s)
}

fn affix_file(s: &str) -> IResult<&str, AffixData> {
    let (s, _) = opt(char(BYTE_ORDER_MARK)).parse(s)?; // discard BOM

    let mut d = AffixData { };
    let (s, lines) = many0(terminated(line, line_ending))(s)?;
    for l in lines.iter() {
        match l {
            AffixLine::SetEncoding(enc) => (), // Only UTF-8 is accepted anyway
        };
    }
    let (s, _) = eof(s)?;
    Ok((s, d))
}

pub fn parse_affix_data(text: &str) -> Result<AffixData> {
    match delimited(opt(char(BYTE_ORDER_MARK)),
                   affix_file,
                   eof).parse(text).finish() {
        Ok((_, d)) => Ok(d),
        Err(e) => Err(anyhow!(e.to_string()))
    }
}
