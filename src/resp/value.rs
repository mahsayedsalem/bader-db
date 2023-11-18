use crate::resp::parser::Parser;
use anyhow::{Error, Result};
use bytes::BytesMut;

#[derive(Eq, PartialEq, Clone, Debug)]
pub enum Value {
    Null,
    SimpleString(String),
    Integer(String),
    Error(String),
    BulkString(String),
    Array(Vec<Value>),
}

impl Value {
    pub fn to_command(&self) -> Result<(String, Vec<Value>)> {
        match self {
            Value::Array(items) => {
                return Ok((
                    items.first().unwrap().unwrap_bulk(),
                    items.clone().into_iter().skip(1).collect(),
                ));
            }
            _ => Err(Error::msg("not an array")),
        }
    }

    fn unwrap_bulk(&self) -> String {
        match self {
            Value::BulkString(str) => str.clone(),
            _ => panic!("not a bulk string"),
        }
    }

    pub fn encode(self) -> String {
        match self {
            Value::Null => "$-1\r\n".to_string(),
            Value::SimpleString(s) => format!("+{}\r\n", s),
            Value::Integer(s) => format!(":{}\r\n", s),
            Value::Error(msg) => format!("-{}\r\n", msg),
            Value::BulkString(s) => format!("${}\r\n{}\r\n", s.chars().count(), s),
            _ => panic!("value encode not implemented for: {:?}", self),
        }
    }
}

impl From<&mut BytesMut> for Value {
    fn from(buffer: &mut BytesMut) -> Self {
        match Parser::parse_message(buffer) {
            Ok((v, _)) => v,
            _ => Self::Error("error in parsing".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::resp::value::Value;
    use bytes::{BufMut, BytesMut};

    #[test]
    fn test_unwrap_bulk_string() {
        let bulk_string = "this is a bulk string";
        let value = Value::BulkString(bulk_string.to_string());
        assert_eq!(bulk_string.to_string(), value.unwrap_bulk());
    }

    #[test]
    fn test_to_command() {
        let v = vec![
            Value::BulkString("set".to_string()),
            Value::BulkString("country egypt".to_string()),
        ];
        let v = Value::Array(v);
        let command = v.to_command().unwrap();
        assert_eq!(command.0, "set".to_string());
        assert_eq!(
            command.1.first().unwrap().unwrap_bulk(),
            "country egypt".to_string()
        );
    }

    #[test]
    fn test_to_command_error() {
        let v = Value::BulkString("set".to_string());
        assert_eq!(v.to_command().is_err(), true);
    }

    #[test]
    fn test_to_command_one_entry() {
        let v = vec![Value::BulkString("set".to_string())];
        let v = Value::Array(v);
        let command = v.to_command().unwrap();
        assert_eq!(command.1.len(), 0);
    }

    #[test]
    fn test_encode_null_value() {
        let value = Value::Null;
        assert_eq!("$-1\r\n".to_string(), value.encode());
    }

    #[test]
    fn test_encode_simple_string_value() {
        let value = Value::SimpleString("m".to_string());
        assert_eq!("+m\r\n".to_string(), value.encode());
    }

    #[test]
    fn test_encode_integer_value() {
        let value = Value::Integer("5".to_string());
        assert_eq!(":5\r\n".to_string(), value.encode());
    }

    #[test]
    fn test_encode_error_value() {
        let value = Value::Error("error".to_string());
        assert_eq!("-error\r\n".to_string(), value.encode());
    }

    #[test]
    fn test_encode_bulk_string_value() {
        let value = Value::BulkString("bulk_string".to_string());
        assert_eq!("$11\r\nbulk_string\r\n".to_string(), value.encode());
    }

    #[test]
    #[should_panic]
    fn test_encode_array_value_cause_panic() {
        let v = vec![
            Value::BulkString("set".to_string()),
            Value::BulkString("country egypt".to_string()),
        ];
        let v = Value::Array(v);
        v.encode();
    }

    #[test]
    fn test_from_bytes() {
        let mut bytes = BytesMut::new();
        bytes.put_slice(b"*2\r\n$5\r\nhello\r\n$5\r\nworld\r\n");
        let v = Value::from(&mut bytes);
        assert_eq!(
            v,
            Value::Array(vec![
                Value::BulkString("hello".to_string()),
                Value::BulkString("world".to_string())
            ])
        )
    }
}
