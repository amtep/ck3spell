/// Parser for hunspell-format .aff files
use anyhow::{anyhow, Error, Result};
use nom::branch::alt;
use nom::bytes::complete::{tag, take_till1};
use nom::character::complete::{
    anychar, char, line_ending, not_line_ending, one_of, space0, space1, u32,
    u8,
};
use nom::combinator::{cut, eof, map, opt, success, value};
use nom::error::{Error as NomError, ErrorKind, ParseError};
use nom::multi::many0;
use nom::sequence::{delimited, preceded, separated_pair, terminated};
use nom::{Compare, Err, Finish, IResult, InputLength, Parser};

use crate::hunspell::affixdata::{AffixEntry, FlagMode};
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
    AddIconv((char, char)),
    AddOconv((char, char)),
    AddCompoundRule(&'a str),
    AddRelatedChars(&'a str),
    AddWordBreaks(&'a str),
    SetFullstrip,
    SetCheckSharps,
    NextAllowCross(bool),
    AddPrefix((&'a str, (&'a str, &'a str, &'a str))),
    AddSuffix((&'a str, (&'a str, &'a str, &'a str))),
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

/// Parse a line that is a table entry. Each line of a table
/// starts with the same keyword. The first word contains the
/// number of entries that follow, which we ignore.
///
/// Takes the tag for the keyword, a parser for the value, and the
/// `AffixLine` type to convert the value to.
/// Returns `AffixLine::Empty` for the first line, and the given
/// `AffixLine` type for the following lines.
fn table_line<'a, T, O, E: ParseError<Input<'a>>>(
    key: T,
    mut value: impl Parser<Input<'a>, O, E>,
    conv: impl Fn(O) -> AffixLine<'a>,
) -> impl FnMut(Input<'a>) -> IResult<Input<'a>, AffixLine<'a>, E>
where
    Input<'a>: Compare<T>,
    T: InputLength + Copy,
{
    move |s: Input<'a>| {
        let (s, _) = tag(key).parse(s)?;
        let (s, _) = space1.parse(s)?;
        if let Ok((s, _)) = u32::<Input<'a>, E>(s) {
            return Ok((s, AffixLine::Empty));
        }
        // re-implement cut() because I don't know how to pass cut(value)
        // without errors about copying value.
        match value.parse(s) {
            Err(Err::Error(e)) => Err(Err::Failure(e)),
            Ok((s, v)) => Ok((s, conv(v))),
            Err(other) => Err(other),
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

const FLAG_NAMES: [&str; 10] = [
    "FORBIDDENWORD",
    "COMPOUNDBEGIN",
    "COMPOUNDMIDDLE",
    "COMPOUNDEND",
    "COMPOUNDPERMITFLAG",
    "ONLYINCOMPOUND",
    "NOSUGGEST",
    "CIRCUMFIX",
    "NEEDAFFIX",
    "KEEPCASE",
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
    table_line("ICONV", conv, AffixLine::AddIconv)(s)
}

fn add_oconv(s: &str) -> IResult<&str, AffixLine, AffError> {
    table_line("OCONV", conv, AffixLine::AddOconv)(s)
}

fn add_compound_rule(s: &str) -> IResult<&str, AffixLine, AffError> {
    table_line("COMPOUNDRULE", value_string, AffixLine::AddCompoundRule)(s)
}

fn add_related_chars(s: &str) -> IResult<&str, AffixLine, AffError> {
    table_line("MAP", value_string, AffixLine::AddRelatedChars)(s)
}

fn add_word_breaks(s: &str) -> IResult<&str, AffixLine, AffError> {
    table_line("BREAK", value_string, AffixLine::AddWordBreaks)(s)
}

fn set_fullstrip(s: &str) -> IResult<&str, AffixLine, AffError> {
    value(AffixLine::SetFullstrip, tag("FULLSTRIP"))(s)
}

fn set_checksharps(s: &str) -> IResult<&str, AffixLine, AffError> {
    value(AffixLine::SetCheckSharps, tag("CHECKSHARPS"))(s)
}

fn affix_entry(s: &str) -> IResult<&str, (&str, &str, &str), AffError> {
    map(
        separated_pair(
            separated_pair(value_string, space1, value_string),
            space1,
            value_string,
        ),
        |((v1, v2), v3)| (v1, v2, v3),
    )(s)
}

fn add_affix<'a, T>(
    key: T,
    conv: impl Fn((&'a str, (&'a str, &'a str, &'a str))) -> AffixLine<'a>,
) -> impl FnMut(Input<'a>) -> IResult<Input<'a>, AffixLine<'a>, AffError>
where
    Input<'a>: Compare<T>,
    T: InputLength + Copy,
{
    move |s: Input<'a>| {
        let (s, _) = tag(key)(s)?;
        let (s, _) = space1(s)?;
        let (s, flag) = value_string(s)?;
        let (s, _) = space1(s)?;
        if let Ok((s, entry)) = affix_entry(s) {
            Ok((s, conv((flag, entry))))
        } else {
            // check if it's a valid first line
            let (s, yn) = one_of("YN")(s)?;
            let (s, _) = space1(s)?;
            let (s, _) = u32(s)?;
            Ok((s, AffixLine::NextAllowCross(yn == 'Y')))
        }
    }
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
        add_compound_rule,
        add_related_chars,
        add_word_breaks,
        set_fullstrip,
        set_checksharps,
        add_affix("PFX", AffixLine::AddPrefix),
        add_affix("SFX", AffixLine::AddSuffix),
        success(AffixLine::Empty),
    ))(s)
}

fn affix_file(s: &str) -> IResult<&str, AffixData, AffError> {
    let (s, _) = opt(char(BYTE_ORDER_MARK)).parse(s)?; // discard BOM

    let mut d = AffixData::new();
    let (s, lines) = many0(terminated(line, ending))(s)?;
    let mut allow_cross = false;
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
                    "KEEPCASE" => d.keep_case = v,
                    _ => panic!("Unhandled flag"),
                }
            }
            AffixLine::SetCompoundMin(v) => d.compound_min = *v,
            AffixLine::AddIconv((c1, c2)) => {
                d.iconv.insert(*c1, *c2);
            }
            AffixLine::AddOconv((c1, c2)) => {
                d.oconv.insert(*c1, *c2);
            }
            AffixLine::AddCompoundRule(v) => {
                d.compound_rules
                    .push(d.parse_flags(v).map_err(from_anyhow)?);
            }
            AffixLine::AddRelatedChars(v) => {
                d.related_chars.push(v.to_string());
            }
            AffixLine::AddWordBreaks(v) => {
                d.word_breaks.push(v.to_string());
            }
            AffixLine::SetFullstrip => d.fullstrip = true,
            AffixLine::SetCheckSharps => d.check_sharps = true,
            AffixLine::NextAllowCross(yn) => allow_cross = *yn,
            AffixLine::AddPrefix((k, (v1, v2, v3))) => {
                let entry = AffixEntry::new(allow_cross, v1, v2, v3);
                let fflag = d.parse_flags(k).map_err(from_anyhow)?;
                if fflag.len() != 1 {
                    return Err(AffError::wrapped("Could not parse PFX"));
                }
                d.prefixes.entry(fflag[0]).or_default().push(entry);
            }
            AffixLine::AddSuffix((k, (v1, v2, v3))) => {
                let entry = AffixEntry::new(allow_cross, v1, v2, v3);
                let fflag = d.parse_flags(k).map_err(from_anyhow)?;
                if fflag.len() != 1 {
                    return Err(AffError::wrapped("Could not parse SFX"));
                }
                d.suffixes.entry(fflag[0]).or_default().push(entry);
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
