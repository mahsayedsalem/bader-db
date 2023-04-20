use anyhow::Result;
use std::sync::Arc;
use crate::resp::value::Value;
use crate::cache::expiry::ExpiryFormat;
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

    pub async fn handle_request(&mut self, client_store: &Arc<Cache>) -> Result<Value> {
        let (first_arg, args) = self.value.to_command()?;
        let command = first_arg.to_ascii_lowercase().as_str().into();
        let response = match command {
            Command::PING => Value::SimpleString("PONG".to_string()),
            Command::ECHO=> args.first().unwrap().clone(),
            Command::Get => self.handle_get(client_store, &args).await,
            Command::SET => self.handle_set(client_store, &args).await,
            Command::DELETE => self.handle_delete(client_store, &args).await,
            Command::EXISTS => self.handle_exists(client_store, &args).await,
            _ => Value::Error(format!("command not implemented: {}", first_arg)),
        };
        Ok(response)
    }

    async fn handle_get(&self, client_store: &Arc<Cache>, args: &Vec<Value>) -> Value {
        if let Some(Value::BulkString(key)) = args.get(0) {
            if let Some(value) = client_store.get(key.clone()).await {
                Value::SimpleString(value)
            } else {
                Value::Null
            }
        } else {
            Value::Error("GET requires one argument".to_string())
        }
    }

    async fn handle_set(&self, client_store: &Arc<Cache>, args: &Vec<Value>) -> Value {
        if let (Some(Value::BulkString(key)), Some(Value::BulkString(value))) =
            (args.get(0), args.get(1))
        {
            if let (Some(Value::BulkString(expiry_format)), Some(Value::BulkString(amount))) =
                (args.get(2), args.get(3))
            {
                let e = ExpiryFormat::from(expiry_format.as_str());
                if e != ExpiryFormat::Uninitialized {
                    self.handle_set_with_expiry(client_store, key, value, amount, Some(expiry_format)).await
                } else {
                    self.handle_set_with_expiry(client_store, key, value, amount, None).await
                }
            } else {
                client_store.set(key.clone(), value.clone()).await;
                Value::SimpleString("OK".to_string())
            }
        } else {
            Value::Error("SET requires two or four arguments".to_string())
        }
    }

    async fn handle_set_with_expiry(&self, client_store: &Arc<Cache>, key: &String, value: &String, amount: &String, expiry_format: Option<&String>) -> Value {
        if let Ok(amount) = amount.parse::<u64>() {

            match expiry_format {
                Some(e) => {
                    client_store.set_with_expiry(
                    key.clone(),
                    value.clone(),
                    (amount, e),
                ).await;
                }
                _ => {
                    client_store.set_with_expiry(
                        key.clone(),
                        value.clone(),
                        amount,
                    ).await;
                }

            }
            Value::SimpleString("OK".to_string())
        } else {
            Value::Error("Unsupported expiry format".to_string())
        }
    }

    async fn handle_delete(&self, client_store: &Arc<Cache>, args: &Vec<Value>) -> Value {
        if let Some(Value::BulkString(key)) = args.get(0) {
            match client_store.remove(key.clone()).await {
                Ok(e) => Value::SimpleString("OK".to_string()),
                Err(e) => Value::Error(format!("Error while deleting: {:?}", e))
            }
        } else {
            Value::Error("DEL requires one argument".to_string())
        }
    }

    async fn handle_exists(&self, client_store: &Arc<Cache>, args: &Vec<Value>) -> Value {
        if let Some(Value::BulkString(key)) = args.get(0) {
            match client_store.exists(key.clone()).await {
                true => Value::SimpleString("true".to_string()),
                false => Value::SimpleString("false".to_string()),
            }
        } else {
            Value::Error("EXISTS requires one argument".to_string())
        }
    }

}

#[derive(Debug, PartialEq)]
pub enum Command {
    PING,
    ECHO,
    Get,
    SET,
    DELETE,
    EXISTS,
    Uninitialized,
}

impl From<&str> for Command {
    fn from(s: &str) -> Self {
        match s {
            "ping" => Command::PING,
            "echo" => Command::ECHO,
            "get" => Command::Get,
            "set" => Command::SET,
            "del" => Command::DELETE,
            "exists" => Command::EXISTS,
            _ => Command::Uninitialized,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::Cache;
    use crate::resp::value::Value::Null;

    #[tokio::test]
    async fn test_ping_command() -> Result<()> {
        let mut handler = Handler::new(Value::Array(vec![Value::BulkString("PING".to_string())]));
        let cache = Arc::new(Cache::default());
        let response = handler.handle_request(&cache).await?;
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
        let response = handler.handle_request(&cache).await?;
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

        let response = handler.handle_request(&cache).await?;
        assert_eq!(response, Value::Null);

        cache.set("key".to_string(), "value".to_string()).await;
        let response = handler.handle_request(&cache).await?;
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

        let response = handler.handle_request(&cache).await?;
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
        let response = handler.handle_request(&cache).await?;
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
        let response = handler.handle_request(&cache).await?;
        assert_eq!(response, Value::SimpleString("OK".to_string()));
        assert_eq!(cache.get("key".to_string()).await, None);

        Ok(())
    }

    #[tokio::test]
    async fn test_del_command() -> Result<()> {
        let cache = Arc::new(Cache::default());
        cache.set("key".to_string(), "value".to_string()).await;

        let mut handler = Handler::new(Value::Array(vec![
            Value::BulkString("get".to_string()),
            Value::BulkString("key".to_string())
        ]));

        let response = handler.handle_request(&cache).await?;
        assert_eq!(response, Value::SimpleString("value".to_string()));

        let mut handler = Handler::new(Value::Array(vec![
            Value::BulkString("del".to_string()),
            Value::BulkString("key".to_string())
        ]));

        let response = handler.handle_request(&cache).await?;
        assert_eq!(response, Value::SimpleString("OK".to_string()));

        let mut handler = Handler::new(Value::Array(vec![
            Value::BulkString("get".to_string()),
            Value::BulkString("key".to_string())
        ]));

        let response = handler.handle_request(&cache).await?;
        assert_eq!(response, Null);

        Ok(())
    }

    #[tokio::test]
    async fn test_exists_command() -> Result<()> {
        let cache = Arc::new(Cache::default());
        cache.set("key".to_string(), "value".to_string()).await;

        let mut handler = Handler::new(Value::Array(vec![
            Value::BulkString("exists".to_string()),
            Value::BulkString("key".to_string())
        ]));

        let response = handler.handle_request(&cache).await?;
        assert_eq!(response, Value::SimpleString("true".to_string()));

        let mut handler = Handler::new(Value::Array(vec![
            Value::BulkString("exists".to_string()),
            Value::BulkString("key1".to_string())
        ]));

        let response = handler.handle_request(&cache).await?;
        assert_eq!(response, Value::SimpleString("false".to_string()));

        Ok(())
    }

    #[test]
    fn test_command_from_str() {
        assert_eq!(Command::from("ping"), Command::PING);
        assert_eq!(Command::from("echo"), Command::ECHO);
        assert_eq!(Command::from("get"), Command::Get);
        assert_eq!(Command::from("set"), Command::SET);
        assert_eq!(Command::from("del"), Command::DELETE);
        assert_eq!(Command::from("exists"), Command::EXISTS);
        assert_eq!(Command::from("unknown"), Command::Uninitialized);
    }
}
