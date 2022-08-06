/// Parser for hunspell-format .aff files
use anyhow::{bail, Result};
use nom::branch::alt;
use nom::bytes::complete::{tag, take_till1};
use nom::character::complete::{
    char, one_of, satisfy, space0, space1, u32, u8,
};
use nom::combinator::{all_consuming, cut, map, opt, rest, success, value};
use nom::error::{Error, ErrorKind, ParseError};
use nom::multi::many1;
use nom::sequence::{pair, preceded, separated_pair, terminated};
use nom::{Compare, Err, Finish, IResult, InputLength, Parser};

use crate::hunspell::affixdata::{AffixEntry, FlagMode};
use crate::hunspell::compoundrule::CompoundRule;
use crate::hunspell::wordflags::WordFlags;
use crate::hunspell::AffixData;

type Input<'a> = &'a str;

const BYTE_ORDER_MARK: char = '\u{FEFF}';

#[derive(Clone)]
enum AffixLine<'a> {
    Empty,
    SetEncoding(&'a str),
    SetFlagMode(FlagMode),
    SetKeyboardString(&'a str),
    SetTryString(&'a str),
    SetExtraWordString(&'a str),
    SetFlag(WordFlags, &'a str, &'a str),
    SetCompoundMin(u8),
    SetMaxNGramSuggestions(u8),
    AddIconv((&'a str, &'a str)),
    AddOconv((&'a str, &'a str)),
    AddCompoundRule(&'a str),
    AddRelatedChars(&'a str),
    ClearBreakTable,
    AddWordBreaks(&'a str),
    AddRep((&'a str, &'a str)),
    SetFullstrip,
    SetCheckSharps,
    NextAllowCross(bool),
    AddAffix((bool, &'a str, (&'a str, &'a str, &'a str))),
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
    value((), preceded(char('#'), rest))(s)
}

fn ending(s: &str) -> IResult<&str, ()> {
    value((), preceded(space0, opt(comment)))(s)
}

fn value_string(s: &str) -> IResult<&str, &str> {
    take_till1(|c: char| c.is_whitespace())(s)
}

const FLAG_NAMES: [(&str, WordFlags); 11] = [
    ("FORBIDDENWORD", WordFlags::Forbidden),
    ("COMPOUNDBEGIN", WordFlags::CompoundBegin),
    ("COMPOUNDMIDDLE", WordFlags::CompoundMiddle),
    ("COMPOUNDEND", WordFlags::CompoundEnd),
    ("COMPOUNDPERMITFLAG", WordFlags::CompoundPermit),
    ("ONLYINCOMPOUND", WordFlags::OnlyInCompound),
    ("NOSUGGEST", WordFlags::NoSuggest),
    ("CIRCUMFIX", WordFlags::Circumfix),
    ("NEEDAFFIX", WordFlags::NeedAffix),
    ("PSEUDOROOT", WordFlags::NeedAffix), // backwards compat for NEEDAFFIX
    ("KEEPCASE", WordFlags::KeepCase),
];

fn assign_flag(s: &str) -> IResult<&str, AffixLine> {
    let (s, key) = value_string(s)?;
    let mut wflag = WordFlags::empty();
    for (name, flag) in FLAG_NAMES {
        if name == key {
            wflag = flag;
            break;
        }
    }
    if wflag == WordFlags::empty() {
        return Err(Err::Error(Error::from_error_kind(key, ErrorKind::Tag)));
    }
    let (s, _) = space1(s)?;
    let (s, v) = cut(value_string)(s)?;
    Ok((s, AffixLine::SetFlag(wflag, key, v)))
}

fn set_encoding(s: &str) -> IResult<&str, AffixLine> {
    map(keyword("SET", value_string), AffixLine::SetEncoding)(s)
}

fn flag_mode(s: &str) -> IResult<&str, FlagMode> {
    alt((
        value(FlagMode::DoubleChar, tag("long")),
        value(FlagMode::Numeric, tag("num")),
        value(FlagMode::Utf8, tag("UTF-8")),
    ))(s)
}

fn set_flag_mode(s: &str) -> IResult<&str, AffixLine> {
    map(keyword("FLAG", flag_mode), AffixLine::SetFlagMode)(s)
}

fn set_keyboard_string(s: &str) -> IResult<&str, AffixLine> {
    map(keyword("KEY", value_string), AffixLine::SetKeyboardString)(s)
}

fn set_try_string(s: &str) -> IResult<&str, AffixLine> {
    map(keyword("TRY", value_string), AffixLine::SetTryString)(s)
}

fn set_extra_word_string(s: &str) -> IResult<&str, AffixLine> {
    map(
        keyword("WORDCHARS", value_string),
        AffixLine::SetExtraWordString,
    )(s)
}

fn set_compound_min(s: &str) -> IResult<&str, AffixLine> {
    map(keyword("COMPOUNDMIN", u8), AffixLine::SetCompoundMin)(s)
}

fn set_max_ngram_suggestions(s: &str) -> IResult<&str, AffixLine> {
    map(
        keyword("MAXNGRAMSUGS", u8),
        AffixLine::SetMaxNGramSuggestions,
    )(s)
}

fn conv(s: &str) -> IResult<&str, (&str, &str)> {
    separated_pair(value_string, space1, value_string)(s)
}

fn add_iconv(s: &str) -> IResult<&str, AffixLine> {
    table_line("ICONV", conv, AffixLine::AddIconv)(s)
}

fn add_oconv(s: &str) -> IResult<&str, AffixLine> {
    table_line("OCONV", conv, AffixLine::AddOconv)(s)
}

fn add_compound_rule(s: &str) -> IResult<&str, AffixLine> {
    table_line("COMPOUNDRULE", value_string, AffixLine::AddCompoundRule)(s)
}

fn add_related_chars(s: &str) -> IResult<&str, AffixLine> {
    table_line("MAP", value_string, AffixLine::AddRelatedChars)(s)
}

fn add_word_breaks(s: &str) -> IResult<&str, AffixLine> {
    map(
        table_line("BREAK", value_string, AffixLine::AddWordBreaks),
        |al| match al {
            AffixLine::Empty => AffixLine::ClearBreakTable,
            _ => al,
        },
    )(s)
}

fn add_rep(s: &str) -> IResult<&str, AffixLine> {
    table_line("REP", conv, AffixLine::AddRep)(s)
}

fn set_fullstrip(s: &str) -> IResult<&str, AffixLine> {
    value(AffixLine::SetFullstrip, tag("FULLSTRIP"))(s)
}

fn set_checksharps(s: &str) -> IResult<&str, AffixLine> {
    value(AffixLine::SetCheckSharps, tag("CHECKSHARPS"))(s)
}

fn morph_id(s: Input) -> IResult<Input, ()> {
    value(
        (),
        pair(
            satisfy(|c| c.is_alphabetic()),
            satisfy(|c| c.is_alphabetic()),
        ),
    )(s)
}

fn morph_flag(s: Input) -> IResult<Input, ()> {
    value((), separated_pair(morph_id, char(':'), value_string))(s)
}

fn morph_flags(s: Input) -> IResult<Input, ()> {
    alt((
        value((), many1(preceded(space1, morph_flag))),
        // The morph flag space can also contain stuff like "<ZERO>>"
        value((), preceded(space1, value_string)),
        value((), space0),
    ))(s)
}

fn affix_entry(s: &str) -> IResult<&str, (&str, &str, &str)> {
    map(
        separated_pair(
            separated_pair(value_string, space1, value_string),
            space1,
            value_string,
        ),
        |((v1, v2), v3)| (v1, v2, v3),
    )(s)
}

fn affix_line(s: Input) -> IResult<Input, (&str, &str, &str)> {
    terminated(affix_entry, morph_flags)(s)
}

fn add_affix<'a, T>(
    key: T,
    is_pfx: bool,
) -> impl FnMut(Input<'a>) -> IResult<Input<'a>, AffixLine<'a>>
where
    Input<'a>: Compare<T>,
    T: InputLength + Copy,
{
    move |s: Input<'a>| {
        let (s, _) = tag(key)(s)?;
        let (s, _) = space1(s)?;
        let (s, flag) = value_string(s)?;
        let (s, _) = space1(s)?;
        if let Ok((s, entry)) = affix_line(s) {
            Ok((s, AffixLine::AddAffix((is_pfx, flag, entry))))
        } else {
            // check if it's a valid first line
            let (s, yn) = one_of("YN")(s)?;
            let (s, _) = space1(s)?;
            let (s, _) = u32(s)?;
            Ok((s, AffixLine::NextAllowCross(yn == 'Y')))
        }
    }
}

fn line(s: &str) -> IResult<&str, AffixLine> {
    alt((
        set_encoding,
        set_flag_mode,
        set_keyboard_string,
        set_try_string,
        set_extra_word_string,
        assign_flag,
        set_compound_min,
        set_max_ngram_suggestions,
        add_iconv,
        add_oconv,
        add_compound_rule,
        add_related_chars,
        add_word_breaks,
        add_rep,
        set_fullstrip,
        set_checksharps,
        add_affix("PFX", true),
        add_affix("SFX", false),
        success(AffixLine::Empty),
    ))(s)
}

fn full_line(s: &str) -> IResult<&str, AffixLine> {
    all_consuming(terminated(line, ending))(s)
}

pub fn parse_affix_data(s: &str) -> Result<AffixData> {
    let s = s.trim_start_matches(BYTE_ORDER_MARK);

    let mut d = AffixData::new();
    let mut allow_cross = false;

    // default break table
    d.word_breaks.push("-".to_string());
    d.word_breaks.push("^-".to_string());
    d.word_breaks.push("-$".to_string());

    for l in s.lines() {
        let (_, afline) = full_line.parse(l).finish().unwrap_or_else(|e| {
            d.errors.push(e.to_string());
            ("", AffixLine::Empty)
        });
        match afline {
            AffixLine::Empty => (),
            AffixLine::SetEncoding(enc) => {
                if enc != "UTF-8" {
                    bail!(format!("Unsupported encoding {}", enc));
                }
            }
            AffixLine::SetFlagMode(fm) => d.flag_mode = fm,
            AffixLine::SetKeyboardString(k) => {
                d.keyboard_string = Some(k.to_string())
            }
            AffixLine::SetTryString(t) => d.try_string = Some(t.to_string()),
            AffixLine::SetExtraWordString(t) => {
                d.extra_word_string = Some(t.to_string())
            }
            AffixLine::SetFlag(wflag, key, v) => {
                let fflag = d.parse_flags(v)?;
                if fflag.len() != 1 {
                    bail!(format!("Could not parse {}", key));
                }
                d.special_flags.insert(wflag, fflag[0]);
            }
            AffixLine::SetCompoundMin(v) => d.compound_min = v,
            AffixLine::SetMaxNGramSuggestions(v) => d.max_ngram_suggestions = v,
            AffixLine::AddIconv((c1, c2)) => {
                d.iconv.push(c1, c2);
            }
            AffixLine::AddOconv((c1, c2)) => {
                d.oconv.push(c1, c2);
            }
            AffixLine::AddCompoundRule(v) => {
                d.compound_rules.push(CompoundRule::from_str(v, &d)?);
            }
            AffixLine::AddRelatedChars(v) => {
                d.related_chars.push(v.to_string());
            }
            AffixLine::ClearBreakTable => d.word_breaks.clear(),
            AffixLine::AddWordBreaks(v) => {
                d.word_breaks.push(v.to_string());
            }
            AffixLine::AddRep((f, t)) => {
                let f = f.replace('_', " ");
                let t = t.replace('_', " ");
                d.replacements.push(&f, &t);
            }
            AffixLine::SetFullstrip => d.fullstrip = true,
            AffixLine::SetCheckSharps => d.check_sharps = true,
            AffixLine::NextAllowCross(yn) => allow_cross = yn,
            AffixLine::AddAffix((is_pfx, k, (mut v1, mut v2, mut v3))) => {
                let fflag = d.parse_flags(k)?;
                if fflag.len() != 1 {
                    bail!("Could not parse PFX");
                }
                if v1 == "0" {
                    v1 = "";
                }
                let mut cflags = Vec::new();
                if let Some((nv2, contflags)) = v2.split_once('/') {
                    v2 = nv2;
                    cflags = d.parse_flags(contflags)?;
                }
                if v2 == "0" {
                    v2 = "";
                }
                if v3 == "." {
                    v3 = "";
                }
                let entry =
                    AffixEntry::new(allow_cross, fflag[0], v1, v2, v3, cflags);
                if is_pfx {
                    d.prefixes.push(entry);
                } else {
                    d.suffixes.push(entry);
                }
            }
        };
    }
    d.finalize();
    Ok(d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rep_unanchored() {
        let s = "REP eau o";
        let d = parse_affix_data(s).unwrap();
        assert_eq!("mow", d.replacements.conv("meauw"));
    }

    #[test]
    fn rep_anchored() {
        let s = "REP ^l l'\nREP ies$ isch";
        let d = parse_affix_data(s).unwrap();

        assert_eq!("l'eau", d.replacements.conv("leau"));
        assert_eq!("l'ol", d.replacements.conv("lol"));
        assert_eq!("oil", d.replacements.conv("oil"));
        assert_eq!("l'", d.replacements.conv("l"));

        assert_eq!("visch", d.replacements.conv("vies"));
        assert_eq!("vieslijk", d.replacements.conv("vieslijk"));
        assert_eq!("isch", d.replacements.conv("ies"));
    }

    #[test]
    fn rep_with_spaces() {
        let s = "REP alot a_lot";
        let d = parse_affix_data(s).unwrap();
        assert_eq!("a lot", d.replacements.conv("alot"));
    }
}
