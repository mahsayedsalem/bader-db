pub mod expiry;
mod entry;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::RwLock;
use std::time::{Duration, Instant};
use anyhow::{Error, Result};
use std::cmp;
use rand::prelude::*;
use async_timer::Interval;
use tokio::sync::mpsc;
use crate::cache::entry::Entry;
use crate::cache::expiry::Expiry;
use crate::server::shutdown::Shutdown;

#[derive(Debug)]
pub struct Cache {
    store: RwLock<BTreeMap<String, Entry>>,
    sample: usize,
    threshold: f64,
    frequency: Duration,
    shutdown: Option<Shutdown>,
    _shutdown_complete: Option<mpsc::Sender<()>>,
}

impl Cache {
    pub fn new(sample: usize, threshold: f64, frequency: Duration, shutdown: Option<Shutdown>, _shutdown_complete: Option<mpsc::Sender<()>>) -> Self {
        Cache {
            store: RwLock::new(BTreeMap::new()),
            sample,
            threshold,
            frequency,
            shutdown,
            _shutdown_complete
        }
    }

    pub async fn set(&self, key: String, value: String) {
        let expiry = Expiry::none();
        let entry = Entry::new(value, expiry);

        log::debug!("inserting key {} and value {:?}", key.clone(), entry);

        let mut store = self.store.write().unwrap();
        store.insert(key, entry);
    }

    pub async fn set_with_expiry<E>(&self, key: String, value: String, e: E)
        where
            E: Into<Expiry>,
    {
        let entry = Entry::new(value, e.into());

        log::debug!("inserting key {} and value {:?}", key.clone(), entry);

        let mut store = self.store.write().unwrap();
        store.insert(key, entry);
    }

    pub async fn get(&self, key: String) -> Option<String> {
        let store = self.store.read().unwrap();
        match store.get(key.as_str()) {
            Some(entry) => {

                log::debug!("getting key {} and value {:?}", key.clone(), entry);

                if !entry.expiration().is_expired() {
                    Some(entry.value().clone())
                } else {
                    drop(store);
                    let mut store = self.store.write().unwrap();
                    store.remove(key.as_str());
                    None
                }
            }
            None => None,
        }
    }

    pub async fn remove(&self, key: String) -> Result<()> {
        let mut store = self.store.write().unwrap();
        match store.get(key.as_str()) {
            Some(entry) => {
                log::debug!("removing key {} and value {:?}", key.clone(), entry);
                store.remove(key.as_str());
                Ok(())
            }
            _ => {
                Err(Error::msg(format!("Error in removing entry with key {:?}", key)))
            },
        }
    }

    pub async fn exists(&self, key: String) -> bool {
        let mut store = self.store.write().unwrap();
        store.contains_key(key.as_str())
    }

    pub async fn monitor_for_expiry(&self) {

        log::debug!("removing garbage in the background");

        let frequency = self.frequency;
        let mut interval = Interval::platform_new(frequency);
        while self.shutdown.is_none() || !self.shutdown.as_ref().unwrap().is_shutdown()  {
            interval.as_mut().await;
            self.purge().await;
        }
    }

    pub async fn purge(&self) {
        let start = Instant::now();
        log::debug!("purging is starting in {:?}", start);

        let sample = self.sample;
        let threshold = self.threshold;
        let mut total = 0usize;
        let mut locked = Duration::from_nanos(0);
        let mut removed = 0;

        loop {

            let store = self.store.read().unwrap();

            if store.is_empty() {
                break;
            }

            total = store.len();
            let sample = cmp::min(sample, total);

            let mut gone = 0;

            let mut expired_keys = Vec::with_capacity(sample);
            let mut indices: BTreeSet<usize> = BTreeSet::new();

            {
                // fetch `sample` keys at random
                let mut rng = rand::thread_rng();
                while indices.len() < sample {
                    indices.insert(rng.gen_range(0..total));
                }
            }

            {
                // tracker for previous index
                let mut prev = 0;

                // boxed iterator to allow us to iterate a single time for all indices
                let mut iter: Box<dyn Iterator<Item = (&String, &Entry)>> =
                    Box::new(store.iter());

                // walk our index list
                for idx in indices {
                    // calculate how much we need to shift the iterator
                    let offset = idx
                        .checked_sub(prev)
                        .and_then(|idx| idx.checked_sub(1))
                        .unwrap_or(0);

                    // shift and mark the current index
                    iter = Box::new(iter.skip(offset));
                    prev = idx;

                    // fetch the next pair (at our index)
                    let (key, entry) = iter.next().unwrap();

                    // skip if not expired
                    if !entry.expiration().is_expired() {
                        continue;
                    }

                    // otherwise mark for removal
                    expired_keys.push(key.to_owned());

                    // and increment remove count
                    gone += 1;
                }
            }

            {
                // drop the read lock
                drop(store);

                // upgrade to a write guard so that we can make our changes
                let acquired = Instant::now();

                let mut store = self.store.write().unwrap();

                // remove all expired keys
                for key in &expired_keys {
                    store.remove(key);
                }

                // increment the lock timer tracking directly
                locked = locked.checked_add(acquired.elapsed()).unwrap();
            }

            log::debug!("Removed {} / {} ({:.2}%) of the sampled keys", gone, sample, (gone as f64 / sample as f64) * 100f64);

            removed += gone;

            if (gone as f64) < (sample as f64 * threshold) {
                break;
            }
        }
        log::debug!("Purge loop removed {} entries out of {} in {:.0?} ({:.0?} locked)", removed, total, start.elapsed(), locked);
    }

    pub async fn len(&self) -> usize {
        let store = self.store.read().unwrap();
        store.len()
    }

    pub async fn is_empty(&self) -> bool {
        let store = self.store.read().unwrap();
        return store.is_empty()
    }

    pub async fn existing(&self) -> usize {
        let store = self.store.read().unwrap();
        store.iter().filter(|(_, entry)| !entry.expiration().is_expired()).count()
    }

    pub async fn expired(&self) -> usize {
        let store = self.store.read().unwrap();
        store.iter().filter(|(_, entry)| entry.expiration().is_expired()).count()
    }

    pub async fn clear(&self) {
        let mut store = self.store.write().unwrap();
        store.clear();
    }

}

impl Default for Cache {
    fn default() -> Cache {
        Cache::new(25, 0.25, Duration::from_secs(1), None, None)
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
        let cache = Cache::default();
        let key = "key".to_string();
        let value = "value".to_string();

        cache.set(key.clone(), value.clone()).await;
        let result = cache.get(key.clone()).await;
        assert_eq!(result, Some(value.clone()));
    }

    #[tokio::test]
    async fn test_expired_is_zero() {
        let cache = Cache::default();
        let expiry = Expiry::new(Instant::now()  + Duration::from_secs(2));
        cache.set_with_expiry("key".to_string(), "value".to_string(), expiry.clone()).await;
        let count = cache.expired().await;
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_expired_is_one() {
        let cache = Cache::default();
        let expiry = Expiry::new(Instant::now());
        cache.set_with_expiry("key".to_string(), "value".to_string(), expiry.clone()).await;
        let count = cache.expired().await;
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn test_len() {
        let cache = Cache::default();
        let expiry = Expiry::new(Instant::now());
        cache.set_with_expiry("key".to_string(), "value".to_string(), expiry.clone()).await;
        assert_eq!(cache.len().await, 1);
    }

    #[tokio::test]
    async fn test_clear() {
        let cache = Cache::default();
        let expiry = Expiry::new(Instant::now());
        cache.set_with_expiry("key".to_string(), "value".to_string(), expiry.clone()).await;
        cache.clear().await;
        assert_eq!(cache.len().await, 0);
    }

    #[tokio::test]
    async fn test_is_empty() {
        let cache = Cache::default();
        let expiry = Expiry::new(Instant::now());
        cache.set_with_expiry("key".to_string(), "value".to_string(), expiry.clone()).await;
        cache.clear().await;
        assert!(cache.is_empty().await);
    }

    #[tokio::test]
    async fn test_set_with_expiry_get() {
        let cache = Cache::default();
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
        let cache = Cache::default();
        let key = "key".to_string();
        let value = "value".to_string();

        let expiry = Expiry::new(Instant::now() + Duration::from_secs(5));
        cache.set_with_expiry(key.clone(), value.clone(), expiry.clone()).await;
        let result = cache.get(key.clone()).await;
        assert_eq!(result, Some(value.clone()));
    }

    #[tokio::test]
    async fn test_set_with_expiry_update_expiry() {
        let cache = Cache::default();
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
        let cache = Cache::default();
        let result = cache.get("non_existing_key".to_string()).await;
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_remove() {
        let cache = Cache::default();
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
        let cache = Cache::default();
        let key = "key".to_string();
        let result = cache.remove(key.clone()).await;
        assert!(result.is_err())
    }

    #[tokio::test]
    async fn test_existing_is_one() {
        let cache = Cache::default();
        let expiry = Expiry::new(Instant::now()  + Duration::from_secs(2));
        cache.set_with_expiry("key".to_string(), "value".to_string(), expiry.clone()).await;
        let count = cache.existing().await;
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn test_purge_empty_cache() {
        let cache = Cache::new(10, 0.5, Duration::from_secs(1), None, None);
        cache.purge().await;
        assert_eq!(cache.len().await, 0);
    }

    #[tokio::test]
    async fn test_purge_expired_keys() {
        let cache = Cache::new(10, 0.5, Duration::from_millis(1), None, None);
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
    async fn test_expiry_formats() {
        let cache = Cache::new(10, 0.5, Duration::from_millis(1), None, None);
        cache.set_with_expiry("key1".to_string(), "value1".to_string(), (10u64, &"PX".to_string())).await;
        cache.set_with_expiry("key2".to_string(), "value2".to_string(), (1u64, &"EX".to_string())).await;
        cache.set_with_expiry("key3".to_string(), "value3".to_string(), (3u64, &"EX".to_string())).await;
        sleep(Duration::from_secs(2));
        cache.purge().await;
        assert_eq!(cache.len().await, 1);
        assert_eq!(cache.get("key1".to_string()).await, None);
        assert_eq!(cache.get("key2".to_string()).await, None);
        assert_eq!(cache.get("key3".to_string()).await, Some("value3".to_string()));
    }

    #[tokio::test]
    async fn test_purge_all_expired_entries() {
        let cache = Cache::new(2, 0.5, Duration::from_secs(1), None, None);
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
        let cache = Cache::new(3, 0.5, Duration::from_secs(1), None, None);
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
        let cache = Arc::new(Cache::new(10, 0.5, Duration::from_millis(100), None, None));
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
        cache.set_with_expiry("key39".to_string(), "value1".to_string(), Duration::from_secs(6)).await;
        cache.set_with_expiry("key10".to_string(), "value2".to_string(), Duration::from_secs(7)).await;

        task::spawn(async move {
            clone.monitor_for_expiry().await;
        });

        // Sleep for 5 seconds to allow the monitoring task to run
        sleep(Duration::from_secs(5));

        // Check that the expired keys were removed
        assert_eq!(2, cache.len().await);
    }

}
