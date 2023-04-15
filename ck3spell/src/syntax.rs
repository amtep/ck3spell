use nom::branch::alt;
use nom::bytes::complete::{is_not, tag, take_until, take_while, take_while1};
use nom::character::complete::{
    alpha1, alphanumeric1, anychar, char, digit0, none_of, one_of, satisfy, space0,
};
use nom::combinator::{eof, map, not, opt, peek, recognize, rest};
use nom::multi::{fold_many0, many0_count, many1_count, separated_list1};
use nom::sequence::{delimited, pair, preceded, separated_pair, terminated, tuple};
use nom::{Finish, IResult};
use nom_locate::{position, LocatedSpan};
use std::fmt::Debug;
use std::ops::Range;
use unicode_categories::UnicodeCategories;

type Span<'a> = LocatedSpan<&'a str>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenType {
    Comment,
    LocKey,
    KeyReference,
    Word,
    WordPart,
    Escape,
    Code,
    Custom,
    Markup,
    IconTag,
}

#[derive(Clone, Debug)]
pub struct Token {
    pub ttype: TokenType,
    pub range: Range<usize>,
}

fn token<'a, F: 'a, O>(
    ttype: TokenType,
    mut inner: F,
) -> impl FnMut(Span<'a>) -> IResult<Span<'a>, Vec<Token>>
where
    F: FnMut(Span<'a>) -> IResult<Span<'a>, O>,
{
    move |s: Span| {
        let (s, start) = position(s)?;
        let (s, _) = inner(s)?;
        let (s, end) = position(s)?;
        let token = Token {
            ttype,
            range: start.location_offset()..end.location_offset(),
        };
        Ok((s, vec![token]))
    }
}

fn no_token<'a, F: 'a, O>(mut inner: F) -> impl FnMut(Span<'a>) -> IResult<Span<'a>, Vec<Token>>
where
    F: FnMut(Span<'a>) -> IResult<Span<'a>, O>,
{
    move |s: Span| {
        let (s, _) = inner(s)?;
        Ok((s, Vec::new()))
    }
}

#[allow(dead_code)]
fn log<'a, F: 'a, O>(mut inner: F) -> impl FnMut(Span<'a>) -> IResult<Span<'a>, O>
where
    F: FnMut(Span<'a>) -> IResult<Span<'a>, O>,
    O: Debug,
{
    move |s: Span| {
        let r = inner(s);
        eprintln!("Trace: {:?}", r);
        r
    }
}

fn vec_add<T>(mut v1: Vec<T>, mut v2: Vec<T>) -> Vec<T> {
    v1.append(&mut v2);
    v1
}

fn vec_pair<T>((v1, v2): (Vec<T>, Vec<T>)) -> Vec<T> {
    vec_add(v1, v2)
}

fn vec_triple<T>((v1, v2, v3): (Vec<T>, Vec<T>, Vec<T>)) -> Vec<T> {
    vec_add(vec_add(v1, v2), v3)
}

fn comment(s: Span) -> IResult<Span, Span> {
    preceded(char('#'), rest)(s)
}

fn loc_key(s: Span) -> IResult<Span, Span> {
    recognize(many0_count(alt((recognize(one_of("_.-'")), alphanumeric1))))(s)
}

fn loc_key_header(s: Span) -> IResult<Span, Span> {
    recognize(tuple((loc_key, char(':'), digit0)))(s)
}

// These format codes are used in games other than CK3.
// Specifically EU4, Stellaris, and HOI4.
fn alternate_format_code(s: Span) -> IResult<Span, char> {
    // § codes are always one character long. (The EU4 wiki disagrees with
    // this, but I scanned the EU4 vanilla loc files and found no examples
    // of longer codes, and some examples of multiple codes like `§G§0`).
    //
    // Which characters are allowed depends on the game. Here we accept
    // all of them from all the games.
    preceded(
        char('§'),
        alt((one_of("!%*=+-"), satisfy(char::is_alphanumeric))),
    )(s)
}

fn word(s: Span) -> IResult<Span, Span> {
    // U+2019 is the unicode apostrophe
    recognize(separated_list1(
        one_of("-'\u{2019}"),
        many1_count(pair(
            take_while1(char::is_alphanumeric),
            take_while(char::is_mark),
        )),
    ))(s)
}

fn code_block(s: Span) -> IResult<Span, Span> {
    delimited(char('['), is_not("]"), char(']'))(s)
}

fn icon_tag(s: Span) -> IResult<Span, Span> {
    // TODO: get fancy and separately mark up $Keyword$ inside icon tags.
    delimited(char('@'), is_not("! "), one_of("! "))(s)
}

fn alternate_icon_tag(s: Span) -> IResult<Span, Span> {
    // This form of icon tags is used in some other games than CK3
    delimited(char('£'), is_not("£ "), one_of("£ "))(s)
}

// Markup can be of the form #high;bold if you really want
fn markup(s: Span) -> IResult<Span, Span> {
    recognize(separated_list1(char(';'), alpha1))(s)
}

fn custom_tag(s: Span) -> IResult<Span, Span> {
    alt((tag("Custom('"), tag("Custom2('")))(s)
}

fn code_upto_custom(s: Span) -> IResult<Span, Span> {
    recognize(tuple((
        char('['),
        many0_count(terminated(none_of("]"), not(custom_tag))),
        none_of("]"),
        custom_tag,
    )))(s)
}

fn custom_code(s: Span) -> IResult<Span, Vec<Token>> {
    map(
        tuple((
            token(TokenType::Code, code_upto_custom),
            token(TokenType::Custom, is_not("'")),
            token(TokenType::Code, terminated(is_not("]"), char(']'))),
        )),
        vec_triple,
    )(s)
}

fn code_tokens(s: Span) -> IResult<Span, Vec<Token>> {
    alt((custom_code, token(TokenType::Code, code_block)))(s)
}

fn loc_value(s: Span) -> IResult<Span, Vec<Token>> {
    fold_many0(
        alt((
            map(
                pair(token(TokenType::WordPart, word), code_tokens),
                vec_pair,
            ),
            map(
                pair(code_tokens, token(TokenType::WordPart, word)),
                vec_pair,
            ),
            token(TokenType::Word, word),
            code_tokens,
            token(TokenType::IconTag, icon_tag),
            token(TokenType::IconTag, alternate_icon_tag),
            // $$ is used to represent a single $ instead of a KeyReference
            no_token(tag("$$")),
            token(
                TokenType::KeyReference,
                delimited(char('$'), is_not("$"), char('$')),
            ),
            token(TokenType::Escape, preceded(char('\\'), anychar)),
            token(
                TokenType::Markup,
                preceded(char('#'), alt((tag("!"), markup))),
            ),
            // Alternate markup syntax, for Stellaris, EU4, HOI4
            token(TokenType::Markup, alternate_format_code),
            // Unescaped embedded double-quotes are allowed.
            // The game engine reads up to the last double-quote on the line.
            no_token(pair(char('"'), peek(take_until("\"")))),
            no_token(none_of("\"")),
        )),
        Vec::new,
        vec_add,
    )(s)
}

fn loc_definition(s: Span) -> IResult<Span, Vec<Token>> {
    map(
        separated_pair(
            token(TokenType::LocKey, loc_key_header),
            space0,
            opt(delimited(char('"'), loc_value, char('"'))),
        ),
        |(v1, v2)| vec_add(v1, v2.unwrap_or_default()),
    )(s)
}

fn line(s: Span) -> IResult<Span, Vec<Token>> {
    delimited(
        space0,
        map(
            separated_pair(
                opt(loc_definition),
                space0,
                opt(token(TokenType::Comment, comment)),
            ),
            |(v1, v2)| vec_add(v1.unwrap_or_default(), v2.unwrap_or_default()),
        ),
        eof,
    )(s)
}

pub fn parse_line(text: &str) -> Vec<Token> {
    match line(Span::new(text)).finish() {
        Ok((_, v)) => v,
        Err(err) => {
            eprintln!("Could not parse line: {}\n {:#}", text, err);
            Vec::new()
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_icon_syntax() {
        let icon = "@warning_icon!";
        let txt = format!(r#" key: "{} Some warning""#, icon);

        let tokens = parse_line(&txt);

        assert_eq!(4, tokens.len());
        assert_eq!(TokenType::LocKey, tokens[0].ttype);
        assert_eq!(1..5, tokens[0].range);
        assert_eq!(TokenType::IconTag, tokens[1].ttype);
        assert_eq!(7..7 + icon.len(), tokens[1].range);
        assert_eq!(TokenType::Word, tokens[2].ttype);
        assert_eq!(TokenType::Word, tokens[3].ttype);
    }

    #[test]
    fn test_icon_syntax_with_code() {
        let icon = "@[saved_scope.GetFlag]!";
        let txt = format!(r#" key: "{} Some text""#, icon);

        let tokens = parse_line(&txt);

        assert_eq!(4, tokens.len());
        assert_eq!(TokenType::LocKey, tokens[0].ttype);
        assert_eq!(1..5, tokens[0].range);
        assert_eq!(TokenType::IconTag, tokens[1].ttype);
        assert_eq!(7..7 + icon.len(), tokens[1].range);
        assert_eq!(TokenType::Word, tokens[2].ttype);
        assert_eq!(TokenType::Word, tokens[3].ttype);
    }

    #[test]
    fn test_alternate_icon_syntax() {
        let icon = "£minerals£";
        let txt = format!(r#" key: "{} minerals""#, icon);

        let tokens = parse_line(&txt);

        assert_eq!(3, tokens.len());
        assert_eq!(TokenType::LocKey, tokens[0].ttype);
        assert_eq!(1..5, tokens[0].range);
        assert_eq!(TokenType::IconTag, tokens[1].ttype);
        assert_eq!(7..7 + icon.len(), tokens[1].range);
        assert_eq!(TokenType::Word, tokens[2].ttype);
    }

    #[test]
    fn test_alternate_markup_syntax() {
        let txt = r#" key: "§Yword§!""#;

        let tokens = parse_line(&txt);

        assert_eq!(4, tokens.len());
        assert_eq!(TokenType::LocKey, tokens[0].ttype);
        assert_eq!(1..5, tokens[0].range);
        assert_eq!(TokenType::Markup, tokens[1].ttype);
        assert_eq!(7..10, tokens[1].range);
        assert_eq!(TokenType::Word, tokens[2].ttype);
        assert_eq!(10..14, tokens[2].range);
        assert_eq!(TokenType::Markup, tokens[3].ttype);
        assert_eq!(14..17, tokens[3].range);
    }

    #[test]
    fn test_custom_code() {
        let txt = r#" key: "ami[bg_opponent.Custom('FR_E')]""#;

        let tokens = parse_line(&txt);

        assert_eq!(5, tokens.len());
        assert_eq!(TokenType::LocKey, tokens[0].ttype);
        assert_eq!(TokenType::WordPart, tokens[1].ttype);
        assert_eq!(TokenType::Code, tokens[2].ttype);
        assert_eq!(TokenType::Custom, tokens[3].ttype);
        assert_eq!(TokenType::Code, tokens[4].ttype);
        assert_eq!(1..5, tokens[0].range);
        assert_eq!(7..10, tokens[1].range);
        assert_eq!(10..31, tokens[2].range);
        assert_eq!(31..35, tokens[3].range);
        assert_eq!(35..38, tokens[4].range);
    }

    #[test]
    fn test_combined_markup() {
        let txt = r##" key: "#high;bold word #!""##;

        let tokens = parse_line(&txt);

        assert_eq!(4, tokens.len());
        assert_eq!(TokenType::LocKey, tokens[0].ttype);
        assert_eq!(TokenType::Markup, tokens[1].ttype);
        assert_eq!(TokenType::Word, tokens[2].ttype);
        assert_eq!(TokenType::Markup, tokens[3].ttype);
        assert_eq!(1..5, tokens[0].range);
        assert_eq!(7..17, tokens[1].range);
        assert_eq!(18..22, tokens[2].range);
        assert_eq!(23..25, tokens[3].range);
    }
}
