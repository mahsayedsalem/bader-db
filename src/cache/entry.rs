use crate::cache::expiry::Expiry;

#[derive(Eq, PartialEq, Clone, Debug)]
pub struct Entry {
    value: String,
    expiration: Expiry,
}

impl Entry {
    /// Create a new cache entry from a value and expiration.
    pub fn new(value: String, expiration: Expiry) -> Self {
        Self { value, expiration }
    }

    /// Retrieve the internal expiration.
    pub fn expiration(&self) -> &Expiry {
        &self.expiration
    }

    /// Retrieve the internal value.
    pub fn value(&self) -> &String {
        &self.value
    }

    /// Retrieve the mutable internal value.
    pub fn value_mut(&mut self) -> &mut String {
        &mut self.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn test_new_entry() {
        let value = String::from("test");
        let instant = Instant::now() + Duration::from_secs(5);
        let expiry = Expiry::new(instant);
        let entry = Entry::new(value.clone(), expiry.clone());
        assert_eq!(*entry.value(), value);
        assert_eq!(entry.expiration(), &expiry);
    }

    #[test]
    fn test_getters_and_setters() {
        let value = String::from("test");
        let instant = Instant::now() + Duration::from_secs(5);
        let mut expiry = Expiry::new(instant);
        let mut entry = Entry::new(value.clone(), expiry.clone());

        // Test expiration getter
        assert_eq!(entry.expiration(), &expiry);

        // Test value getter
        assert_eq!(entry.value(), &value);

        // Test value_mut setter and getter
        let new_value = String::from("new_value");
        *entry.value_mut() = new_value.clone();
        assert_eq!(entry.value(), &new_value);

        // Test expiration setter and getter
        let new_instant = Instant::now() + Duration::from_secs(10);
        expiry = Expiry::new(new_instant);
        entry.expiration = expiry.clone();
        assert_eq!(entry.expiration(), &expiry);
    }

    #[test]
    fn test_entry_is_expired() {
        let value = String::from("test");

        // Create an expired entry
        let expired_instant = Instant::now() - Duration::from_secs(5);
        let expired_expiry = Expiry::new(expired_instant);
        let expired_entry = Entry::new(value.clone(), expired_expiry);

        // Create a non-expired entry
        let non_expired_instant = Instant::now() + Duration::from_secs(5);
        let non_expired_expiry = Expiry::new(non_expired_instant);
        let non_expired_entry = Entry::new(value.clone(), non_expired_expiry);

        // Test is_expired() for expired and non-expired entries
        assert!(expired_entry.expiration().is_expired());
        assert!(!non_expired_entry.expiration().is_expired());
    }

    #[test]
    fn test_entry_remaining() {
        let value = String::from("test");

        // Create an entry with expiration in the future
        let future_instant = Instant::now() + Duration::from_secs(5);
        let future_expiry = Expiry::new(future_instant);
        let future_entry = Entry::new(value.clone(), future_expiry);
        assert!(future_entry.expiration().remaining().is_some());

        // Create an entry with expired expiration
        let expired_instant = Instant::now() - Duration::from_secs(5);
        let expired_expiry = Expiry::new(expired_instant);
        let expired_entry = Entry::new(value.clone(), expired_expiry);
        assert_eq!(
            expired_entry.expiration().remaining(),
            Some(Duration::from_nanos(0))
        );
    }
}
