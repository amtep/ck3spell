use nom::branch::alt;
use nom::bytes::complete::{is_not, tag, take_until, take_while1};
use nom::character::complete::{
    alpha1, alphanumeric1, anychar, char, digit0, none_of, one_of, space0,
    space1,
};
use nom::combinator::{eof, map, not, opt, peek, recognize, rest};
use nom::multi::{fold_many0, many0_count, separated_list1};
use nom::sequence::{
    delimited, pair, preceded, separated_pair, terminated, tuple,
};
use nom::{Finish, IResult};
use nom_locate::{position, LocatedSpan};
use std::fmt::Debug;
use std::ops::Range;

type Span<'a> = LocatedSpan<&'a str>;

#[derive(Clone, Copy, Debug)]
pub enum TokenType {
    Comment,
    LocKey,
    KeyReference,
    Word,
    WordPart,
    Escape,
    Code,
    Markup,
    IconTag,
}

#[derive(Clone, Debug)]
pub struct Token {
    pub ttype: TokenType,
    pub range: Range<usize>,
}

fn is_word_char(c: char) -> bool {
    // U+2019 is the unicode apostrophe
    // alphanumeric is accepted for words like "2nd"
    c.is_alphanumeric() || c == '\'' || c == '\u{2019}' || c == '-'
}

fn is_word_char_no_apostrophes(c: char) -> bool {
    c.is_alphanumeric() || c == '-'
}

fn is_word_char_no_dash(c: char) -> bool {
    c != '-' && is_word_char(c)
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

fn no_token<'a, F: 'a, O>(
    mut inner: F,
) -> impl FnMut(Span<'a>) -> IResult<Span<'a>, Vec<Token>>
where
    F: FnMut(Span<'a>) -> IResult<Span<'a>, O>,
{
    move |s: Span| {
        let (s, _) = inner(s)?;
        Ok((s, Vec::new()))
    }
}

#[allow(dead_code)]
fn log<'a, F: 'a, O>(
    mut inner: F,
) -> impl FnMut(Span<'a>) -> IResult<Span<'a>, O>
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

fn vec_flatten<T>(mut vv: Vec<Vec<T>>) -> Vec<T> {
    let mut v = Vec::new();
    for vt in vv.iter_mut() {
        v.append(vt);
    }
    v
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

fn word(s: Span) -> IResult<Span, Span> {
    take_while1(is_word_char)(s)
}

fn word_no_apostrophes(s: Span) -> IResult<Span, Span> {
    take_while1(is_word_char_no_apostrophes)(s)
}

fn word_no_double_dash(s: Span) -> IResult<Span, Span> {
    recognize(separated_list1(
        char('-'),
        take_while1(is_word_char_no_dash),
    ))(s)
}

fn quoted_phrase(s: Span) -> IResult<Span, Vec<Token>> {
    map(
        separated_list1(space1, token(TokenType::Word, word_no_apostrophes)),
        vec_flatten,
    )(s)
}

fn code_block(s: Span) -> IResult<Span, Span> {
    delimited(char('['), is_not("]"), char(']'))(s)
}

fn icon_tag(s: Span) -> IResult<Span, Span> {
    // TODO: get fancy and separately mark up $Keyword$ inside icon tags.
    delimited(char('@'), is_not("! "), one_of("! "))(s)
}

fn loc_value(s: Span) -> IResult<Span, Vec<Token>> {
    fold_many0(
        alt((
            terminated(token(TokenType::Word, word_no_double_dash), tag("--")),
            delimited(char('\''), quoted_phrase, pair(char('\''), not(word))),
            delimited(
                char('\u{2018}'),
                quoted_phrase,
                pair(char('\u{2019}'), not(word)),
            ),
            map(
                pair(
                    token(TokenType::WordPart, word),
                    token(TokenType::Code, code_block),
                ),
                vec_pair,
            ),
            map(
                pair(
                    token(TokenType::Code, code_block),
                    token(TokenType::WordPart, word),
                ),
                vec_pair,
            ),
            token(TokenType::Word, word),
            token(TokenType::Code, code_block),
            token(TokenType::IconTag, icon_tag),
            token(
                TokenType::KeyReference,
                delimited(char('$'), is_not("$"), char('$')),
            ),
            token(TokenType::Escape, preceded(char('\\'), anychar)),
            token(
                TokenType::Markup,
                preceded(char('#'), alt((tag("!"), alpha1))),
            ),
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
