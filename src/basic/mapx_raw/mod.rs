//!
//! A `Map`-like structure but storing data in disk.
//!
//! NOTE:
//! - Both keys and values will **NOT** be encoded in this structure
//!
//! # Examples
//!
//! ```
//! use vsdb::MapxRaw;
//!
//! let mut l = MapxRaw::new();
//!
//! l.insert(&[1], &[0]);
//! l.insert(&[1], &[0]);
//! l.insert(&[2], &[0]);
//!
//! l.iter().for_each(|(_, v)| {
//!     assert_eq!(&v[..], &[0]);
//! });
//!
//! l.remove(&[2]);
//! assert_eq!(l.len(), 1);
//!
//! l.clear();
//! assert_eq!(l.len(), 0);
//! ```
//!

#[cfg(test)]
mod test;

use crate::common::{
    ende::{SimpleVisitor, ValueEnDe},
    engines, InstanceCfg, RawKey, RawValue,
};
use ruc::*;
use serde::{Deserialize, Serialize};
use std::{
    mem::ManuallyDrop,
    ops::{Deref, DerefMut, RangeBounds},
    result::Result as StdResult,
};

/// To solve the problem of unlimited memory usage,
/// use this to replace the original in-memory `BTreeMap<_, _>`.
#[derive(PartialEq, Eq, Debug)]
pub struct MapxRaw {
    inner: engines::Mapx,
}

impl From<InstanceCfg> for MapxRaw {
    fn from(cfg: InstanceCfg) -> Self {
        Self {
            inner: engines::Mapx::from(cfg),
        }
    }
}

impl Default for MapxRaw {
    fn default() -> Self {
        Self::new()
    }
}

////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////

impl MapxRaw {
    /// Create an instance.
    #[inline(always)]
    pub fn new() -> Self {
        MapxRaw {
            inner: engines::Mapx::new(),
        }
    }

    // Get the database storage path
    pub(crate) fn get_instance_cfg(&self) -> InstanceCfg {
        self.inner.get_instance_cfg()
    }

    /// Imitate the behavior of 'BTreeMap<_>.get(...)'
    #[inline(always)]
    pub fn get(&self, key: &[u8]) -> Option<RawValue> {
        self.inner.get(key)
    }

    /// Check if a key is exists.
    #[inline(always)]
    pub fn contains_key(&self, key: &[u8]) -> bool {
        self.get(key).is_some()
    }

    /// less or equal value
    #[inline(always)]
    pub fn get_le(&self, key: &[u8]) -> Option<(RawKey, RawValue)> {
        self.range(..=key).next_back()
    }

    /// great or equal value
    #[inline(always)]
    pub fn get_ge(&self, key: &[u8]) -> Option<(RawKey, RawValue)> {
        self.range(key..).next()
    }

    /// Imitate the behavior of 'BTreeMap<_>.get_mut(...)'
    #[inline(always)]
    pub fn get_mut(&mut self, key: &[u8]) -> Option<ValueMut<'_>> {
        self.inner
            .get(key)
            .map(move |v| ValueMut::new(self, key.to_owned().into_boxed_slice(), v))
    }

    /// Imitate the behavior of 'BTreeMap<_>.len()'.
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// A helper func
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Imitate the behavior of '.entry(...).or_insert(...)'
    #[inline(always)]
    pub fn entry<'a>(&'a mut self, key: &'a [u8]) -> Entry<'a> {
        Entry { key, hdr: self }
    }

    /// Imitate the behavior of '.iter()'
    #[inline(always)]
    pub fn iter(&self) -> MapxRawIter {
        MapxRawIter {
            iter: self.inner.iter(),
        }
    }

    /// range(start..end)
    #[inline(always)]
    pub fn range<'a, R: RangeBounds<&'a [u8]>>(&'a self, bounds: R) -> MapxRawIter {
        MapxRawIter {
            iter: self.inner.range(bounds),
        }
    }

    /// Imitate the behavior of 'BTreeMap<_>.insert(...)'.
    #[inline(always)]
    pub fn insert(&mut self, key: &[u8], value: &[u8]) -> Option<RawValue> {
        self.inner.insert(key, value)
    }

    /// Try to remove an entry
    #[inline(always)]
    pub fn remove(&mut self, key: &[u8]) -> Option<RawValue> {
        self.inner.remove(key)
    }

    /// Clear all data.
    #[inline(always)]
    pub fn clear(&mut self) {
        self.inner.clear();
    }
}

////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////

/// Returned by `<MapxRaw>.get_mut(...)`
#[derive(PartialEq, Eq, Debug)]
pub struct ValueMut<'a> {
    hdr: &'a mut MapxRaw,
    key: ManuallyDrop<RawKey>,
    value: ManuallyDrop<RawValue>,
}

impl<'a> ValueMut<'a> {
    fn new(hdr: &'a mut MapxRaw, key: RawKey, value: RawValue) -> Self {
        ValueMut {
            hdr,
            key: ManuallyDrop::new(key),
            value: ManuallyDrop::new(value),
        }
    }
}

/// NOTE: Very Important !!!
impl<'a> Drop for ValueMut<'a> {
    fn drop(&mut self) {
        // This operation is safe within a `drop()`.
        // SEE: [**ManuallyDrop::take**](std::mem::ManuallyDrop::take)
        unsafe {
            self.hdr.insert(
                &ManuallyDrop::take(&mut self.key),
                &ManuallyDrop::take(&mut self.value),
            );
        };
    }
}

impl<'a> Deref for ValueMut<'a> {
    type Target = RawValue;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<'a> DerefMut for ValueMut<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////

/// Imitate the `btree_map/btree_map::Entry`.
pub struct Entry<'a> {
    key: &'a [u8],
    hdr: &'a mut MapxRaw,
}

impl<'a> Entry<'a> {
    /// Imitate the `btree_map/btree_map::Entry.or_insert(...)`.
    pub fn or_insert(self, default: &'a [u8]) -> ValueMut<'a> {
        if !self.hdr.contains_key(self.key) {
            self.hdr.insert(self.key, default);
        }
        pnk!(self.hdr.get_mut(self.key))
    }
}

////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////

#[allow(missing_docs)]
pub struct MapxRawIter {
    iter: engines::MapxIter,
}

impl Iterator for MapxRawIter {
    type Item = (RawKey, RawValue);
    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

impl DoubleEndedIterator for MapxRawIter {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.iter.next_back()
    }
}

////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////

impl Serialize for MapxRaw {
    fn serialize<S>(&self, serializer: S) -> StdResult<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_bytes(&<InstanceCfg as ValueEnDe>::encode(
            &self.get_instance_cfg(),
        ))
    }
}

impl<'de> Deserialize<'de> for MapxRaw {
    fn deserialize<D>(deserializer: D) -> StdResult<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_bytes(SimpleVisitor).map(|meta| {
            let meta = pnk!(<InstanceCfg as ValueEnDe>::decode(&meta));
            MapxRaw::from(meta)
        })
    }
}

////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////
