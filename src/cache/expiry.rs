use std::time::{Duration, Instant};

#[derive(Eq, PartialEq, Clone, Debug)]
pub struct Expiry {
    instant: Option<Instant>,
}

impl Expiry {
    pub fn new<I>(instant: I) -> Self
    where
        I: Into<Instant>,
    {
        Self {
            instant: Some(instant.into()),
        }
    }

    /// Create an empty expiration (i.e. no expiration).
    pub fn none() -> Self {
        Self { instant: None }
    }

    /// Retrieve the instant associated with this expiration.
    pub fn instant(&self) -> &Option<Instant> {
        &self.instant
    }

    /// Retrieve whether a cache entry has passed expiration.
    pub fn is_expired(&self) -> bool {
        self.instant()
            .map(|expiration| expiration < Instant::now())
            .unwrap_or(false)
    }

    /// Retrieve the time remaining before expiration.
    pub fn remaining(&self) -> Option<Duration> {
        self.instant
            .map(|i| i.saturating_duration_since(Instant::now()))
    }
}

// Automatic conversation from `Instant`.
impl From<Instant> for Expiry {
    fn from(instant: Instant) -> Self {
        Self::new(instant)
    }
}

// Automatic conversation from `u64`.
impl From<u64> for Expiry {
    fn from(millis: u64) -> Self {
        Duration::from_millis(millis).into()
    }
}

// Automatic conversation from `u64`.
impl From<(u64, &String)> for Expiry {
    fn from(expiry: (u64, &String)) -> Self {
        let amount = expiry.0;
        let format = expiry.1;
        let expiry_type = format.to_ascii_lowercase().as_str().into();
        match expiry_type {
            ExpiryFormat::PX => Duration::from_millis(amount).into(),
            ExpiryFormat::EX => Duration::from_secs(amount).into(),
            _ => Self { instant: None },
        }
    }
}

// Automatic conversation from `Duration`.
impl From<Duration> for Expiry {
    fn from(duration: Duration) -> Self {
        Instant::now().checked_add(duration).unwrap().into()
    }
}

#[derive(Debug, PartialEq)]
pub enum ExpiryFormat {
    EX,
    PX,
    Uninitialized,
}

impl From<&str> for ExpiryFormat {
    fn from(s: &str) -> Self {
        match s {
            "ex" => ExpiryFormat::EX,
            "px" => ExpiryFormat::PX,
            _ => ExpiryFormat::Uninitialized,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let instant = Instant::now();
        let expiry = Expiry::new(instant);
        assert_eq!(expiry.instant(), &Some(instant));
    }

    #[test]
    fn test_none() {
        let expiry = Expiry::none();
        assert_eq!(expiry.instant(), &None);
    }

    #[test]
    fn test_is_expired() {
        let instant = Instant::now() + Duration::from_secs(1);
        let expiry = Expiry::new(instant);
        assert!(!expiry.is_expired());

        let past_instant = Instant::now() - Duration::from_secs(1);
        let expired_expiry = Expiry::new(past_instant);
        assert!(expired_expiry.is_expired());

        let none_expiry = Expiry::none();
        assert!(!none_expiry.is_expired());
    }

    #[test]
    fn test_remaining() {
        let instant = Instant::now() + Duration::from_secs(1);
        let expiry = Expiry::new(instant);
        assert!(expiry.remaining().is_some());

        let past_instant = Instant::now() - Duration::from_secs(3);
        let expired_expiry = Expiry::new(past_instant);
        assert_eq!(expired_expiry.remaining(), Some(Duration::from_nanos(0)));

        let none_expiry = Expiry::none();
        assert!(none_expiry.remaining().is_none());
    }

    #[test]
    fn test_conversions() {
        let instant = Instant::now();
        let expiry_from_instant: Expiry = instant.into();
        assert_eq!(expiry_from_instant.instant(), &Some(instant));

        let millis = 1000;
        let expiry_from_millis: Expiry = millis.into();
        assert!(expiry_from_millis.instant().is_some());

        let duration = Duration::from_secs(1);
        let expiry_from_duration: Expiry = duration.into();
        assert!(expiry_from_duration.instant().is_some());
    }
}
