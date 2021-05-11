/*
 * Created on Mon May 10 2021
 *
 * This file is a part of Skytable
 * Skytable (formerly known as TerrabaseDB or Skybase) is a free and open-source
 * NoSQL database written by Sayan Nandan ("the Author") with the
 * vision to provide flexibility in data modelling without compromising
 * on performance, queryability or scalability.
 *
 * Copyright (c) 2021, Sayan Nandan <ohsayan@outlook.com>
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU Affero General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU Affero General Public License for more details.
 *
 * You should have received a copy of the GNU Affero General Public License
 * along with this program. If not, see <https://www.gnu.org/licenses/>.
 *
*/

use std::hint::unreachable_unchecked;

#[derive(Debug)]
pub(super) struct Parser<'a> {
    cursor: usize,
    buffer: &'a [u8],
}

#[derive(Debug, PartialEq)]
pub enum ParseError {
    /// Didn't get the number of expected bytes
    NotEnough,
    /// The query contains an unexpected byte
    UnexpectedByte,
    /// The packet simply contains invalid data
    BadPacket,
    /// A data type was given but the parser failed to serialize it into this type
    DataTypeParseError,
    /// A data type that the server doesn't know was passed into the query
    ///
    /// This is a frequent problem that can arise between different server editions as more data types
    /// can be added with changing server versions
    UnknownDatatype,
}

#[derive(Debug, PartialEq)]
pub enum Query {
    SimpleQuery(DataType),
    PipelinedQuery(Vec<DataType>),
}

#[derive(Debug, PartialEq)]
#[non_exhaustive]
pub enum DataType {
    /// Arrays can be nested! Their `<tsymbol>` is `&`
    Array(Vec<DataType>),
    /// A String value; `<tsymbol>` is `+`
    String(String),
    /// An unsigned integer value; `<tsymbol>` is `:`
    UnsignedInt(u64),
}

type ParseResult<T> = Result<T, ParseError>;

impl<'a> Parser<'a> {
    pub const fn new(buffer: &'a [u8]) -> Self {
        Parser {
            cursor: 0usize,
            buffer,
        }
    }
    /// Read from the current cursor position to `until` number of positions ahead
    /// This **will forward the cursor itself** if the bytes exist or it will just return a `NotEnough` error
    fn read_until(&mut self, until: usize) -> ParseResult<&[u8]> {
        if let Some(b) = self.buffer.get(self.cursor..self.cursor + until) {
            self.cursor += until;
            Ok(b)
        } else {
            Err(ParseError::NotEnough)
        }
    }
    /// This returns the position at which the line parsing began and the position at which the line parsing
    /// stopped, in other words, you should be able to do self.buffer[started_at..stopped_at] to get a line
    /// and do it unchecked. This **will move the internal cursor ahead** and place it **at the `\n` byte**
    fn read_line(&mut self) -> (usize, usize) {
        let started_at = self.cursor;
        let mut stopped_at = self.cursor;
        while self.cursor < self.buffer.len() {
            if self.buffer[self.cursor] == b'\n' {
                // Oh no! Newline reached, time to break the loop
                // But before that ... we read the newline, so let's advance the cursor
                self.incr_cursor();
                break;
            }
            // So this isn't an LF, great! Let's forward the stopped_at position
            stopped_at += 1;
            self.incr_cursor();
        }
        (started_at, stopped_at)
    }
    fn incr_cursor(&mut self) {
        self.cursor += 1;
    }
    fn will_cursor_give_char(&self, ch: u8, this_if_nothing_ahead: bool) -> ParseResult<bool> {
        self.buffer.get(self.cursor).map_or(
            if this_if_nothing_ahead {
                Ok(true)
            } else {
                Err(ParseError::NotEnough)
            },
            |v| Ok(*v == ch),
        )
    }
    fn will_cursor_give_linefeed(&self) -> ParseResult<bool> {
        self.will_cursor_give_char(b'\n', false)
    }
    fn parse_into_usize(bytes: &[u8]) -> ParseResult<usize> {
        if bytes.len() == 0 {
            return Err(ParseError::NotEnough);
        }
        let mut byte_iter = bytes.into_iter();
        let mut item_usize = 0usize;
        while let Some(dig) = byte_iter.next() {
            if !dig.is_ascii_digit() {
                // dig has to be an ASCII digit
                return Err(ParseError::DataTypeParseError);
            }
            // 48 is the ASCII code for 0, and 57 is the ascii code for 9
            // so if 0 is given, the subtraction should give 0; similarly
            // if 9 is given, the subtraction should give us 9!
            let curdig: usize = dig
                .checked_sub(48)
                .unwrap_or_else(|| unsafe { unreachable_unchecked() })
                .into();
            // The usize can overflow; check that case
            let product = match item_usize.checked_mul(10) {
                Some(not_overflowed) => not_overflowed,
                None => return Err(ParseError::DataTypeParseError),
            };
            let sum = match product.checked_add(curdig) {
                Some(not_overflowed) => not_overflowed,
                None => return Err(ParseError::DataTypeParseError),
            };
            item_usize = sum;
        }
        Ok(item_usize)
    }
    fn parse_into_u64(bytes: &[u8]) -> ParseResult<u64> {
        if bytes.len() == 0 {
            return Err(ParseError::NotEnough);
        }
        let mut byte_iter = bytes.into_iter();
        let mut item_u64 = 0u64;
        while let Some(dig) = byte_iter.next() {
            if !dig.is_ascii_digit() {
                // dig has to be an ASCII digit
                return Err(ParseError::DataTypeParseError);
            }
            // 48 is the ASCII code for 0, and 57 is the ascii code for 9
            // so if 0 is given, the subtraction should give 0; similarly
            // if 9 is given, the subtraction should give us 9!
            let curdig: u64 = dig
                .checked_sub(48)
                .unwrap_or_else(|| unsafe { unreachable_unchecked() })
                .into();
            // Now the entire u64 can overflow, so let's attempt to check it
            let product = match item_u64.checked_mul(10) {
                Some(not_overflowed) => not_overflowed,
                None => return Err(ParseError::DataTypeParseError),
            };
            let sum = match product.checked_add(curdig) {
                Some(not_overflowed) => not_overflowed,
                None => return Err(ParseError::DataTypeParseError),
            };
            item_u64 = sum;
        }
        Ok(item_u64)
    }
    /// This will return the number of datagroups present in this query packet
    ///
    /// This **will forward the cursor itself**
    fn parse_metaframe_get_datagroup_count(&mut self) -> ParseResult<usize> {
        // the smallest query we can have is: *1\n or 3 chars
        if self.buffer.len() < 3 {
            return Err(ParseError::NotEnough);
        }
        // Now we want to read `*<n>\n`
        let (start, stop) = self.read_line();
        if let Some(our_chunk) = self.buffer.get(start..stop) {
            if our_chunk[0] == b'*' {
                // Good, this will tell us the number of actions
                // Let us attempt to read the usize from this point onwards
                // that is excluding the '*' (so 1..)
                let ret = Self::parse_into_usize(&our_chunk[1..])?;
                Ok(ret)
            } else {
                Err(ParseError::UnexpectedByte)
            }
        } else {
            Err(ParseError::NotEnough)
        }
    }
    /// Get the next element **without** the tsymbol
    ///
    /// This function **does not forward the newline**
    fn __get_next_element(&mut self) -> ParseResult<&[u8]> {
        let string_sizeline = self.read_line();
        if let Some(line) = self.buffer.get(string_sizeline.0..string_sizeline.1) {
            let string_size = Self::parse_into_usize(line)?;
            let our_chunk = self.read_until(string_size)?;
            Ok(our_chunk)
        } else {
            Err(ParseError::NotEnough)
        }
    }
    /// The cursor should have passed the `+` tsymbol
    fn parse_next_string(&mut self) -> ParseResult<String> {
        let our_string_chunk = self.__get_next_element()?;
        let our_string = String::from_utf8_lossy(&our_string_chunk).to_string();
        if self.will_cursor_give_linefeed()? {
            // there is a lf after the end of the string; great!
            // let's skip that now
            self.incr_cursor();
            // let's return our string
            Ok(our_string)
        } else {
            Err(ParseError::UnexpectedByte)
        }
    }
    /// The cursor should have passed the `:` tsymbol
    fn parse_next_u64(&mut self) -> ParseResult<u64> {
        let our_u64_chunk = self.__get_next_element()?;
        let our_u64 = Self::parse_into_u64(our_u64_chunk)?;
        if self.will_cursor_give_linefeed()? {
            // line feed after u64; heck yeah!
            self.incr_cursor();
            // return it
            Ok(our_u64)
        } else {
            Err(ParseError::UnexpectedByte)
        }
    }
    /// The cursor should be **at the tsymbol**
    fn parse_next_element(&mut self) -> ParseResult<DataType> {
        if let Some(tsymbol) = self.buffer.get(self.cursor) {
            // so we have a tsymbol; nice, let's match it
            // but advance the cursor before doing that
            self.incr_cursor();
            let ret = match *tsymbol {
                b'+' => DataType::String(self.parse_next_string()?),
                b':' => DataType::UnsignedInt(self.parse_next_u64()?),
                b'&' => DataType::Array(self.parse_next_array()?),
                _ => return Err(ParseError::UnknownDatatype),
            };
            Ok(ret)
        } else {
            // Not enough bytes to read an element
            Err(ParseError::NotEnough)
        }
    }
    /// The tsymbol `&` should have been passed!
    fn parse_next_array(&mut self) -> ParseResult<Vec<DataType>> {
        let (start, stop) = self.read_line();
        if let Some(our_size_chunk) = self.buffer.get(start..stop) {
            let array_size = Self::parse_into_usize(our_size_chunk)?;
            let mut array = Vec::with_capacity(array_size);
            for _ in 0..array_size {
                array.push(self.parse_next_element()?);
            }
            Ok(array)
        } else {
            Err(ParseError::NotEnough)
        }
    }
    pub fn parse(mut self) -> Result<(Query, usize), ParseError> {
        let number_of_queries = self.parse_metaframe_get_datagroup_count()?;
        println!("Got count: {}", number_of_queries);
        if number_of_queries == 0 {
            // how on earth do you expect us to execute 0 queries? waste of bandwidth
            return Err(ParseError::BadPacket);
        }
        if number_of_queries == 1 {
            // This is a simple query
            let single_group = self.parse_next_element()?;
            // The below line defaults to false if no item is there in the buffer
            // or it checks if the next time is a \r char; if it is, then it is the beginning
            // of the next query
            if self
                .will_cursor_give_char(b'*', true)
                .unwrap_or_else(|_| unsafe {
                    // This will never be the case because we'll always get a result and no error value
                    // as we've passed true which will yield Ok(true) even if there is no byte ahead
                    unreachable_unchecked()
                })
            {
                Ok((Query::SimpleQuery(single_group), self.cursor))
            } else {
                // the next item isn't the beginning of a query but something else?
                // that doesn't look right!
                Err(ParseError::UnexpectedByte)
            }
        } else {
            // This is a pipelined query
            // We'll first make space for all the actiongroups
            let mut queries = Vec::with_capacity(number_of_queries);
            for _ in 0..number_of_queries {
                queries.push(self.parse_next_element()?);
            }
            if self.will_cursor_give_char(b'*', true)? {
                Ok((Query::PipelinedQuery(queries), self.cursor))
            } else {
                Err(ParseError::UnexpectedByte)
            }
        }
    }
}

#[test]
fn test_metaframe_parse() {
    let metaframe = "*2\n".as_bytes();
    let mut parser = Parser::new(&metaframe);
    assert_eq!(2, parser.parse_metaframe_get_datagroup_count().unwrap());
    assert_eq!(parser.cursor, metaframe.len());
}

#[test]
fn test_cursor_next_char() {
    let bytes = &[b'\n'];
    assert!(Parser::new(&bytes[..])
        .will_cursor_give_char(b'\n', false)
        .unwrap());
    let bytes = &[];
    assert!(Parser::new(&bytes[..])
        .will_cursor_give_char(b'\r', true)
        .unwrap());
    let bytes = &[];
    assert!(
        Parser::new(&bytes[..])
            .will_cursor_give_char(b'\n', false)
            .unwrap_err()
            == ParseError::NotEnough
    );
}

#[test]
fn test_metaframe_parse_fail() {
    // First byte should be CR and not $
    let metaframe = "$2\n*2\n".as_bytes();
    let mut parser = Parser::new(&metaframe);
    assert_eq!(
        parser.parse_metaframe_get_datagroup_count().unwrap_err(),
        ParseError::UnexpectedByte
    );
    // Give a wrong length approximation
    let metaframe = "\r1\n*2\n".as_bytes();
    assert_eq!(
        Parser::new(&metaframe)
            .parse_metaframe_get_datagroup_count()
            .unwrap_err(),
        ParseError::UnexpectedByte
    );
}

#[test]
fn test_query_fail_not_enough() {
    let query_packet = "*".as_bytes();
    assert_eq!(
        Parser::new(&query_packet).parse().err().unwrap(),
        ParseError::NotEnough
    );
    let metaframe = "*2".as_bytes();
    assert_eq!(
        Parser::new(&metaframe)
            .parse_metaframe_get_datagroup_count()
            .unwrap_err(),
        ParseError::NotEnough
    );
}

#[test]
fn test_parse_next_string() {
    let bytes = "5\nsayan\n".as_bytes();
    let st = Parser::new(&bytes).parse_next_string().unwrap();
    assert_eq!(st, "sayan".to_owned());
}

#[test]
fn test_parse_next_u64() {
    let max = 18446744073709551615;
    assert!(u64::MAX == max);
    let bytes = "20\n18446744073709551615\n".as_bytes();
    let our_u64 = Parser::new(&bytes).parse_next_u64().unwrap();
    assert_eq!(our_u64, max);
    // now overflow the u64
    let bytes = "21\n184467440737095516156\n".as_bytes();
    let our_u64 = Parser::new(&bytes).parse_next_u64().unwrap_err();
    assert_eq!(our_u64, ParseError::DataTypeParseError);
}

#[test]
fn test_parse_next_element_string() {
    let bytes = "+5\nsayan\n".as_bytes();
    let next_element = Parser::new(&bytes).parse_next_element().unwrap();
    assert_eq!(next_element, DataType::String("sayan".to_owned()));
}

#[test]
fn test_parse_next_element_string_fail() {
    let bytes = "+5\nsayan".as_bytes();
    assert_eq!(Parser::new(&bytes).parse_next_element().unwrap_err(), ParseError::NotEnough);
}

#[test]
fn test_parse_next_element_u64() {
    let bytes = ":20\n18446744073709551615\n".as_bytes();
    let our_u64 = Parser::new(&bytes).parse_next_element().unwrap();
    assert_eq!(our_u64, DataType::UnsignedInt(u64::MAX));
}

#[test]
fn test_parse_next_element_u64_fail() {
    let bytes = ":20\n18446744073709551615".as_bytes();
    assert_eq!(Parser::new(&bytes).parse_next_element().unwrap_err(), ParseError::NotEnough);
}

#[test]
fn test_parse_next_element_array() {
    let bytes = "&3\n+4\nMGET\n+3\nfoo\n+3\nbar\n".as_bytes();
    let mut parser = Parser::new(&bytes);
    let array = parser.parse_next_element().unwrap();
    assert_eq!(
        array,
        DataType::Array(vec![
            DataType::String("MGET".to_owned()),
            DataType::String("foo".to_owned()),
            DataType::String("bar".to_owned())
        ])
    );
    assert_eq!(parser.cursor, bytes.len());
}

#[test]
fn test_parse_next_element_array_fail() {
    // should've been three elements, but there are two!
    let bytes = "&3\n+4\nMGET\n+3\nfoo\n+3\n".as_bytes();
    let mut parser = Parser::new(&bytes);
    assert_eq!(parser.parse_next_element().unwrap_err(), ParseError::NotEnough);
}

#[test]
fn test_parse_nested_array() {
    // let's test a nested array
    let bytes =
        "&3\n+3\nACT\n+3\nfoo\n&4\n+5\nsayan\n+2\nis\n+7\nworking\n&2\n+6\nreally\n+4\nhard\n"
            .as_bytes();
    let mut parser = Parser::new(&bytes);
    let array = parser.parse_next_element().unwrap();
    assert_eq!(
        array,
        DataType::Array(vec![
            DataType::String("ACT".to_owned()),
            DataType::String("foo".to_owned()),
            DataType::Array(vec![
                DataType::String("sayan".to_owned()),
                DataType::String("is".to_owned()),
                DataType::String("working".to_owned()),
                DataType::Array(vec![
                    DataType::String("really".to_owned()),
                    DataType::String("hard".to_owned())
                ])
            ])
        ])
    );
    assert_eq!(parser.cursor, bytes.len());
}

#[test]
fn test_parse_multitype_array() {
    // let's test a nested array
    let bytes = "&3\n+3\nACT\n+3\nfoo\n&4\n+5\nsayan\n+2\nis\n+7\nworking\n&2\n:2\n23\n+5\napril\n"
        .as_bytes();
    let mut parser = Parser::new(&bytes);
    let array = parser.parse_next_element().unwrap();
    assert_eq!(
        array,
        DataType::Array(vec![
            DataType::String("ACT".to_owned()),
            DataType::String("foo".to_owned()),
            DataType::Array(vec![
                DataType::String("sayan".to_owned()),
                DataType::String("is".to_owned()),
                DataType::String("working".to_owned()),
                DataType::Array(vec![
                    DataType::UnsignedInt(23),
                    DataType::String("april".to_owned())
                ])
            ])
        ])
    );
    assert_eq!(parser.cursor, bytes.len());
}

#[test]
fn test_parse_a_query() {
    let bytes =
        "*1\n&3\n+3\nACT\n+3\nfoo\n&4\n+5\nsayan\n+2\nis\n+7\nworking\n&2\n:2\n23\n+5\napril\n"
            .as_bytes();
    let parser = Parser::new(&bytes);
    let (resp, forward_by) = parser.parse().unwrap();
    assert_eq!(
        resp,
        Query::SimpleQuery(DataType::Array(vec![
            DataType::String("ACT".to_owned()),
            DataType::String("foo".to_owned()),
            DataType::Array(vec![
                DataType::String("sayan".to_owned()),
                DataType::String("is".to_owned()),
                DataType::String("working".to_owned()),
                DataType::Array(vec![
                    DataType::UnsignedInt(23),
                    DataType::String("april".to_owned())
                ])
            ])
        ]))
    );
    assert_eq!(forward_by, bytes.len());
}

#[test]
fn test_parse_a_query_fail_moredata() {
    let bytes =
        "*1\n&3\n+3\nACT\n+3\nfoo\n&4\n+5\nsayan\n+2\nis\n+7\nworking\n&1\n:2\n23\n+5\napril\n"
            .as_bytes();
    let parser = Parser::new(&bytes);
    assert_eq!(parser.parse().unwrap_err(), ParseError::UnexpectedByte);
}

#[test]
fn test_pipelined_query_incomplete() {
    // this was a pipelined query: we expected two queries but got one!
    let bytes =
        "*2\n&3\n+3\nACT\n+3\nfoo\n&4\n+5\nsayan\n+2\nis\n+7\nworking\n&2\n:2\n23\n+5\napril\n"
            .as_bytes();
    assert_eq!(
        Parser::new(&bytes).parse().unwrap_err(),
        ParseError::NotEnough
    )
}

#[test]
fn test_pipelined_query() {
    let bytes =
        "*2\n&3\n+3\nACT\n+3\nfoo\n&3\n+5\nsayan\n+2\nis\n+7\nworking\n+4\nHEYA\n".as_bytes();
    /*
    (\r2\n*2\n)(&3\n)({+3\nACT\n}{+3\nfoo\n}{[&3\n][+5\nsayan\n][+2\nis\n][+7\nworking\n]})(+4\nHEYA\n)
    */
    let (res, forward_by) = Parser::new(&bytes).parse().unwrap();
    assert_eq!(
        res,
        Query::PipelinedQuery(vec![
            DataType::Array(vec![
                DataType::String("ACT".to_owned()),
                DataType::String("foo".to_owned()),
                DataType::Array(vec![
                    DataType::String("sayan".to_owned()),
                    DataType::String("is".to_owned()),
                    DataType::String("working".to_owned())
                ])
            ]),
            DataType::String("HEYA".to_owned())
        ])
    );
    assert_eq!(forward_by, bytes.len());
}

#[test]
fn test_query_with_part_of_next_query() {
    let bytes =
        "*1\n&3\n+3\nACT\n+3\nfoo\n&4\n+5\nsayan\n+2\nis\n+7\nworking\n&2\n:2\n23\n+5\napril\n*1\n"
            .as_bytes();
    let (res, forward_by) = Parser::new(&bytes).parse().unwrap();
    assert_eq!(
        res,
        Query::SimpleQuery(DataType::Array(vec![
            DataType::String("ACT".to_owned()),
            DataType::String("foo".to_owned()),
            DataType::Array(vec![
                DataType::String("sayan".to_owned()),
                DataType::String("is".to_owned()),
                DataType::String("working".to_owned()),
                DataType::Array(vec![
                    DataType::UnsignedInt(23),
                    DataType::String("april".to_owned())
                ])
            ])
        ]))
    );
    // there are some ingenious folks on this planet who might just go bombing one query after the other
    // we happily ignore those excess queries and leave it to the next round of parsing
    assert_eq!(forward_by, bytes.len() - "*1\n".len());
}
