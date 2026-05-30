//! 极简 LRU 缓存。手写避免引入 `lru` crate 这种小依赖。
//!
//! 实现：`HashMap<K, V>` 存值 + `Vec<K>` 维护"最近访问顺序"。
//! - 访问 O(n) 因为 Vec 要找位置，但 n=容量（256~1024），可接受。
//! - 容量满 + 插入新键时淘汰最久未用的键。
//! - 单线程：通过 [`std::cell::RefCell`] 在 [`super::FsAssetDb`] 内部使用。
//!
//! 不支持并发；如果以后需要在多线程下走，[`Cache`] 改成 `Mutex<...>` 即可。

use std::collections::HashMap;
use std::hash::Hash;

/// 通用 LRU。键需 `Eq + Hash + Clone`，值任意。
pub struct Cache<K: Eq + Hash + Clone, V> {
    cap: usize,
    map: HashMap<K, V>,
    /// 最近访问的 key 在末尾；最久未访问的在头部
    order: Vec<K>,
}

impl<K: Eq + Hash + Clone, V> Cache<K, V> {
    pub fn new(cap: usize) -> Self {
        let cap = cap.max(1);
        Self {
            cap,
            map: HashMap::with_capacity(cap),
            order: Vec::with_capacity(cap),
        }
    }

    pub fn capacity(&self) -> usize {
        self.cap
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    pub fn contains(&self, k: &K) -> bool {
        self.map.contains_key(k)
    }

    /// 命中即提升为最新；未命中返回 None。
    pub fn get(&mut self, k: &K) -> Option<&V> {
        if !self.map.contains_key(k) {
            return None;
        }
        self.touch(k);
        self.map.get(k)
    }

    /// 插入或覆盖。容量满时淘汰最久未用键。返回被淘汰的键值对（如果有）。
    pub fn put(&mut self, k: K, v: V) -> Option<(K, V)> {
        let mut evicted = None;
        if self.map.contains_key(&k) {
            // 覆盖：仅刷新顺序
            self.touch(&k);
            self.map.insert(k, v);
            return None;
        }
        if self.map.len() >= self.cap {
            if let Some(oldest) = self.evict_oldest() {
                evicted = Some(oldest);
            }
        }
        self.order.push(k.clone());
        self.map.insert(k, v);
        evicted
    }

    /// 把 k 移到 order 末尾（最新访问）。仅在已有时调用。
    fn touch(&mut self, k: &K) {
        if let Some(pos) = self.order.iter().position(|x| x == k) {
            // O(n) 移动 — n=cap，可接受
            let key = self.order.remove(pos);
            self.order.push(key);
        }
    }

    fn evict_oldest(&mut self) -> Option<(K, V)> {
        if self.order.is_empty() {
            return None;
        }
        let k = self.order.remove(0);
        let v = self.map.remove(&k)?;
        Some((k, v))
    }

    /// 清空。
    pub fn clear(&mut self) {
        self.map.clear();
        self.order.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn put_get_basic() {
        let mut c: Cache<u32, &'static str> = Cache::new(3);
        c.put(1, "a");
        c.put(2, "b");
        assert_eq!(c.get(&1), Some(&"a"));
        assert_eq!(c.get(&3), None);
        assert_eq!(c.len(), 2);
    }

    #[test]
    fn evicts_least_recently_used() {
        let mut c: Cache<u32, &'static str> = Cache::new(2);
        c.put(1, "a");
        c.put(2, "b");
        // 访问 1，让 2 变成最久未用
        let _ = c.get(&1);
        // 插 3 应当淘汰 2
        let evicted = c.put(3, "c");
        assert_eq!(evicted, Some((2, "b")));
        assert!(c.contains(&1));
        assert!(c.contains(&3));
        assert!(!c.contains(&2));
    }

    #[test]
    fn capacity_at_least_one() {
        let mut c: Cache<u32, &'static str> = Cache::new(0);
        assert!(c.capacity() >= 1);
        c.put(1, "x");
        assert_eq!(c.get(&1), Some(&"x"));
    }

    #[test]
    fn put_existing_does_not_evict() {
        let mut c: Cache<u32, &'static str> = Cache::new(2);
        c.put(1, "a");
        c.put(2, "b");
        let evicted = c.put(1, "a2");
        assert_eq!(evicted, None);
        assert_eq!(c.get(&1), Some(&"a2"));
        assert_eq!(c.len(), 2);
    }

    #[test]
    fn clear_resets() {
        let mut c: Cache<u32, &'static str> = Cache::new(4);
        c.put(1, "a");
        c.put(2, "b");
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.get(&1), None);
    }
}
