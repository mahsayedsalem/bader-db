use anyhow::Result;
use std::sync::Arc;
use crate::resp::value::Value;
use crate::cache::Cache;

#[derive(Debug)]
pub struct Handler {
    value: Value,
}

impl Handler {
    pub fn new(value: Value) -> Self {
        return Self {
            value,
        };
    }

    pub async fn process_request(&mut self, client_store: &Arc<Cache>) -> Result<Value> {
        let (first_arg, args) = self.value.to_command()?;
        let command = first_arg.to_ascii_lowercase().as_str().into();
        let response = match command {
            Command::PING => Value::SimpleString("PONG".to_string()),
            Command::ECHO=> args.first().unwrap().clone(),
            Command::Get => {
                if let Some(Value::BulkString(key)) = args.get(0) {
                    if let Some(value) = client_store.get(key.clone()).await {
                        Value::SimpleString(value)
                    } else {
                        Value::Null
                    }
                } else {
                    Value::Error("Get requires one argument".to_string())
                }
            }
            Command::SET => {
                if let (Some(Value::BulkString(key)), Some(Value::BulkString(value))) =
                    (args.get(0), args.get(1))
                {
                    // TODO commands for different expiration types
                    if let (Some(Value::BulkString(_)), Some(Value::BulkString(amount))) =
                        (args.get(2), args.get(3))
                    {
                        client_store.set_with_expiry(
                            key.clone(),
                            value.clone(),
                            amount.parse::<u64>()?,
                        ).await;
                    } else {
                        client_store.set(key.clone(), value.clone()).await;
                    }
                    Value::SimpleString("OK".to_string())
                } else {
                    Value::Error("Set requires two or four arguments".to_string())
                }
            }
            _ => Value::Error(format!("command not implemented: {}", first_arg)),
        };
        Ok(response)
    }
}

#[derive(Debug, PartialEq)]
pub enum Command {
    PING,
    ECHO,
    Get,
    SET,
    Uninitialized,
}

impl From<&str> for Command {
    fn from(s: &str) -> Self {
        match s {
            "ping" => Command::PING,
            "echo" => Command::ECHO,
            "get" => Command::Get,
            "set" => Command::SET,
            _ => Command::Uninitialized,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::Cache;

    #[tokio::test]
    async fn test_ping_command() -> Result<()> {
        let mut handler = Handler::new(Value::Array(vec![Value::BulkString("PING".to_string())]));
        let cache = Arc::new(Cache::default());
        let response = handler.process_request(&cache).await?;
        assert_eq!(response, Value::SimpleString("PONG".to_string()));
        Ok(())
    }

    #[tokio::test]
    async fn test_echo_command() -> Result<()> {
        let mut handler = Handler::new(Value::Array(vec![
            Value::BulkString("ECHO".to_string()),
            Value::BulkString("hello".to_string())
        ]));
        let cache = Arc::new(Cache::default());
        let response = handler.process_request(&cache).await?;
        assert_eq!(response, Value::BulkString("hello".to_string()));
        Ok(())
    }

    #[tokio::test]
    async fn test_get_command() -> Result<()> {
        let mut handler = Handler::new(Value::Array(vec![
            Value::BulkString("GET".to_string()),
            Value::BulkString("key".to_string())
        ]));
        let cache = Arc::new(Cache::default());

        let response = handler.process_request(&cache).await?;
        assert_eq!(response, Value::Null);

        cache.set("key".to_string(), "value".to_string()).await;
        let response = handler.process_request(&cache).await?;
        assert_eq!(response, Value::SimpleString("value".to_string()));

        Ok(())
    }

    #[tokio::test]
    async fn test_set_command() -> Result<()> {
        let mut handler = Handler::new(Value::Array(vec![
            Value::BulkString("SET".to_string()),
            Value::BulkString("key".to_string()),
            Value::BulkString("value".to_string())
        ]));
        let cache = Arc::new(Cache::default());

        let response = handler.process_request(&cache).await?;
        assert_eq!(response, Value::SimpleString("OK".to_string()));
        assert_eq!(cache.get("key".to_string()).await, Some("value".to_string()));
        Ok(())
    }

    #[tokio::test]
    async fn test_set_with_expiry_command() -> Result<()> {
        let mut handler = Handler::new(Value::Array(vec![
            Value::BulkString("SET".to_string()),
            Value::BulkString("key".to_string()),
            Value::BulkString("value".to_string()),
            Value::BulkString("EX".to_string()),
            Value::BulkString("100".to_string())
        ]));
        let cache = Arc::new(Cache::default());
        let response = handler.process_request(&cache).await?;
        assert_eq!(response, Value::SimpleString("OK".to_string()));
        assert_eq!(cache.get("key".to_string()).await, Some("value".to_string()));

        Ok(())
    }

    #[tokio::test]
    async fn test_set_with_expiry_zero_command() -> Result<()> {
        let mut handler = Handler::new(Value::Array(vec![
            Value::BulkString("SET".to_string()),
            Value::BulkString("key".to_string()),
            Value::BulkString("value".to_string()),
            Value::BulkString("EX".to_string()),
            Value::BulkString("0".to_string())
        ]));
        let cache = Arc::new(Cache::default());
        let response = handler.process_request(&cache).await?;
        assert_eq!(response, Value::SimpleString("OK".to_string()));
        assert_eq!(cache.get("key".to_string()).await, None);

        Ok(())
    }

    #[test]
    fn test_command_from_str() {
        assert_eq!(Command::from("ping"), Command::PING);
        assert_eq!(Command::from("echo"), Command::ECHO);
        assert_eq!(Command::from("get"), Command::Get);
        assert_eq!(Command::from("set"), Command::SET);
        assert_eq!(Command::from("unknown"), Command::Uninitialized);
    }
}
