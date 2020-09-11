use std::cmp::Ordering;
use std::collections::hash_map::RandomState;
use std::error::Error;
use std::fmt;
use std::fmt::Debug;
use std::hash::{BuildHasher, Hash};
use std::marker::PhantomData;
use std::ops::RangeFull;

use indexmap::{map::*, *};
use std::fmt::Formatter;

/// TypedIndexMap specific errors
pub enum TypedIndexMapError<K, V, I: TypedIndexMapIndexType = usize> {
	TypedIndexMapFull(I, K, V),
}

impl<K, V, I: TypedIndexMapIndexType> Debug for TypedIndexMapError<K, V, I> {
	fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
		use TypedIndexMapError::*;
		match self {
			TypedIndexMapFull(max, _key, _value) => {
				f.debug_tuple("TypedIndexMapFull").field(max).finish()
			}
		}
	}
}

impl<K, V, I: TypedIndexMapIndexType> std::fmt::Display for TypedIndexMapError<K, V, I> {
	fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
		match self {
			TypedIndexMapError::TypedIndexMapFull(size, _k, _v) => {
				f.write_fmt(format_args!("TypedIndexMap index is full with {:?}", size))
			}
		}
	}
}

impl<K, V, I: TypedIndexMapIndexType> std::error::Error for TypedIndexMapError<K, V, I> {
	fn source(&self) -> Option<&(dyn Error + 'static)> {
		None
	}
}

pub trait TypedIndexMapIndexType: Copy + fmt::Debug {
	const MAX: Self;
	fn to_usize(self) -> usize;
	fn try_from_usize(index: usize) -> Option<Self>;
	fn from_usize(index: usize) -> Self;
}

macro_rules! implement_primitive_typed_index_map_index_type {
	($typ:ty) => {
		impl TypedIndexMapIndexType for $typ {
			const MAX: Self = <$typ>::MAX;
			fn to_usize(self) -> usize {
				self as usize
			}
			fn try_from_usize(index: usize) -> Option<Self> {
				if index <= Self::MAX as usize {
					Some(index as Self)
				} else {
					None
				}
			}
			fn from_usize(index: usize) -> Self {
				index as Self
			}
		}
	};
}

implement_primitive_typed_index_map_index_type!(u8);
implement_primitive_typed_index_map_index_type!(u16);
implement_primitive_typed_index_map_index_type!(u32);
implement_primitive_typed_index_map_index_type!(u64);
implement_primitive_typed_index_map_index_type!(usize);

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct TypedIndexMapIndex<T, IndexType: TypedIndexMapIndexType = usize>(
	IndexType,
	PhantomData<T>,
);

impl<T, I: TypedIndexMapIndexType> TypedIndexMapIndex<T, I> {
	fn try_new(index: usize) -> Option<Self> {
		Some(TypedIndexMapIndex(
			I::try_from_usize(index)?,
			Default::default(),
		))
	}

	fn new(index: usize) -> Self {
		TypedIndexMapIndex(I::from_usize(index), Default::default())
	}

	pub const MAX: I = I::MAX;
}
impl<T, I: TypedIndexMapIndexType> Into<usize> for TypedIndexMapIndex<T, I> {
	fn into(self) -> usize {
		self.0.to_usize()
	}
}

/// Please see the documentation of `index_map::map::IndexMap` for details as this just wraps it.
pub struct TypedIndexMap<T, K, V, IndexType: TypedIndexMapIndexType = usize, S = RandomState> {
	index_map: IndexMap<K, V, S>,
	_phantom: PhantomData<(T, IndexType)>,
}

impl<T, K: Clone, V: Clone, I: TypedIndexMapIndexType, S: Clone> Clone
	for TypedIndexMap<T, K, V, I, S>
{
	#[inline]
	fn clone(&self) -> Self {
		TypedIndexMap {
			index_map: Clone::clone(&self.index_map),
			_phantom: Default::default(),
		}
	}

	#[inline]
	fn clone_from(&mut self, source: &Self) {
		Clone::clone_from(&mut self.index_map, &source.index_map)
	}
}

impl<T, K, V, I, S> fmt::Debug for TypedIndexMap<T, K, V, I, S>
where
	K: fmt::Debug + Hash + Eq,
	V: fmt::Debug,
	I: TypedIndexMapIndexType,
	S: BuildHasher,
{
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		fmt::Debug::fmt(&self.index_map, f)
	}
}

impl<T, K, V, I> TypedIndexMap<T, K, V, I>
where
	I: TypedIndexMapIndexType,
{
	/// Create a new map. (Does not allocate.)
	#[inline]
	pub fn new() -> Self {
		Self::with_capacity(0)
	}

	/// Create a new map with capacity for `n` key-value pairs. (Does not
	/// allocate if `n` is zero.)
	///
	/// Computes in **O(n)** time.
	#[inline]
	pub fn with_capacity(n: usize) -> Self {
		Self::with_capacity_and_hasher(n, <_>::default())
	}
}

impl<T, K, V, I, S> TypedIndexMap<T, K, V, I, S>
where
	I: TypedIndexMapIndexType,
{
	/// Create a new map with capacity for `n` key-value pairs. (Does not
	/// allocate if `n` is zero.)
	///
	/// Computes in **O(n)** time.
	#[inline]
	pub fn with_capacity_and_hasher(n: usize, hash_builder: S) -> Self
	where
		S: BuildHasher,
	{
		TypedIndexMap {
			index_map: IndexMap::with_capacity_and_hasher(n, hash_builder),
			_phantom: Default::default(),
		}
	}

	/// Return the number of key-value pairs in the map.
	///
	/// Computes in **O(1)** time.
	#[inline]
	pub fn len(&self) -> usize {
		self.index_map.len()
	}

	/// Returns true if the map contains no elements.
	///
	/// Computes in **O(1)** time.
	#[inline]
	pub fn is_empty(&self) -> bool {
		self.len() == 0
	}

	/// Create a new map with `hash_builder`
	#[inline]
	pub fn with_hasher(hash_builder: S) -> Self
	where
		S: BuildHasher,
	{
		Self::with_capacity_and_hasher(0, hash_builder)
	}

	/// Return a reference to the map's `BuildHasher`.
	#[inline]
	pub fn hasher(&self) -> &S
	where
		S: BuildHasher,
	{
		&self.index_map.hasher()
	}

	/// Computes in **O(1)** time.
	#[inline]
	pub fn capacity(&self) -> usize {
		self.index_map.capacity()
	}
}

impl<T, K, V, I, S> TypedIndexMap<T, K, V, I, S>
where
	K: Hash + Eq,
	S: BuildHasher,
	I: TypedIndexMapIndexType,
{
	/// Remove all key-value pairs in the map, while preserving its capacity.
	///
	/// Computes in **O(n)** time.
	#[inline]
	pub fn clear(&mut self) {
		self.index_map.clear();
	}

	/// Reserve capacity for `additional` more key-value pairs.
	///
	/// Computes in **O(n)** time.
	#[inline]
	pub fn reserve(&mut self, additional: usize) {
		self.index_map.reserve(additional);
	}

	/// Shrink the capacity of the map as much as possible.
	///
	/// Computes in **O(n)** time.
	#[inline]
	pub fn shrink_to_fit(&mut self) {
		self.index_map.shrink_to_fit();
	}

	/// Insert a key-value pair in the map.
	///
	/// If an equivalent key already exists in the map: the key remains and
	/// retains in its place in the order, its corresponding value is updated
	/// with `value` and the older value is returned inside `Some(_)`.
	///
	/// If no equivalent key existed in the map: the new key-value pair is
	/// inserted, last in order, and `None` is returned.
	///
	/// Computes in **O(1)** time (amortized average).
	///
	/// See also [`entry`](#method.entry) if you you want to insert *or* modify
	/// or if you need to get the index of the corresponding key-value pair.
	#[inline]
	pub fn insert(&mut self, key: K, value: V) -> Result<Option<V>, TypedIndexMapError<K, V, I>> {
		Ok(self.insert_full(key, value)?.1)
	}

	/// Insert a key-value pair in the map, and get their index.
	///
	/// If an equivalent key already exists in the map: the key remains and
	/// retains in its place in the order, its corresponding value is updated
	/// with `value` and the older value is returned inside `(index, Some(_))`.
	///
	/// If no equivalent key existed in the map: the new key-value pair is
	/// inserted, last in order, and `(index, None)` is returned.
	///
	/// Computes in **O(1)** time (amortized average).
	///
	/// See also [`entry`](#method.entry) if you you want to insert *or* modify
	/// or if you need to get the index of the corresponding key-value pair.
	#[inline]
	pub fn insert_full(
		&mut self,
		key: K,
		value: V,
	) -> Result<(TypedIndexMapIndex<T, I>, Option<V>), TypedIndexMapError<K, V, I>> {
		if TypedIndexMapIndex::<T, I>::try_new(self.index_map.len()).is_none() {
			return Err(TypedIndexMapError::TypedIndexMapFull(I::MAX, key, value));
		}
		let (idx, res) = self.index_map.insert_full(key, value);
		Ok((TypedIndexMapIndex::new(idx), res))
	}

	/// Get the given key’s corresponding entry in the map for insertion and/or
	/// in-place manipulation.
	///
	/// Computes in **O(1)** time (amortized average).
	#[inline]
	pub fn entry(&mut self, key: K) -> Entry<K, V> {
		self.index_map.entry(key)
	}

	/// Return an iterator over the key-value pairs of the map, in their order
	#[inline]
	pub fn iter(&self) -> Iter<K, V> {
		self.index_map.iter()
	}

	/// Return an iterator over the key-value pairs of the map, in their order
	#[inline]
	pub fn iter_mut(&mut self) -> IterMut<K, V> {
		self.index_map.iter_mut()
	}

	/// Return an iterator over the keys of the map, in their order
	#[inline]
	pub fn keys(&self) -> Keys<K, V> {
		self.index_map.keys()
	}

	/// Return an iterator over the values of the map, in their order
	#[inline]
	pub fn values(&self) -> Values<K, V> {
		self.index_map.values()
	}

	/// Return an iterator over mutable references to the the values of the map,
	/// in their order
	#[inline]
	pub fn values_mut(&mut self) -> ValuesMut<K, V> {
		self.index_map.values_mut()
	}

	/// Return `true` if an equivalent to `key` exists in the map.
	///
	/// Computes in **O(1)** time (average).
	#[inline]
	pub fn contains_key<Q: ?Sized>(&self, key: &Q) -> bool
	where
		Q: Hash + Equivalent<K>,
	{
		self.index_map.contains_key(key)
	}

	/// Return a reference to the value stored for `key`, if it is present,
	/// else `None`.
	///
	/// Computes in **O(1)** time (average).
	#[inline]
	pub fn get<Q: ?Sized>(&self, key: &Q) -> Option<&V>
	where
		Q: Hash + Equivalent<K>,
	{
		self.index_map.get(key)
	}

	/// Return references to the key-value pair stored for `key`,
	/// if it is present, else `None`.
	///
	/// Computes in **O(1)** time (average).
	#[inline]
	pub fn get_key_value<Q: ?Sized>(&self, key: &Q) -> Option<(&K, &V)>
	where
		Q: Hash + Equivalent<K>,
	{
		self.index_map.get_key_value(key)
	}

	/// Return item index, key and value
	#[inline]
	pub fn get_full<Q: ?Sized>(&self, key: &Q) -> Option<(TypedIndexMapIndex<T, I>, &K, &V)>
	where
		Q: Hash + Equivalent<K>,
	{
		self.index_map
			.get_full(key)
			.map(|(idx, k, v)| (TypedIndexMapIndex::new(idx), k, v))
	}

	/// Return item index, if it exists in the map
	#[inline]
	pub fn get_index_of<Q: ?Sized>(&self, key: &Q) -> Option<TypedIndexMapIndex<T, I>>
	where
		Q: Hash + Equivalent<K>,
	{
		self.index_map
			.get_index_of(key)
			.map(TypedIndexMapIndex::new)
	}

	#[inline]
	pub fn get_mut<Q: ?Sized>(&mut self, key: &Q) -> Option<&mut V>
	where
		Q: Hash + Equivalent<K>,
	{
		self.index_map.get_mut(key)
	}

	#[inline]
	pub fn get_full_mut<Q: ?Sized>(
		&mut self,
		key: &Q,
	) -> Option<(TypedIndexMapIndex<T, I>, &K, &mut V)>
	where
		Q: Hash + Equivalent<K>,
	{
		self.index_map
			.get_full_mut(key)
			.map(|(idx, k, v)| (TypedIndexMapIndex::new(idx), k, v))
	}

	/// Remove the key-value pair equivalent to `key` and return
	/// its value.
	///
	/// **NOTE:** This is equivalent to `.swap_remove(key)`, if you need to
	/// preserve the order of the keys in the map, use `.shift_remove(key)`
	/// instead.
	///
	/// Computes in **O(1)** time (average).
	#[inline]
	pub fn remove<Q: ?Sized>(&mut self, key: &Q) -> Option<V>
	where
		Q: Hash + Equivalent<K>,
	{
		self.swap_remove(key)
	}

	/// Remove and return the key-value pair equivalent to `key`.
	///
	/// **NOTE:** This is equivalent to `.swap_remove_entry(key)`, if you need to
	/// preserve the order of the keys in the map, use `.shift_remove_entry(key)`
	/// instead.
	///
	/// Computes in **O(1)** time (average).
	#[inline]
	pub fn remove_entry<Q: ?Sized>(&mut self, key: &Q) -> Option<(K, V)>
	where
		Q: Hash + Equivalent<K>,
	{
		self.swap_remove_entry(key)
	}

	/// Remove the key-value pair equivalent to `key` and return
	/// its value.
	///
	/// Like `Vec::swap_remove`, the pair is removed by swapping it with the
	/// last element of the map and popping it off. **This perturbs
	/// the postion of what used to be the last element!**
	///
	/// Return `None` if `key` is not in map.
	///
	/// Computes in **O(1)** time (average).
	#[inline]
	pub fn swap_remove<Q: ?Sized>(&mut self, key: &Q) -> Option<V>
	where
		Q: Hash + Equivalent<K>,
	{
		self.index_map.swap_remove(key)
	}

	/// Remove and return the key-value pair equivalent to `key`.
	///
	/// Like `Vec::swap_remove`, the pair is removed by swapping it with the
	/// last element of the map and popping it off. **This perturbs
	/// the postion of what used to be the last element!**
	///
	/// Return `None` if `key` is not in map.
	///
	/// Computes in **O(1)** time (average).
	#[inline]
	pub fn swap_remove_entry<Q: ?Sized>(&mut self, key: &Q) -> Option<(K, V)>
	where
		Q: Hash + Equivalent<K>,
	{
		self.index_map.swap_remove_entry(key)
	}

	/// Remove the key-value pair equivalent to `key` and return it and
	/// the index it had.
	///
	/// Like `Vec::swap_remove`, the pair is removed by swapping it with the
	/// last element of the map and popping it off. **This perturbs
	/// the postion of what used to be the last element!**
	///
	/// Return `None` if `key` is not in map.
	///
	/// Computes in **O(1)** time (average).
	#[inline]
	pub fn swap_remove_full<Q: ?Sized>(
		&mut self,
		key: &Q,
	) -> Option<(TypedIndexMapIndex<T, I>, K, V)>
	where
		Q: Hash + Equivalent<K>,
	{
		self.index_map
			.swap_remove_full(key)
			.map(|(idx, k, v)| (TypedIndexMapIndex::new(idx), k, v))
	}

	/// Remove the key-value pair equivalent to `key` and return
	/// its value.
	///
	/// Like `Vec::remove`, the pair is removed by shifting all of the
	/// elements that follow it, preserving their relative order.
	/// **This perturbs the index of all of those elements!**
	///
	/// Return `None` if `key` is not in map.
	///
	/// Computes in **O(n)** time (average).
	#[inline]
	pub fn shift_remove<Q: ?Sized>(&mut self, key: &Q) -> Option<V>
	where
		Q: Hash + Equivalent<K>,
	{
		self.index_map.shift_remove(key)
	}

	/// Remove and return the key-value pair equivalent to `key`.
	///
	/// Like `Vec::remove`, the pair is removed by shifting all of the
	/// elements that follow it, preserving their relative order.
	/// **This perturbs the index of all of those elements!**
	///
	/// Return `None` if `key` is not in map.
	///
	/// Computes in **O(n)** time (average).
	#[inline]
	pub fn shift_remove_entry<Q: ?Sized>(&mut self, key: &Q) -> Option<(K, V)>
	where
		Q: Hash + Equivalent<K>,
	{
		self.index_map.shift_remove_entry(key)
	}

	/// Remove the key-value pair equivalent to `key` and return it and
	/// the index it had.
	///
	/// Like `Vec::remove`, the pair is removed by shifting all of the
	/// elements that follow it, preserving their relative order.
	/// **This perturbs the index of all of those elements!**
	///
	/// Return `None` if `key` is not in map.
	///
	/// Computes in **O(n)** time (average).
	#[inline]
	pub fn shift_remove_full<Q: ?Sized>(
		&mut self,
		key: &Q,
	) -> Option<(TypedIndexMapIndex<T, I>, K, V)>
	where
		Q: Hash + Equivalent<K>,
	{
		self.index_map
			.shift_remove_full(key)
			.map(|(idx, k, v)| (TypedIndexMapIndex::new(idx), k, v))
	}

	/// Remove the last key-value pair
	///
	/// Computes in **O(1)** time (average).
	#[inline]
	pub fn pop(&mut self) -> Option<(K, V)> {
		self.index_map.pop()
	}

	/// Scan through each key-value pair in the map and keep those where the
	/// closure `keep` returns `true`.
	///
	/// The elements are visited in order, and remaining elements keep their
	/// order.
	///
	/// Computes in **O(n)** time (average).
	#[inline]
	pub fn retain<F>(&mut self, keep: F)
	where
		F: FnMut(&K, &mut V) -> bool,
	{
		self.index_map.retain(keep)
	}

	/// Sort the map’s key-value pairs by the default ordering of the keys.
	///
	/// See `sort_by` for details.
	#[inline]
	pub fn sort_keys(&mut self)
	where
		K: Ord,
	{
		self.index_map.sort_keys()
	}

	/// Sort the map’s key-value pairs in place using the comparison
	/// function `compare`.
	///
	/// The comparison function receives two key and value pairs to compare (you
	/// can sort by keys or values or their combination as needed).
	///
	/// Computes in **O(n log n + c)** time and **O(n)** space where *n* is
	/// the length of the map and *c* the capacity. The sort is stable.
	#[inline]
	pub fn sort_by<F>(&mut self, cmp: F)
	where
		F: FnMut(&K, &V, &K, &V) -> Ordering,
	{
		self.index_map.sort_by(cmp)
	}

	/// Sort the key-value pairs of the map and return a by value iterator of
	/// the key-value pairs with the result.
	///
	/// The sort is stable.
	#[inline]
	pub fn sorted_by<F>(self, cmp: F) -> IntoIter<K, V>
	where
		F: FnMut(&K, &V, &K, &V) -> Ordering,
	{
		self.index_map.sorted_by(cmp)
	}

	/// Reverses the order of the map’s key-value pairs in place.
	///
	/// Computes in **O(n)** time and **O(1)** space.
	#[inline]
	pub fn reverse(&mut self) {
		self.index_map.reverse()
	}

	/// Clears the `IndexMap`, returning all key-value pairs as a drain iterator.
	/// Keeps the allocated memory for reuse.
	#[inline]
	pub fn drain(&mut self, range: RangeFull) -> Drain<K, V> {
		self.index_map.drain(range)
	}
}

impl<T, K, V, I, S> TypedIndexMap<T, K, V, I, S>
where
	I: TypedIndexMapIndexType,
{
	/// Get a key-value pair by index
	///
	/// Valid indices are *0 <= index < self.len()*
	///
	/// Computes in **O(1)** time.
	#[inline]
	pub fn get_index(&self, index: TypedIndexMapIndex<T, I>) -> Option<(&K, &V)> {
		self.index_map.get_index(index.into())
	}

	/// Get a key-value pair by index
	///
	/// Valid indices are *0 <= index < self.len()*
	///
	/// Computes in **O(1)** time.
	#[inline]
	pub fn get_index_mut(&mut self, index: TypedIndexMapIndex<T, I>) -> Option<(&mut K, &mut V)> {
		self.index_map.get_index_mut(index.into())
	}

	/// Remove the key-value pair by index
	///
	/// Valid indices are *0 <= index < self.len()*
	///
	/// Like `Vec::swap_remove`, the pair is removed by swapping it with the
	/// last element of the map and popping it off. **This perturbs
	/// the postion of what used to be the last element!**
	///
	/// Computes in **O(1)** time (average).
	#[inline]
	pub fn swap_remove_index(&mut self, index: TypedIndexMapIndex<T, I>) -> Option<(K, V)> {
		self.index_map.swap_remove_index(index.into())
	}

	/// Remove the key-value pair by index
	///
	/// Valid indices are *0 <= index < self.len()*
	///
	/// Like `Vec::remove`, the pair is removed by shifting all of the
	/// elements that follow it, preserving their relative order.
	/// **This perturbs the index of all of those elements!**
	///
	/// Computes in **O(n)** time (average).
	#[inline]
	pub fn shift_remove_index(&mut self, index: TypedIndexMapIndex<T, I>) -> Option<(K, V)> {
		self.index_map.shift_remove_index(index.into())
	}
}
