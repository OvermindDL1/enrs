use std::any::TypeId;
use std::marker::PhantomData;

use crate::frunk::{prelude::HList, HCons, HNil};

pub mod typed_index_map;

pub struct TypeListIterExactTypes<C: TypeList>(usize, PhantomData<C>);

impl<C: TypeList> Iterator for TypeListIterExactTypes<C> {
	type Item = TypeId;

	fn next(&mut self) -> Option<Self::Item> {
		let idx = self.0;
		self.0 += 1;
		C::get_type_id_at(idx)
	}

	fn size_hint(&self) -> (usize, Option<usize>) {
		(C::LEN, Some(C::LEN))
	}
}

impl<C: TypeList> ExactSizeIterator for TypeListIterExactTypes<C> {
	fn len(&self) -> usize {
		C::LEN
	}
}

/// This type extends `enrs::frunk::HList` so see it for more information.
pub trait TypeList: 'static + HList {
	/// TODO: This `LENGTH` is to work around:  https://github.com/rust-lang/rust/issues/75961
	/// TODO: Remove this and just call HList's `LEN` when it's fixed...
	type LenTN: generic_array::typenum::Unsigned + generic_array::ArrayLength<TypeId>;

	/// Get the TypeId at a given index in this HList.
	///
	/// TODO:  Make a constant version of this once const generics are in Rust.
	///
	/// ```rust
	/// # use enrs::{frunk::{*, prelude::*}, storages::*};
	/// assert_eq!(<Hlist![]>::get_type_id_at(0), None);
	/// assert_eq!(<Hlist![usize]>::get_type_id_at(0), Some(std::any::TypeId::of::<usize>()));
	/// assert_eq!(<Hlist![usize]>::get_type_id_at(1), None);
	/// ```
	fn get_type_id_at(idx: usize) -> Option<TypeId>;

	/// Get an iterator over all the TypeId's of the types in this HList.
	///
	/// ```rust
	/// # use enrs::{frunk::{*, prelude::*}, storages::*};
	/// type T = Hlist![usize, String];
	/// let v: Vec<_> = T::iter_types().collect();
	/// assert_eq!(v[0], std::any::TypeId::of::<usize>());
	/// assert_eq!(v[1], std::any::TypeId::of::<String>());
	/// ```
	fn iter_types() -> TypeListIterExactTypes<Self>;

	/// Populate a slice of length `Self::LEN` with the TypeId values of the types in this HList.
	///
	/// TODO:  Change `[TypeId]` into `[TypeId; Self::LEN]` when Rust supports const generics.
	///
	/// ```rust
	/// # use enrs::{frunk::{*, prelude::*}, storages::*};
	/// type T = Hlist![usize, String];
	/// let mut v = vec![std::any::TypeId::of::<()>(); T::LEN];
	/// T::populate_type_slice(v.as_mut_slice());
	/// assert_eq!(v[0], std::any::TypeId::of::<usize>());
	/// assert_eq!(v[1], std::any::TypeId::of::<String>());
	/// ```
	fn populate_type_slice(slice: &mut [TypeId]);
}

impl TypeList for HNil {
	type LenTN = generic_array::typenum::U0;
	#[inline]
	fn get_type_id_at(idx: usize) -> Option<TypeId> {
		None
	}
	#[inline]
	fn iter_types() -> TypeListIterExactTypes<Self> {
		TypeListIterExactTypes(0, Default::default())
	}
	// TODO: Change `[TypeId]` to `[TypeId; Self::LEN]` when Rust finally supports it.
	#[inline]
	fn populate_type_slice(_slice: &mut [TypeId]) {}
}

impl<H: 'static, T: TypeList> TypeList for HCons<H, T>
where
	<T as TypeList>::LenTN: std::ops::Add<generic_array::typenum::B1>,
	<<T as TypeList>::LenTN as std::ops::Add<generic_array::typenum::B1>>::Output:
		generic_array::typenum::Unsigned,
	<<T as TypeList>::LenTN as std::ops::Add<generic_array::typenum::B1>>::Output:
		generic_array::ArrayLength<std::any::TypeId>,
{
	type LenTN = generic_array::typenum::Add1<T::LenTN>;

	#[inline]
	fn get_type_id_at(idx: usize) -> Option<TypeId> {
		if idx == 0 {
			Some(TypeId::of::<H>())
		} else {
			T::get_type_id_at(idx - 1)
		}
	}

	#[inline]
	fn iter_types() -> TypeListIterExactTypes<Self> {
		TypeListIterExactTypes(0, Default::default())
	}

	#[inline]
	fn populate_type_slice(slice: &mut [TypeId]) {
		slice[0] = std::any::TypeId::of::<H>();
		T::populate_type_slice(&mut slice[1..]);
	}
}
