//! Cache mapping content hashes to Telegram file IDs.
//!
//! When a file is sent, the returned `file_id` is cached keyed by a hash of
//! the [`crate::types::FileSource`] content.  Next time the same content needs sending, the
//! cached `file_id` is used — instant delivery, no re-upload required.
//!
//! # Examples
//!
//! ```
//! use blazegram::file_cache::FileIdCache;
//! use blazegram::types::FileSource;
//!
//! let cache = FileIdCache::new();
//! let source = FileSource::Url("https://example.com/photo.jpg".into());
//!
//! // First time: no hit
//! assert!(cache.get(&source).is_none());
//!
//! // After sending, store the file_id telegram returned
//! cache.put(&source, "AgACAgIAAxk...".into());
//!
//! // Second time: cache hit returns a FileId variant
//! let cached = cache.get(&source).unwrap();
//! assert!(matches!(cached, FileSource::FileId(_)));
//! ```

use std::hash::{Hash, Hasher};

use dashmap::DashMap;

use crate::types::FileSource;

/// In-memory file-ID cache.
///
/// Thread-safe (backed by [`DashMap`]).  Typically one instance lives for the
/// lifetime of the bot.
pub struct FileIdCache {
    /// content_hash → Telegram file_id
    cache: DashMap<u64, String>,
}

impl FileIdCache {
    /// Create an empty cache.
    pub fn new() -> Self {
        Self {
            cache: DashMap::new(),
        }
    }

    /// Look up a cached file‑id for `source`.
    ///
    /// Returns `Some(FileSource::FileId(…))` on a cache hit, or `None`.
    #[must_use]
    pub fn get(&self, source: &FileSource) -> Option<FileSource> {
        // FileId sources are already lightweight — no need to cache-redirect
        if matches!(source, FileSource::FileId(_)) {
            return Some(source.clone());
        }

        let h = hash_source(source);
        self.cache
            .get(&h)
            .map(|entry| FileSource::FileId(entry.value().clone()))
    }

    /// Store a mapping from `source` → `file_id`.
    ///
    /// Only meaningful for non-FileId sources (Url, LocalPath, Bytes).
    pub fn put(&self, source: &FileSource, file_id: String) {
        if matches!(source, FileSource::FileId(_)) {
            return;
        }
        let h = hash_source(source);
        self.cache.insert(h, file_id);
    }

    /// Number of entries in the cache.
    #[must_use]
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Whether the cache is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    /// Remove all entries.
    pub fn clear(&self) {
        self.cache.clear();
    }

    /// Remove the entry for a specific source, if present.
    pub fn remove(&self, source: &FileSource) -> Option<String> {
        let h = hash_source(source);
        self.cache.remove(&h).map(|(_, v)| v)
    }
}

impl Default for FileIdCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Hash a [`crate::types::FileSource`] using the standard library hasher.
fn hash_source(source: &FileSource) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    source.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_new_cache_is_empty() {
        let c = FileIdCache::new();
        assert_eq!(c.len(), 0);
        assert!(c.is_empty());
    }

    #[test]
    fn test_put_and_get_url() {
        let c = FileIdCache::new();
        let src = FileSource::Url("https://example.com/img.png".into());
        c.put(&src, "AgACAgI_abc".into());

        let hit = c.get(&src);
        assert!(hit.is_some());
        match hit.unwrap() {
            FileSource::FileId(id) => assert_eq!(id, "AgACAgI_abc"),
            other => panic!("expected FileId, got {:?}", other),
        }
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn test_put_and_get_local_path() {
        let c = FileIdCache::new();
        let src = FileSource::LocalPath(PathBuf::from("/tmp/photo.jpg"));
        c.put(&src, "file123".into());

        assert!(c.get(&src).is_some());
    }

    #[test]
    fn test_put_and_get_bytes() {
        let c = FileIdCache::new();
        let src = FileSource::Bytes {
            data: vec![0xFF, 0xD8, 0xFF],
            filename: "pic.jpg".into(),
        };
        c.put(&src, "bytes_fid".into());

        let hit = c.get(&src).unwrap();
        assert!(matches!(hit, FileSource::FileId(ref id) if id == "bytes_fid"));
    }

    #[test]
    fn test_miss_returns_none() {
        let c = FileIdCache::new();
        let src = FileSource::Url("https://example.com/nope.png".into());
        assert!(c.get(&src).is_none());
    }

    #[test]
    fn test_file_id_passthrough() {
        let c = FileIdCache::new();
        let src = FileSource::FileId("already_cached".into());

        // get on a FileId source returns it directly, no cache write needed
        let hit = c.get(&src).unwrap();
        assert!(matches!(hit, FileSource::FileId(ref id) if id == "already_cached"));
        assert_eq!(c.len(), 0); // nothing stored
    }

    #[test]
    fn test_put_file_id_is_noop() {
        let c = FileIdCache::new();
        let src = FileSource::FileId("fid".into());
        c.put(&src, "other".into());
        assert_eq!(c.len(), 0);
    }

    #[test]
    fn test_overwrite() {
        let c = FileIdCache::new();
        let src = FileSource::Url("https://example.com/img.png".into());
        c.put(&src, "v1".into());
        c.put(&src, "v2".into());

        let hit = c.get(&src).unwrap();
        assert!(matches!(hit, FileSource::FileId(ref id) if id == "v2"));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn test_clear() {
        let c = FileIdCache::new();
        c.put(&FileSource::Url("https://a.com".into()), "a".into());
        c.put(&FileSource::Url("https://b.com".into()), "b".into());
        assert_eq!(c.len(), 2);

        c.clear();
        assert_eq!(c.len(), 0);
        assert!(c.is_empty());
    }

    #[test]
    fn test_remove() {
        let c = FileIdCache::new();
        let src = FileSource::Url("https://example.com/img.png".into());
        c.put(&src, "fid".into());
        assert_eq!(c.len(), 1);

        let removed = c.remove(&src);
        assert_eq!(removed.as_deref(), Some("fid"));
        assert!(c.is_empty());
    }

    #[test]
    fn test_remove_missing() {
        let c = FileIdCache::new();
        let src = FileSource::Url("https://example.com/nope.png".into());
        assert!(c.remove(&src).is_none());
    }

    #[test]
    fn test_different_sources_different_keys() {
        let c = FileIdCache::new();
        let url = FileSource::Url("https://example.com/img.png".into());
        let path = FileSource::LocalPath(PathBuf::from("https://example.com/img.png"));

        c.put(&url, "url_fid".into());
        c.put(&path, "path_fid".into());

        // They should be separate entries (different discriminant in Hash impl)
        let url_hit = c.get(&url).unwrap();
        let path_hit = c.get(&path).unwrap();
        assert!(matches!(url_hit, FileSource::FileId(ref id) if id == "url_fid"));
        assert!(matches!(path_hit, FileSource::FileId(ref id) if id == "path_fid"));
    }
}
