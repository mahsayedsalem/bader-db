use crate::resp::value::Value;
use anyhow::{Error, Result};
use bytes::BytesMut;

const CARRIAGE_RETURN: u8 = '\r' as u8;
const NEWLINE: u8 = '\n' as u8;

#[derive(Eq, PartialEq, Clone, Debug)]
pub struct Parser;

impl Parser {
    pub fn parse_message(buffer: &BytesMut) -> Result<(Value, usize)> {
        match buffer[0] as char {
            '+' => Self::decode_simple_string(buffer),
            ':' => Self::decode_integer(buffer),
            '*' => Self::decode_array(buffer),
            '$' => Self::decode_bulk_string(buffer),
            _ => Err(Error::msg("unrecognised message type")),
        }
    }

    fn decode_simple_string(buffer: &BytesMut) -> Result<(Value, usize)> {
        if let Some((line, len)) = Self::read_until_crlf(&buffer[1..]) {
            let str = Self::parse_string(line)?;
            return Ok((Value::SimpleString(str), len + 1));
        }
        return Err(Error::msg("Error in decoding simple string"));
    }

    fn decode_integer(buffer: &BytesMut) -> Result<(Value, usize)> {
        if let Some((line, len)) = Self::read_until_crlf(&buffer[1..]) {
            let str = Self::parse_string(line)?;
            return Ok((Value::Integer(str), len + 1));
        }
        return Err(Error::msg("Error in decoding simple string"));
    }

    fn decode_array(buffer: &BytesMut) -> Result<(Value, usize)> {
        let (array_length, mut bytes_consumed) =
            if let Some((line, len)) = Self::read_until_crlf(&buffer[1..]) {
                let array_length = Self::parse_integer(line)?;
                (array_length, len + 1)
            } else {
                return Err(Error::msg("Error in decoding simple array"));
            };

        let mut items: Vec<Value> = Vec::new();
        for _ in 0..array_length {
            match Self::parse_message(&BytesMut::from(&buffer[bytes_consumed..])) {
                Ok((v, len)) => {
                    items.push(v);
                    bytes_consumed += len;
                }
                Err(_e) => {
                    return Err(Error::msg("Error in decoding simple array"));
                }
            }
        }
        return Ok((Value::Array(items), bytes_consumed));
    }

    fn decode_bulk_string(buffer: &BytesMut) -> Result<(Value, usize)> {
        let (bulk_length, bytes_consumed) =
            if let Some((line, len)) = Parser::read_until_crlf(&buffer[1..]) {
                let bulk_length = Parser::parse_integer(line)?;
                (bulk_length, len + 1)
            } else {
                return Err(Error::msg("Error in decoding simple array"));
            };
        let end_of_bulk = bytes_consumed + (bulk_length as usize);
        let end_of_bulk_line = end_of_bulk + 2;
        return if end_of_bulk_line <= buffer.len() {
            Ok((
                Value::BulkString(Self::parse_string(&buffer[bytes_consumed..end_of_bulk])?),
                end_of_bulk_line,
            ))
        } else {
            return Err(Error::msg("Error in decoding simple array"));
        };
    }

    fn read_until_crlf(buffer: &[u8]) -> Option<(&[u8], usize)> {
        for i in 1..buffer.len() {
            if buffer[i - 1] == CARRIAGE_RETURN && buffer[i] == NEWLINE {
                return Some((&buffer[0..(i - 1)], i + 1));
            }
        }
        None
    }

    fn parse_string(bytes: &[u8]) -> Result<String> {
        String::from_utf8(bytes.to_vec()).map_err(|_| Error::msg("Could not parse string"))
    }

    fn parse_integer(bytes: &[u8]) -> Result<i64> {
        let str_integer = Parser::parse_string(bytes)?;
        (str_integer.parse::<i64>()).map_err(|_| Error::msg("Could not parse integer"))
    }
}

#[cfg(test)]
mod tests {
    use crate::resp::parser::Parser;
    use crate::resp::value::Value;
    use bytes::{BufMut, BytesMut};

    #[test]
    fn test_parse_simple_string() {
        let mut bytes = BytesMut::new();
        bytes.put_slice(b"+OK\r\n");
        let (v, s) = Parser::parse_message(&bytes).unwrap();
        assert_eq!(s, 5);
        assert_eq!(v, Value::SimpleString("OK".to_string()));
    }

    #[test]
    fn test_parse_integer() {
        let mut bytes = BytesMut::new();
        bytes.put_slice(b":5\r\n");
        let (v, s) = Parser::parse_message(&bytes).unwrap();
        assert_eq!(s, 4);
        assert_eq!(v, Value::Integer("5".to_string()));
    }

    #[test]
    fn test_parse_bulk_string() {
        let mut bytes = BytesMut::new();
        bytes.put_slice(b"$11\r\nbulk_string\r\n");
        let (v, s) = Parser::parse_message(&bytes).unwrap();
        assert_eq!(s, 18);
        assert_eq!(v, Value::BulkString("bulk_string".to_string()));
    }

    #[test]
    fn test_parse_array() {
        let mut bytes = BytesMut::new();
        bytes.put_slice(b"*2\r\n$5\r\nhello\r\n$5\r\nworld\r\n");
        let (v, s) = Parser::parse_message(&bytes).unwrap();
        assert_eq!(s, 26);
        assert_eq!(
            v,
            Value::Array(vec![
                Value::BulkString("hello".to_string()),
                Value::BulkString("world".to_string())
            ])
        );
    }

    #[test]
    fn test_parse_unknown_input() {
        let mut bytes = BytesMut::new();
        bytes.put_slice(b"hello world");
        assert_eq!(Parser::parse_message(&bytes).is_err(), true);
    }
}
