use crate::domain::preview::PreviewModel;
use lru::LruCache;
use std::{
    num::NonZeroUsize,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub struct PreviewCache {
    inner: LruCache<PathBuf, PreviewModel>,
}

impl PreviewCache {
    pub fn new(capacity: usize) -> Self {
        let capacity = NonZeroUsize::new(capacity.max(1)).expect("capacity > 0");
        Self {
            inner: LruCache::new(capacity),
        }
    }

    pub fn get(&mut self, path: &Path) -> Option<PreviewModel> {
        self.inner.get(path).cloned()
    }

    pub fn put(&mut self, path: PathBuf, model: PreviewModel) {
        self.inner.put(path, model);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::preview::PreviewModel;

    #[test]
    fn evicts_least_recently_used_entry() {
        let mut cache = PreviewCache::new(1);
        let first = PathBuf::from("/tmp/first");
        let second = PathBuf::from("/tmp/second");

        cache.put(
            first.clone(),
            PreviewModel::Loading {
                path: first.clone(),
            },
        );
        cache.put(
            second.clone(),
            PreviewModel::Loading {
                path: second.clone(),
            },
        );

        assert!(cache.get(&first).is_none());
        assert!(cache.get(&second).is_some());
    }
}
