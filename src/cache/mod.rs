pub mod expiry;
mod entry;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::RwLock;
use std::time::{Duration, Instant};
use anyhow::{Error, Result};
use std::cmp;
use crate::cache::entry::Entry;
use crate::cache::expiry::Expiry;
use rand::prelude::*;
use async_timer::Interval;

#[derive(Debug)]
pub struct Cache {
    store: RwLock<BTreeMap<String, Entry>>,
    sample: usize,
    threshold: f64,
    frequency: Duration
}

impl Cache {
    pub fn new(sample: usize, threshold: f64, frequency: Duration) -> Self {
        Cache {
            store: RwLock::new(BTreeMap::new()),
            sample,
            threshold,
            frequency,
        }
    }

    pub async fn expired(&self) -> usize {
        let store = self.store.read().unwrap();
        store.iter().filter(|(_, entry)| entry.expiration().is_expired()).count()
    }

    pub async fn clear(&self) {
        let mut store = self.store.write().unwrap();
        store.clear();
    }

    pub async fn len(&self) -> usize {
        let store = self.store.read().unwrap();
        store.len()
    }

    pub async fn is_empty(&self) -> bool {
        let store = self.store.read().unwrap();
        return store.is_empty()
    }

    pub async fn set(&self, key: String, value: String) {
        let expiry = Expiry::none();
        let entry = Entry::new(value, expiry);
        let mut store = self.store.write().unwrap();
        store.insert(key, entry);
    }

    pub async fn set_with_expiry<E>(&self, key: String, value: String, e: E)
        where
            E: Into<Expiry>,
    {
        let entry = Entry::new(value, e.into());
        let mut store = self.store.write().unwrap();
        store.insert(key, entry);
    }

    pub async fn get(&self, key: String) -> Option<String> {
        let mut store = self.store.read().unwrap();
        match store.get(key.as_str()) {
            Some(entry) => {
                if !entry.expiration().is_expired() {
                    Some(entry.value().clone())
                } else {
                    drop(store);
                    self.remove(key);
                    None
                }
            }
            None => None,
        }
    }

    pub async fn remove(&self, key: String) -> Result<()> {
        let mut store = self.store.write().unwrap();
        match store.get(key.as_str()) {
            Some(_entry) => {
                store.remove(key.as_str());
                Ok(())
            }
            _ => {
                Err(Error::msg(format!("Error in removing entry with key {:?}", key)))
            },
        }
    }

    pub async fn existing(&self) -> usize {
        let store = self.store.read().unwrap();
        store.iter().filter(|(_, entry)| !entry.expiration().is_expired()).count()
    }

    pub async fn remove_garbage(&self) {
        println!("garbage collection started");
        let frequency = self.frequency;
        let mut interval = Interval::platform_new(frequency);
        loop {
            interval.as_mut().await;
            self.purge().await;
        }
    }

    pub async fn purge(&self) {

        let sample = self.sample;
        let threshold = self.threshold;

        let start = Instant::now();

        let locked = Duration::from_nanos(0);
        let mut removed = 0;

        loop {
            let keys_to_remove: Vec<String> = {
                let store = self.store.read().unwrap();

                if store.is_empty() {
                    break;
                }

                let total = store.len();
                let sample = cmp::min(sample, total);

                let mut gone = 0;

                let mut expired_keys = Vec::with_capacity(sample);
                let mut indices: BTreeSet<usize> = BTreeSet::new();

                let mut rng = rand::thread_rng();
                while indices.len() < sample {
                    indices.insert(rng.gen_range(0..total));
                }

                let mut iter = store.iter();

                for _ in 0..sample {
                    let (key, entry) = iter.next().unwrap();

                    if !entry.expiration().is_expired() {
                        continue;
                    }

                    expired_keys.push(key.clone());
                    gone += 1;
                }

                if (gone as f64) < (sample as f64 * threshold) {
                    break;
                }
                expired_keys
            };

            if keys_to_remove.is_empty() {
                break;
            }

            let mut store = self.store.write().unwrap();

            for key in &keys_to_remove {
                store.remove(key);
            }

            println!("Removed {} / {} ({:.2}%) of the sampled keys", keys_to_remove.len(), sample, (keys_to_remove.len() as f64 / sample as f64) * 100f64);

            removed += keys_to_remove.len();

            if (keys_to_remove.len() as f64) < (sample as f64 * threshold) {
                break;
            }
        }
        println!("Purge loop removed {} entries in {:.0?} ({:.0?} locked)", removed, start.elapsed(), locked);
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;
    use std::sync::Arc;
    use async_std::task;

    #[tokio::test]
    async fn test_set_get() {
        let cache = Cache::new(25, 0.25, Duration::from_secs(1));
        let key = "key".to_string();
        let value = "value".to_string();

        cache.set(key.clone(), value.clone()).await;
        let result = cache.get(key.clone()).await;
        assert_eq!(result, Some(value.clone()));
    }

    #[tokio::test]
    async fn test_expired_is_zero() {
        let cache = Cache::new(25, 0.25, Duration::from_secs(1));
        let expiry = Expiry::new(Instant::now()  + Duration::from_secs(2));
        cache.set_with_expiry("key".to_string(), "value".to_string(), expiry.clone()).await;
        let count = cache.expired().await;
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_expired_is_one() {
        let cache = Cache::new(25, 0.25, Duration::from_secs(1));
        let expiry = Expiry::new(Instant::now());
        cache.set_with_expiry("key".to_string(), "value".to_string(), expiry.clone()).await;
        let count = cache.expired().await;
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn test_len() {
        let cache = Cache::new(25, 0.25, Duration::from_secs(1));
        let expiry = Expiry::new(Instant::now());
        cache.set_with_expiry("key".to_string(), "value".to_string(), expiry.clone()).await;
        assert_eq!(cache.len().await, 1);
    }

    #[tokio::test]
    async fn test_clear() {
        let cache = Cache::new(25, 0.25, Duration::from_secs(1));
        let expiry = Expiry::new(Instant::now());
        cache.set_with_expiry("key".to_string(), "value".to_string(), expiry.clone()).await;
        cache.clear().await;
        assert_eq!(cache.len().await, 0);
    }

    #[tokio::test]
    async fn test_is_empty() {
        let cache = Cache::new(25, 0.25, Duration::from_secs(1));
        let expiry = Expiry::new(Instant::now());
        cache.set_with_expiry("key".to_string(), "value".to_string(), expiry.clone()).await;
        cache.clear().await;
        assert!(cache.is_empty().await);
    }

    #[tokio::test]
    async fn test_set_with_expiry_get() {
        let cache = Cache::new(25, 0.25, Duration::from_secs(1));
        let key = "key".to_string();
        let value = "value".to_string();

        let expiry = Expiry::new(Instant::now() + Duration::from_secs(2));
        cache.set_with_expiry(key.clone(), value.clone(), expiry.clone()).await;
        let result = cache.get(key.clone()).await;
        assert_eq!(result, Some(value.clone()));

        // Wait for expiration
        sleep(Duration::from_secs(3));

        let result_after_expiry = cache.get(key.clone()).await;
        assert_eq!(result_after_expiry, None);
    }

    #[tokio::test]
    async fn test_set_with_expiry_get_non_expired() {
        let cache = Cache::new(25, 0.25, Duration::from_secs(1));
        let key = "key".to_string();
        let value = "value".to_string();

        let expiry = Expiry::new(Instant::now() + Duration::from_secs(5));
        cache.set_with_expiry(key.clone(), value.clone(), expiry.clone()).await;
        let result = cache.get(key.clone()).await;
        assert_eq!(result, Some(value.clone()));
    }

    #[tokio::test]
    async fn test_set_with_expiry_update_expiry() {
        let cache = Cache::new(25, 0.25, Duration::from_secs(1));
        let key = "key".to_string();
        let value = "value".to_string();

        let expiry1 = Expiry::new(Instant::now() + Duration::from_secs(2));
        cache.set_with_expiry(key.clone(), value.clone(), expiry1.clone()).await;
        let result1 = cache.get(key.clone()).await;
        assert_eq!(result1, Some(value.clone()));

        // Update the expiry
        let expiry2 = Expiry::new(Instant::now() + Duration::from_secs(5));
        cache.set_with_expiry(key.clone(), value.clone(), expiry2.clone()).await;
        let result2 = cache.get(key.clone()).await;
        assert_eq!(result2, Some(value.clone()));

        // Wait for expiration of the original expiry
        sleep(Duration::from_secs(3));

        // After original expiry, the value should still be present due to updated expiry
        let result3 = cache.get(key.clone()).await;
        assert_eq!(result3, Some(value.clone()));
    }

    #[tokio::test]
    async fn test_get_non_existing_key() {
        let cache = Cache::new(25, 0.25, Duration::from_secs(1));
        let result = cache.get("non_existing_key".to_string()).await;
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_remove() {
        let cache = Cache::new(25, 0.25, Duration::from_secs(1));
        let key = "key".to_string();
        let value = "value".to_string();
        cache.set(key.clone(), value.clone()).await;
        let result = cache.get(key.clone()).await.unwrap();
        assert_eq!(result, value.to_string());
        _ = cache.remove(key.clone()).await;
        let result = cache.get(key.clone()).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_remove_key_doesnt_exist() {
        let cache = Cache::new(25, 0.25, Duration::from_secs(1));
        let key = "key".to_string();
        let result = cache.remove(key.clone()).await;
        assert!(result.is_err())
    }

    #[tokio::test]
    async fn test_existing_is_one() {
        let cache = Cache::new(25, 0.25, Duration::from_secs(1));
        let expiry = Expiry::new(Instant::now()  + Duration::from_secs(2));
        cache.set_with_expiry("key".to_string(), "value".to_string(), expiry.clone()).await;
        let count = cache.existing().await;
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn test_purge_empty_cache() {
        let cache = Cache::new(10, 0.5, Duration::from_secs(1));
        cache.purge().await;
        assert_eq!(cache.len().await, 0);
    }

    #[tokio::test]
    async fn test_purge_expired_keys() {
        let cache = Cache::new(10, 0.5, Duration::from_millis(1));
        cache.set_with_expiry("key1".to_string(), "value1".to_string(), Duration::from_secs(1)).await;
        cache.set_with_expiry("key2".to_string(), "value2".to_string(), Duration::from_secs(2)).await;
        cache.set_with_expiry("key3".to_string(), "value3".to_string(), Duration::from_secs(3)).await;
        sleep(Duration::from_secs(2));
        cache.purge().await;
        assert_eq!(cache.len().await, 1);
        assert_eq!(cache.get("key1".to_string()).await, None);
        assert_eq!(cache.get("key2".to_string()).await, None);
        assert_eq!(cache.get("key3".to_string()).await, Some("value3".to_string()));
    }

    #[tokio::test]
    async fn test_purge_all_expired_entries() {
        let cache = Cache::new(2, 0.5, Duration::from_secs(1));
        let key1 = "key1".to_string();
        let key2 = "key2".to_string();

        cache.set_with_expiry(key1.clone(), "value1".to_string(), Duration::from_secs(0)).await;
        cache.set_with_expiry(key2.clone(), "value2".to_string(), Duration::from_secs(0)).await;

        // wait for the entries to expire
        sleep(Duration::from_millis(100));

        // purge all expired entries
        cache.purge().await;

        assert_eq!(cache.len().await, 0);
    }

    #[tokio::test]
    async fn test_purge_some_expired_entries() {
        let cache = Cache::new(3, 0.5, Duration::from_secs(1));
        let key1 = "key1".to_string();
        let key2 = "key2".to_string();
        let key3 = "key3".to_string();

        cache.set_with_expiry(key1.clone(), "value1".to_string(), Duration::from_secs(0)).await;
        cache.set_with_expiry(key2.clone(), "value2".to_string(), Duration::from_secs(0)).await;
        cache.set_with_expiry(key3.clone(), "value3".to_string(), Duration::from_secs(60)).await;

        // wait for the entries to expire
        sleep(Duration::from_millis(100));

        // purge a percentage of expired entries
        cache.purge().await;

        // only the entry with key3 should remain
        assert_eq!(cache.len().await, 1);
        assert_eq!(cache.get(key1.clone()).await, None);
        assert_eq!(cache.get(key2.clone()).await, None);
        assert_eq!(cache.get(key3.clone()).await, Some("value3".to_string()));
    }

    #[async_std::test]
    async fn test_monitor() {
        let cache = Arc::new(Cache::new(10, 0.5, Duration::from_millis(100)));
        let clone = cache.clone();
        // Insert some values with an expiry time of 3 seconds
        cache.set_with_expiry("key1".to_string(), "value1".to_string(), Duration::from_secs(3)).await;
        cache.set_with_expiry("key2".to_string(), "value2".to_string(), Duration::from_secs(2)).await;
        cache.set_with_expiry("key3".to_string(), "value1".to_string(), Duration::from_secs(1)).await;
        cache.set_with_expiry("key4".to_string(), "value2".to_string(), Duration::from_secs(1)).await;
        cache.set_with_expiry("key5".to_string(), "value1".to_string(), Duration::from_secs(2)).await;
        cache.set_with_expiry("key6".to_string(), "value2".to_string(), Duration::from_secs(2)).await;
        cache.set_with_expiry("key7".to_string(), "value1".to_string(), Duration::from_secs(3)).await;
        cache.set_with_expiry("key8".to_string(), "value2".to_string(), Duration::from_secs(2)).await;
        cache.set_with_expiry("key9".to_string(), "value1".to_string(), Duration::from_secs(3)).await;
        cache.set_with_expiry("key11".to_string(), "value1".to_string(), Duration::from_secs(3)).await;
        cache.set_with_expiry("key12".to_string(), "value2".to_string(), Duration::from_secs(2)).await;
        cache.set_with_expiry("key13".to_string(), "value1".to_string(), Duration::from_secs(1)).await;
        cache.set_with_expiry("key14".to_string(), "value2".to_string(), Duration::from_secs(1)).await;
        cache.set_with_expiry("key15".to_string(), "value1".to_string(), Duration::from_secs(2)).await;
        cache.set_with_expiry("key16".to_string(), "value2".to_string(), Duration::from_secs(2)).await;
        cache.set_with_expiry("key17".to_string(), "value1".to_string(), Duration::from_secs(3)).await;
        cache.set_with_expiry("key18".to_string(), "value2".to_string(), Duration::from_secs(2)).await;
        cache.set_with_expiry("key19".to_string(), "value1".to_string(), Duration::from_secs(4)).await;
        cache.set_with_expiry("key21".to_string(), "value1".to_string(), Duration::from_secs(3)).await;
        cache.set_with_expiry("key22".to_string(), "value2".to_string(), Duration::from_secs(2)).await;
        cache.set_with_expiry("key23".to_string(), "value1".to_string(), Duration::from_secs(1)).await;
        cache.set_with_expiry("key24".to_string(), "value2".to_string(), Duration::from_secs(1)).await;
        cache.set_with_expiry("key25".to_string(), "value1".to_string(), Duration::from_secs(2)).await;
        cache.set_with_expiry("key26".to_string(), "value2".to_string(), Duration::from_secs(2)).await;
        cache.set_with_expiry("key27".to_string(), "value1".to_string(), Duration::from_secs(4)).await;
        cache.set_with_expiry("key28".to_string(), "value2".to_string(), Duration::from_secs(2)).await;
        cache.set_with_expiry("key29".to_string(), "value1".to_string(), Duration::from_secs(3)).await;
        cache.set_with_expiry("key31".to_string(), "value1".to_string(), Duration::from_secs(3)).await;
        cache.set_with_expiry("key32".to_string(), "value2".to_string(), Duration::from_secs(2)).await;
        cache.set_with_expiry("key33".to_string(), "value1".to_string(), Duration::from_secs(1)).await;
        cache.set_with_expiry("key34".to_string(), "value2".to_string(), Duration::from_secs(1)).await;
        cache.set_with_expiry("key35".to_string(), "value1".to_string(), Duration::from_secs(4)).await;
        cache.set_with_expiry("key36".to_string(), "value2".to_string(), Duration::from_secs(2)).await;
        cache.set_with_expiry("key37".to_string(), "value1".to_string(), Duration::from_secs(3)).await;
        cache.set_with_expiry("key38".to_string(), "value2".to_string(), Duration::from_secs(2)).await;
        cache.set_with_expiry("key39".to_string(), "value1".to_string(), Duration::from_secs(3)).await;
        cache.set_with_expiry("key10".to_string(), "value2".to_string(), Duration::from_secs(7)).await;

        task::spawn(async move {
            clone.remove_garbage().await;
        });

        // Sleep for 5 seconds to allow the monitoring task to run
        sleep(Duration::from_secs(5));

        // Check that the expired keys were removed
        assert_eq!(1, cache.len().await);
    }

}
