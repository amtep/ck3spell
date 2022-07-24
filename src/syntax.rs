use nom::branch::alt;
use nom::bytes::complete::{is_not, take_while1};
use nom::character::complete::{
    alphanumeric1, anychar, char, digit0, none_of, one_of, space0,
};
use nom::combinator::{eof, map, not, recognize, rest};
use nom::multi::{fold_many0, many0_count};
use nom::sequence::{
    delimited, pair, preceded, separated_pair, terminated, tuple,
};
use nom::{Finish, IResult};
use nom_locate::{position, LocatedSpan};
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

fn is_word_char_no_apo(c: char) -> bool {
    c.is_alphanumeric() || c == '-'
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

fn vec_add<T>(mut v1: Vec<T>, mut v2: Vec<T>) -> Vec<T> {
    v1.append(&mut v2);
    v1
}

fn vec_pair<T>((v1, v2): (Vec<T>, Vec<T>)) -> Vec<T> {
    vec_add(v1, v2)
}

fn comment(s: Span) -> IResult<Span, Span> {
    preceded(char('#'), rest)(s)
}

fn loc_key(s: Span) -> IResult<Span, Span> {
    recognize(many0_count(alt((recognize(one_of("_.")), alphanumeric1))))(s)
}

fn loc_key_header(s: Span) -> IResult<Span, Span> {
    recognize(tuple((loc_key, char(':'), digit0)))(s)
}

fn word(s: Span) -> IResult<Span, Span> {
    take_while1(is_word_char)(s)
}

fn word_no_apo(s: Span) -> IResult<Span, Span> {
    take_while1(is_word_char_no_apo)(s)
}

fn code_block(s: Span) -> IResult<Span, Span> {
    delimited(char('['), is_not("]"), char(']'))(s)
}

fn loc_value(s: Span) -> IResult<Span, Vec<Token>> {
    fold_many0(
        alt((
            delimited(
                char('\''),
                token(TokenType::Word, word_no_apo),
                char('\''),
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
            token(
                TokenType::KeyReference,
                delimited(char('$'), is_not("$"), char('$')),
            ),
            token(TokenType::Escape, preceded(char('\\'), anychar)),
            no_token(pair(char('"'), not(pair(space0, eof)))),
            no_token(none_of("\"")),
        )),
        Vec::new,
        vec_add,
    )(s)
}

fn line(s: Span) -> IResult<Span, Vec<Token>> {
    delimited(
        space0,
        alt((
            no_token(eof),
            token(TokenType::Comment, comment),
            map(
                separated_pair(
                    token(TokenType::LocKey, loc_key_header),
                    space0,
                    delimited(
                        char('"'),
                        loc_value,
                        terminated(char('"'), space0),
                    ),
                ),
                vec_pair,
            ),
            terminated(token(TokenType::LocKey, loc_key_header), space0),
        )),
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
