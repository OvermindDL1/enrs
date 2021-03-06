use std::any::{Any, TypeId};
use std::cell::{Ref, RefCell, RefMut};
use std::collections::hash_map::RandomState;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::PoisonError;

use bitvec::prelude::*;
use generic_array::typenum::Unsigned;
use generic_array::GenericArray;
use indexmap::IndexMap;
use itertools::Itertools;
//use reffers::rc8::*;
//use reffers::arcu::*;
use owning_ref::{OwningHandle, OwningRef, OwningRefMut};

use crate::entity::Entity;
use crate::frunk::{prelude::HList, HCons, HNil};
use crate::storages::secondary_index::{SecondaryIndex, SecondaryIndexErrors};
use crate::storages::TypeList;
use crate::utils::unique_hasher::UniqueHasherBuilder;

/// Possible Errors given by a SparsePageMap's operation.
#[derive(Debug, PartialEq, Eq)]
pub enum SparseTypedPagedMapErrors<EntityType: Entity> {
	PoisonError,
	SecondaryIndexError(SecondaryIndexErrors<EntityType>),
	StorageDoesNotExistInGroup(usize, TypeId),
	StorageAlreadyExistsInGroup(usize, TypeId),
	EntityAlreadyExistsInStorage,
	ComponentStorageDoesNotExist(&'static str),
	EntityDoesNotExistInStorage(EntityType, &'static str),
	EntityGenerationMismatch(EntityType, EntityType),
	IteratorsNotAllSameLength,
}

impl<EntityType: Entity> std::error::Error for SparseTypedPagedMapErrors<EntityType> {
	fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
		use SparseTypedPagedMapErrors::*;
		match self {
			PoisonError => None,
			SecondaryIndexError(source) => Some(source),
			StorageDoesNotExistInGroup(_group, _tid) => None,
			StorageAlreadyExistsInGroup(_group, _tid) => None,
			ComponentStorageDoesNotExist(_name) => None,
			EntityAlreadyExistsInStorage => None,
			EntityDoesNotExistInStorage(_entity, _name) => None,
			EntityGenerationMismatch(_requested_entity, _existing_entity) => None,
			IteratorsNotAllSameLength => None,
		}
	}
}

impl<EntityType: Entity> std::fmt::Display for SparseTypedPagedMapErrors<EntityType> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
		use SparseTypedPagedMapErrors::*;
		match self {
			PoisonError => write!(f, "Lock Poisoned"),
			SecondaryIndexError(_source) => write!(f, "SecondaryIndexError"),
			StorageDoesNotExistInGroup(group, tid) => {
				write!(f, "Storage does not exist in group {}: {:?}", group, tid)
			}
			StorageAlreadyExistsInGroup(group, tid) => {
				write!(f, "Storage already exists in group {}: {:?}", group, tid)
			}
			ComponentStorageDoesNotExist(name) => {
				write!(f, "Component Static Storage does not exist for: {:?}", name)
			}
			EntityAlreadyExistsInStorage => {
				write!(f, "Entity already had the component, cannot add it again")
			}
			EntityDoesNotExistInStorage(entity, name) => write!(
				f,
				"Entity `{:?}` does not exist in component static storage: {}",
				entity, name
			),
			EntityGenerationMismatch(requested_entity, existing_entity) => write!(
				f,
				"Requested Entity of `{:?}` does not match the internal Entity of `{:?}`",
				requested_entity, existing_entity
			),
			IteratorsNotAllSameLength => write!(
				f,
				"Passed in iterators must all be the same length as the entities iterator"
			),
		}
	}
}

impl<EntityType: Entity> From<SecondaryIndexErrors<EntityType>>
	for SparseTypedPagedMapErrors<EntityType>
{
	fn from(source: SecondaryIndexErrors<EntityType>) -> Self {
		SparseTypedPagedMapErrors::SecondaryIndexError(source)
	}
}

impl<EntityType: Entity, Guard> From<PoisonError<Guard>> for SparseTypedPagedMapErrors<EntityType> {
	fn from(_source: PoisonError<Guard>) -> Self {
		SparseTypedPagedMapErrors::PoisonError
	}
}

mod private {
	pub trait Sealed {}
}

pub trait DensePagedData: private::Sealed + 'static {
	fn as_any(&self) -> &dyn std::any::Any;
	//fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
	fn len_groups(&self) -> usize;
	fn resize(&self, new_len: usize);
	fn try_resize(&self, new_len: usize) -> bool;
	fn truncate_group(&self, group: usize, len: usize);
	fn swap_remove(&self, group: usize, index: usize);
	fn swap_to_group(&self, old_group: usize, index: usize, new_group: usize) -> usize;
}

impl dyn DensePagedData {
	// fn cast<DataType: 'static>(&self) -> &DensePagedDataInstance<DataType> {
	// 	self.as_any()
	// 		.downcast_ref()
	// 		.expect("Type mismatch in map!  Shouldn't happen!")
	// }
	//
	// fn cast_mut<DataType: 'static>(&mut self) -> &mut DensePagedDataInstance<DataType> {
	// 	self.as_any_mut()
	// 		.downcast_mut()
	// 		.expect("Type mismatch in map!  Shouldn't happen!")
	// }

	fn get_strong<DataType: 'static>(&self) -> Rc<RefCell<DensePagedDataActual<DataType>>> {
		self.as_any()
			.downcast_ref::<DensePagedDataInstance<DataType>>()
			.expect("Type mismatch in map!  Shouldn't happen!")
			.0
			.clone()
	}

	// fn get_weak<DataType: 'static>(&self) -> Weak<DensePagedDataActual<DataType>> {
	// 	// Rc::downgrade(
	// 	// 	&self
	// 	// 		.as_any()
	// 	// 		.downcast_ref::<DensePagedDataInstance<DataType>>()
	// 	// 		.expect("Type mismatch in map!  Shouldn't happen!")
	// 	// 		.0,
	// 	// )
	// 	todo!()
	// }

	// fn get_ref<DataType: 'static>(&self) -> Ref<DensePagedDataActual<DataType>> {
	// 	self.as_any()
	// 		.downcast_ref::<DensePagedDataInstance<DataType>>()
	// 		.expect("Type mismatch in map!  Shouldn't happen!")
	// 		.0
	// 		.borrow()
	// }

	fn get_refmut<DataType: 'static>(&self) -> RefMut<DensePagedDataActual<DataType>> {
		self.as_any()
			.downcast_ref::<DensePagedDataInstance<DataType>>()
			.expect("Type mismatch in map!  Shouldn't happen!")
			.0
			.borrow_mut()
	}

	// fn get<DataType: 'static>(&self, group: usize, idx: usize) -> Option<&DataType> {
	// 	let data_storage: &DensePagedDataInstance<DataType> = self.cast();
	// 	let group_storage = &data_storage.0.data.get_ref()[group];
	// 	group_storage.get(idx)
	// }
	//
	// fn get_mut<DataType: 'static>(&mut self, group: usize, idx: usize) -> Option<&mut DataType> {
	// 	let data_storage: &mut DensePagedDataInstance<DataType> = self.cast_mut();
	// 	let group_storage = &mut data_storage.0.data.get_refmut[group];
	// 	group_storage.get_mut(idx)
	// }
}

pub struct DensePagedDataActual<DataType: 'static> {
	index: usize,
	data: Vec<Vec<DataType>>,
}

pub struct DensePagedDataInstance<DataType: 'static>(Rc<RefCell<DensePagedDataActual<DataType>>>);

impl<DataType: 'static> DensePagedDataActual<DataType> {
	fn push(&mut self, group: usize, data: DataType) -> usize {
		let storage = &mut self.data[group];
		storage.push(data);
		storage.len() - 1
	}

	fn push_all<I: IntoIterator<Item = DataType>>(&mut self, group: usize, data: I) -> usize {
		let storage = &mut self.data[group];
		let start_idx = storage.len();
		storage.extend(data);
		start_idx
	}

	#[inline]
	fn truncate_group(&mut self, group: usize, len: usize) {
		self.data[group].truncate(len);
	}
}

impl<DataType: 'static> DensePagedDataInstance<DataType> {
	fn with_groups(map_idx: usize, group_size: usize) -> Self {
		Self(Rc::new(RefCell::new(DensePagedDataActual {
			index: map_idx,
			data: (0..group_size).map(|_| vec![]).collect(),
		})))
	}

	// fn push(&mut self, group: usize, data: DataType) -> usize {
	// 	let group_storage = self.0.get_refmut();
	// 	let storage = &mut group_storage.data[group];
	// 	storage.push(data);
	// 	storage.len() - 1
	// }
	//
	// fn push_all<I: IntoIterator<Item = DataType>>(&mut self, group: usize, data: I) -> usize {
	// 	let group_storage = self.0.get_refmut();
	// 	let storage = &mut group_storage.data[group];
	// 	let start_idx = storage.len();
	// 	storage.extend(data);
	// 	start_idx
	// }

	// fn get(&self, group: usize, idx: usize) -> Option<&DataType> {
	// 	self.data[group].get(idx)
	// }
	//
	// fn get_mut(&mut self, group: usize, idx: usize) -> Option<&mut DataType> {
	// 	self.data[group].get_mut(idx)
	// }
}

impl<DataType: 'static> private::Sealed for DensePagedDataInstance<DataType> {}

impl<DataType: 'static> DensePagedData for DensePagedDataInstance<DataType> {
	#[inline]
	fn as_any(&self) -> &dyn Any {
		self
	}
	// #[inline]
	// fn as_any_mut(&mut self) -> &mut dyn Any {
	// 	self
	// }
	#[inline]
	fn len_groups(&self) -> usize {
		self.0.borrow().data.len()
	}
	#[inline]
	fn resize(&self, new_len: usize) {
		self.0.borrow_mut().data.resize_with(new_len, Vec::new);
	}
	#[inline]
	fn try_resize(&self, new_len: usize) -> bool {
		if let Ok(mut inner) = self.0.try_borrow_mut() {
			inner.data.resize_with(new_len, Vec::new);
			true
		} else {
			false
		}
	}
	#[inline]
	fn truncate_group(&self, group: usize, len: usize) {
		self.0.borrow_mut().data[group].truncate(len);
	}
	#[inline]
	fn swap_remove(&self, group: usize, index: usize) {
		self.0.borrow_mut().data[group].swap_remove(index);
	}
	#[inline]
	fn swap_to_group(&self, old_group: usize, index: usize, new_group: usize) -> usize {
		let mut this = self.0.borrow_mut();
		let data = this.data[old_group].swap_remove(index);
		let group = &mut this.data[new_group];
		group.push(data);
		group.len() - 1
	}
}

// pub struct DensePagedMap {
// 	//map: Box<dyn ErasedSparsePageMap<EntityType>>,
// 	map: Box<dyn DensePagedData>,
// }
//
// impl DensePagedMap {
// 	fn new<DataType: 'static>() -> Self {
// 		let map: Box<dyn DensePagedData> =
// 			Box::new(DensePagedDataInstance::<DataType>(Default::default()));
// 		Self { map }
// 	}
//
// 	fn with_groups<DataType: 'static>(group_count: usize) -> Self {
// 		let map: Box<dyn DensePagedData> = Box::new(DensePagedDataInstance::<DataType>(
// 			(0..group_count).map(|_| Vec::new()).collect(),
// 		));
// 		Self { map }
// 	}
//
// 	fn resize(&mut self, size: usize) {
// 		self.map.resize(size);
// 	}
//
// 	fn truncate_group(&mut self, group: usize, size: usize) {
// 		self.map.truncate_group(group, size);
// 	}
//
// 	fn len_groups(&self) -> usize {
// 		self.map.len_groups()
// 	}
//
// 	fn push<DataType: 'static>(&mut self, group: usize, data: DataType) -> usize {
// 		let data_storage: &mut DensePagedDataInstance<DataType> = self
// 			.map
// 			.as_any_mut()
// 			.downcast_mut()
// 			.expect("Type mismatch in map!  Shouldn't happen!");
// 		let group_storage = &mut data_storage.0[group];
// 		group_storage.push(data);
// 		group_storage.len() - 1
// 	}
//
// 	fn push_all<DataType: 'static, I: IntoIterator<Item = DataType>>(
// 		&mut self,
// 		group: usize,
// 		data: I,
// 	) -> usize {
// 		let data_storage: &mut DensePagedDataInstance<DataType> = self
// 			.map
// 			.as_any_mut()
// 			.downcast_mut()
// 			.expect("Type mismatch in map!  Shouldn't happen!");
// 		let group_storage = &mut data_storage.0[group];
// 		let start_idx = group_storage.len();
// 		group_storage.extend(data);
// 		start_idx
// 	}
//
// 	fn get<DataType: 'static>(&self, group: usize, idx: usize) -> Option<&DataType> {
// 		let data_storage: &DensePagedDataInstance<DataType> = self
// 			.map
// 			.as_any()
// 			.downcast_ref()
// 			.expect("Type mismatch in map!  Shouldn't happen!");
// 		let group_storage = &data_storage.0[group];
// 		group_storage.get(idx)
// 	}
//
// 	fn get_mut<DataType: 'static>(&mut self, group: usize, idx: usize) -> Option<&mut DataType> {
// 		let data_storage: &mut DensePagedDataInstance<DataType> = self
// 			.map
// 			.as_any_mut()
// 			.downcast_mut()
// 			.expect("Type mismatch in map!  Shouldn't happen!");
// 		let group_storage = &mut data_storage.0[group];
// 		group_storage.get_mut(idx)
// 	}
// }

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ComponentLocations {
	group: usize,
	index: usize,
}

impl ComponentLocations {
	const INVALID: ComponentLocations = ComponentLocations {
		group: usize::MAX,
		index: usize::MAX,
	};

	// fn new(group: usize, index: usize) -> Self {
	// 	Self { group, index }
	// }
}

#[derive(PartialEq, Eq)]
struct QueryTypedPagedKey<'a> {
	//<I: ComponentSet, E: ComponentSet> {
	// read_only: generic_array::GenericArray<TypeId, RO::LenTN>,
	// read_write: generic_array::GenericArray<TypeId, RW::LenTN>,
	// include: generic_array::GenericArray<TypeId, I::LenTN>,
	// exclude: generic_array::GenericArray<TypeId, E::LenTN>,
	include: &'a [TypeId],
	exclude: &'a [TypeId],
}

#[derive(PartialEq, Eq)]
struct QueryTypedPagedKeyBoxed {
	// read_only: Box<[TypeId]>,
	// read_write: Box<[TypeId]>,
	include: Box<[TypeId]>,
	exclude: Box<[TypeId]>,
}

//impl<I: ComponentSet, E: ComponentSet> QueryTypedPagedKey<I, E> {
impl<'a> QueryTypedPagedKey<'a> {
	// fn new() -> Self {
	// 	Self {
	// 		// read_only: generic_array::GenericArray::from_exact_iter(RO::iter_types()).unwrap(),
	// 		// read_write: generic_array::GenericArray::from_exact_iter(RW::iter_types()).unwrap(),
	// 		include: generic_array::GenericArray::from_exact_iter(I::iter_types()).unwrap(),
	// 		exclude: generic_array::GenericArray::from_exact_iter(E::iter_types()).unwrap(),
	// 	}
	// }

	fn to_box(self) -> QueryTypedPagedKeyBoxed {
		QueryTypedPagedKeyBoxed {
			// read_only: self.read_only.to_vec().into_boxed_slice(),
			// read_write: self.read_write.to_vec().into_boxed_slice(),
			include: self.include.into(),
			exclude: self.exclude.into(),
		}
	}
}

//impl<I: ComponentSet, E: ComponentSet> Hash for QueryTypedPagedKey<I, E> {
impl<'a> Hash for QueryTypedPagedKey<'a> {
	fn hash<H: Hasher>(&self, state: &mut H) {
		// self.read_only.as_slice().hash(state);
		// self.read_write.as_slice().hash(state);
		// self.include.as_slice().hash(state);
		// self.exclude.as_slice().hash(state);
		self.include.hash(state);
		self.exclude.hash(state);
	}
}

impl Hash for QueryTypedPagedKeyBoxed {
	fn hash<H: Hasher>(&self, state: &mut H) {
		// self.read_only.as_ref().hash(state);
		// self.read_write.as_ref().hash(state);
		self.include.as_ref().hash(state);
		self.exclude.as_ref().hash(state);
	}
}

// impl<I: ComponentSet, E: ComponentSet> indexmap::Equivalent<QueryTypedPagedKeyBoxed>
// 	for QueryTypedPagedKey<I, E>
impl<'a> indexmap::Equivalent<QueryTypedPagedKeyBoxed> for QueryTypedPagedKey<'a> {
	fn equivalent(&self, key: &QueryTypedPagedKeyBoxed) -> bool {
		// key.read_only.as_ref() == self.read_only.as_slice()
		// 	&& key.read_write.as_ref() == self.read_write.as_slice()
		&*key.include == self.include && &*key.exclude == self.exclude
	}
}

/// These are the indexes to the `group_sets_to_maps`
struct QueryTypedPagedLink {
	include_groups: Rc<RefCell<Vec<usize>>>,
	//exclude_groups: Vec<usize>,
	include_maps: Vec<usize>, // read_only_groups: Vec<usize>,
	                          // read_write_groups: Vec<usize>,
	                          // except_groups: Vec<usize>,
	                          // read_only_maps: Vec<usize>,
	                          // read_write_maps: Vec<usize>
}

type MapIndexMap = IndexMap<TypeId, Box<dyn DensePagedData>, UniqueHasherBuilder>;

struct GroupTypeSetAddRemoveIntoIterator<'a>(
	&'a Vec<TypeId>,
	&'a [TypeId],
	&'a [TypeId],
	Vec<TypeId>,
);

impl<'a> GroupTypeSetAddRemoveIntoIterator<'a> {
	fn ensure_valid<EntityType: Entity>(
		&self,
		source_group: usize,
	) -> Result<(), SparseTypedPagedMapErrors<EntityType>> {
		if !self.1.is_empty() {
			return Err(SparseTypedPagedMapErrors::StorageDoesNotExistInGroup(
				source_group,
				self.1[0],
			));
		}
		if !self.3.is_empty() {
			return Err(SparseTypedPagedMapErrors::StorageAlreadyExistsInGroup(
				source_group,
				self.3[0],
			));
		}
		Ok(())
	}
	fn iter<'s: 'a>(&'s mut self) -> impl Iterator<Item = &'s TypeId> {
		let remove = &mut self.1;
		let add = &mut self.2;
		let duplicate_add = &mut self.3;
		self.0
			.iter()
			.filter(move |&tid| {
				if let Some(removing) = remove.first() {
					if tid == removing {
						*remove = &&remove[1..];
						return false;
					}
				}
				true
			})
			.merge_join_by(add.iter(), |tid, add_tid| tid.cmp(add_tid))
			.map(move |either| {
				use itertools::EitherOrBoth::{Both, Left, Right};
				match either {
					Left(tid) => tid,
					Right(tid) => tid,
					Both(tid, _tid) => {
						duplicate_add.push(*tid);
						tid
					}
				}
			})
	}
}

#[derive(Default)]
pub struct GroupTypeSetToMapSet(IndexMap<Box<[TypeId]>, BitBox>);

impl GroupTypeSetToMapSet {
	#[inline(always)]
	fn keys(&self) -> indexmap::map::Keys<'_, Box<[TypeId]>, BitBox> {
		self.0.keys()
	}

	#[inline(always)]
	//fn get_index_of<K: Hash + Equivalent<Vec<TypeId>>>(&self, key: &K) -> Option<usize> {
	fn get_index_of(&self, key: &[TypeId]) -> Option<usize> {
		Some(self.0.get_full(key)?.0)
	}

	#[inline(always)]
	fn insert(&mut self, key: Box<[TypeId]>, map_idxs: BitBox) {
		self.0.insert(key, map_idxs);
	}

	#[inline(always)]
	fn len(&self) -> usize {
		self.0.len()
	}

	#[inline(always)]
	fn get_by_group(&self, group: usize) -> (&Box<[TypeId]>, &BitBox) {
		self.0
			.get_index(group)
			.expect("Passed group was invalid, system in inconsistent state")
	}

	// fn get_group_by_group_with_add_remove<EntityType: Entity>(
	// 	&mut self,
	// 	maps: &mut MapIndexMap,
	// 	source_group: usize,
	// 	mut remove: &[TypeId],
	// 	mut add: &[TypeId],
	// ) -> Result<(usize, &Vec<TypeId>, &BitVec), SparseTypedPagedMapErrors<EntityType>> {
	// 	let (type_set, map_set) = self.get_by_group(source_group);
	//
	// 	let mut new_key: Vec<TypeId> =
	// 		Vec::with_capacity(type_set.len() - remove.len() + add.len());
	// 	let mut new_key_iter = GroupTypeSetAddRemoveIntoIterator(type_set, remove, add, vec![]);
	// 	{
	// 		let iter = new_key_iter.iter();
	// 		drop(iter);
	// 		// for &tid in new_key_iter.iter() {
	// 		// 	//new_key.push(tid);
	// 		// }
	// 		//new_key.extend(new_key_iter.iter());
	// 	}
	// 	new_key_iter.ensure_valid(source_group)?;
	//
	// 	// let new_group_set = if let Some(result) = self.0.get_full(&new_key) {
	// 	// 	result
	// 	// } else {
	// 	// 	let new_map_idxs: BitSet = new_key.iter().map(|&tid| {
	// 	// 		if let Some((idx, _tid, _map)) = maps.get_full(tid) {
	// 	// 			idx
	// 	// 		} else {
	// 	// 			maps.in
	// 	// 		}
	// 	// 	}).collect();
	// 	// 	self.0.insert(new_key, )
	// 	// }
	// 	todo!()
	//
	// 	// let (new_group, map_idxs) = if let Some((group, _key, map_idxs)) = self.group_sets_to_maps.get_full(new_group_key.as_slice())
	// 	// {
	// 	// 	(group, map_idxs)
	// 	// } else {
	// 	// 	self.group_sets_to_maps
	// 	// 		.insert(new_group_key, new_map_idxs);
	// 	// 	entities.push(Vec::with_capacity(1));
	// 	// 	let group = self.group_sets_to_maps.len() - 1;
	// 	// 	for map in maps.values_mut() {
	// 	// 		map.resize(group + 1);
	// 	// 	}
	// 	// 	Self::update_query_mappings(
	// 	// 		&self.group_sets_to_maps,
	// 	// 		&mut *self.query_mappings.borrow_mut(),
	// 	// 		group,
	// 	// 	);
	// 	// 	(group, self.group_sets_to_maps.get_index(group).unwrap().1)
	// 	// };
	//
	// 	// let (old_size, new_group_key, new_map_idxs) = {
	// 	// 	let (new_group_key, new_map_idxs): (Vec<_>, Vec<_>) = group_set.0.iter().zip(group_set.1.iter()).filter(|(tid, map_idx)| {
	// 	// 		if cset.contains(tid) {
	// 	// 			let map = maps.get_index(**map_idx).expect("Invalid storage state").1;
	// 	// 			map.swap_remove(location.group, location.index);
	// 	// 			false
	// 	// 		} else {
	// 	// 			true
	// 	// 		}
	// 	// 	}).unzip();
	// 	// 	(group_set.0.len(), new_group_key, new_map_idxs)
	// 	// };
	// }
}

pub struct SparseTypedPagedMap<EntityType: Entity> {
	reverse: Rc<RefCell<SecondaryIndex<EntityType, ComponentLocations>>>,
	entities: Rc<RefCell<Vec<Vec<EntityType>>>>,
	maps: Rc<RefCell<MapIndexMap>>,
	group_sets_to_maps: Rc<RefCell<GroupTypeSetToMapSet>>,
	query_mappings: Rc<RefCell<IndexMap<QueryTypedPagedKeyBoxed, QueryTypedPagedLink>>>,
}

impl<EntityType: Entity> Default for SparseTypedPagedMap<EntityType> {
	fn default() -> Self {
		Self::new()
	}
}

impl<EntityType: Entity> SparseTypedPagedMap<EntityType> {
	// private

	fn update_query_mappings(
		group_to_maps: &GroupTypeSetToMapSet,
		query_mappings: &mut IndexMap<QueryTypedPagedKeyBoxed, QueryTypedPagedLink>,
		group: usize,
	) {
		let (types, _map) = &group_to_maps
			.0
			.get_index(group)
			.expect("Attempting to update mapping group when group does not yet exist");
		for (query, link) in query_mappings.iter_mut() {
			if query.include.iter().all(|tid| types.contains(tid))
				&& query.exclude.iter().all(|tid| !types.contains(tid))
			{
				link.include_groups.borrow_mut().push(group);
			}
		}
	}

	// public
	pub fn new() -> Self {
		Self {
			reverse: Rc::new(RefCell::new(SecondaryIndex::new(
				ComponentLocations::INVALID,
			))),
			entities: Rc::new(RefCell::new(Default::default())),
			maps: Rc::new(RefCell::new(IndexMap::with_hasher(UniqueHasherBuilder))),
			group_sets_to_maps: Default::default(),
			query_mappings: Default::default(),
		}
	}

	pub fn contains(&self, entity: EntityType) -> bool {
		Self::get_valid_location(&*self.reverse.borrow(), &*self.entities.borrow(), entity).is_ok()
	}

	fn insert_valid_location_mut<'a>(
		reverse: &'a mut SecondaryIndex<EntityType, ComponentLocations>,
		entities: &mut Vec<Vec<EntityType>>,
		entity: EntityType,
		group: usize,
	) -> Result<&'a mut ComponentLocations, SparseTypedPagedMapErrors<EntityType>> {
		let location = reverse.insert_mut(entity)?;
		location.group = group;
		// This should already be in sync so no resizing ever needed
		// if entities.len() <= location.group {
		// 	entities.resize(location.group, vec![]);
		// }
		let entities_group = &mut entities[group];
		location.index = entities_group.len();
		entities_group.push(entity);
		Ok(location)
	}

	fn get_valid_location<'a>(
		reverse: &'a SecondaryIndex<EntityType, ComponentLocations>,
		entities: &Vec<Vec<EntityType>>,
		entity: EntityType,
	) -> Result<&'a ComponentLocations, SparseTypedPagedMapErrors<EntityType>> {
		let location = reverse.get(entity)?;
		if entities[location.group][location.index] != entity {
			return Err(SparseTypedPagedMapErrors::EntityGenerationMismatch(
				entity,
				entities[location.group][location.index],
			));
		}
		Ok(location)
	}

	fn get_valid_location_mut<'a>(
		reverse: &'a mut SecondaryIndex<EntityType, ComponentLocations>,
		entities: &Vec<Vec<EntityType>>,
		entity: EntityType,
	) -> Result<&'a mut ComponentLocations, SparseTypedPagedMapErrors<EntityType>> {
		let location = reverse.get_mut(entity)?;
		if entities[location.group][location.index] != entity {
			return Err(SparseTypedPagedMapErrors::EntityGenerationMismatch(
				entity,
				entities[location.group][location.index],
			));
		}
		Ok(location)
	}

	fn remove_valid_location(
		reverse: &mut SecondaryIndex<EntityType, ComponentLocations>,
		entities: &mut Vec<Vec<EntityType>>,
		entity: EntityType,
	) -> Result<ComponentLocations, SparseTypedPagedMapErrors<EntityType>> {
		let location = reverse.get_mut(entity)?;
		let entities_group = &mut entities[location.group];
		if entities_group[location.index] != entity {
			return Err(SparseTypedPagedMapErrors::EntityGenerationMismatch(
				entity,
				entities[location.group][location.index],
			));
		}
		let loc = *location;
		*location = ComponentLocations::INVALID;
		entities_group.swap_remove(loc.index);
		if entities_group.len() > loc.index {
			let replacement_entity = entities_group[loc.index];
			reverse
				.get_mut(replacement_entity)
				.expect("SecondaryIndex is in invalid state")
				.index = loc.index;
		}
		Ok(loc)
	}

	pub fn remove(
		&mut self,
		entity: EntityType,
	) -> Result<(), SparseTypedPagedMapErrors<EntityType>> {
		let location = Self::remove_valid_location(
			&mut *self.reverse.borrow_mut(),
			&mut *self.entities.borrow_mut(),
			entity,
		)?;
		let mut maps = self.maps.borrow_mut();
		for map in maps.values_mut() {
			map.swap_remove(location.group, location.index);
		}
		Ok(())
	}

	// pub fn add_remove_components<CTR: ComponentTuple<EntityType>, CTA: ComponentTuple<EntityType>>(&mut self, entity: EntityType, add: CTA) -> Result<(), SparseTypedPagedMapErrors<EntityType>> {
	// 	let remove_cset: generic_array::GenericArray<(usize, TypeId), CTR::LenTN> = CTR::get_sorted_indexed_tids();
	// 	let add_set: generic_array::GenericArray<(usize, TypeId), CTA::LenTN> = CTA::get_sorted_indexed_tids();
	// 	let mut reverse = self.reverse.borrow_mut();
	// 	let mut entities = self.entities.borrow_mut();
	// 	let mut location = Self::get_valid_location_mut(
	// 		&mut *reverse,
	// 		&mut *entities,
	// 		entity,
	// 	)?;
	// 	let mut maps = self.maps.borrow_mut();
	// 	let (new_group, new_group_key, new_group_map_idxs) = self.group_sets_to_maps.get_group_by_group_with_add_remove(location.group, &mut *maps, cset.as_slice(), &[])?;
	// 	remove_cset.as_slice()
	//
	// 	// let (old_size, new_group_key, new_map_idxs) = {
	// 	// 	let group_set = self.group_sets_to_maps.get_index(location.group).expect("Internal entity had invalid group");
	// 	// 	if cset.iter().any(|tid| !group_set.0.contains(tid)) {
	// 	// 		// TODO:  Add way to get name of tid from the cset
	// 	// 		return Err(SparseTypedPagedMapErrors::EntityDoesNotExistInStorage(entity, ""));
	// 	// 	}
	// 	// 	let (new_group_key, new_map_idxs): (Vec<_>, Vec<_>) = group_set.0.iter().zip(group_set.1.iter()).filter(|(tid, map_idx)| {
	// 	// 		if cset.contains(tid) {
	// 	// 			let map = maps.get_index(**map_idx).expect("Invalid storage state").1;
	// 	// 			map.swap_remove(location.group, location.index);
	// 	// 			false
	// 	// 		} else {
	// 	// 			true
	// 	// 		}
	// 	// 	}).unzip();
	// 	// 	(group_set.0.len(), new_group_key, new_map_idxs)
	// 	// };
	// 	let (new_group, map_idxs) = if let Some((group, _key, map_idxs)) = self.group_sets_to_maps.get_full(new_group_key.as_slice())
	// 	{
	// 		(group, map_idxs)
	// 	} else {
	// 		self.group_sets_to_maps
	// 			.insert(new_group_key, new_map_idxs);
	// 		entities.push(Vec::with_capacity(1));
	// 		let group = self.group_sets_to_maps.len() - 1;
	// 		for map in maps.values_mut() {
	// 			map.resize(group + 1);
	// 		}
	// 		Self::update_query_mappings(
	// 			&self.group_sets_to_maps,
	// 			&mut *self.query_mappings.borrow_mut(),
	// 			group,
	// 		);
	// 		(group, self.group_sets_to_maps.get_index(group).unwrap().1)
	// 	};
	// 	let old_group = location.group;
	// 	let old_index = location.index;
	// 	let entity = entities[old_group].swap_remove(old_index);
	// 	entities[new_group].push(entity);
	// 	location.group = new_group;
	// 	location.index = entities[new_group].len() - 1;
	// 	for &map_idx in map_idxs {
	// 		let map = maps.get_index(map_idx).expect("Invalid storage state").1;
	// 		map.swap_to_group(old_group, old_index, new_group);
	// 	}
	// 	if old_index < entities[old_group].len() {
	// 		let fixup_entity = entities[old_group][old_index];
	// 		let fixup_location = reverse.get_mut(fixup_entity)?;
	// 		fixup_location.index = old_index;
	// 	}
	// 	Ok(())
	// }

	// pub fn insert_components<CT: ComponentTuple<EntityType>>(&mut self, entity: EntityType, components: CT) -> Result<(), SparseTypedPagedMapErrors<EntityType>> {
	// 	let cset: generic_array::GenericArray<TypeId, CT::LenTN> = CT::get_tids();
	// 	let mut reverse = self.reverse.borrow_mut();
	// 	let mut entities = self.entities.borrow_mut();
	// 	let mut location = Self::get_valid_location_mut(
	// 		&mut *reverse,
	// 		&mut *entities,
	// 		entity,
	// 	)?;
	// 	let mut maps = self.maps.borrow_mut();
	// 	let (old_size, new_group_key, old_map_idxs) = {
	// 		let group_set = self.group_sets_to_maps.get_index(location.group).expect("Internal entity had invalid group");
	// 		if group_set.0.iter().any(|tid| cset.contains(tid)) {
	// 			// TODO:  Add way to get name of tid from the cset
	// 			return Err(SparseTypedPagedMapErrors::EntityAlreadyExistsInStorage);
	// 		}
	// 		let mut new_group_key = Vec::with_capacity(CT::LenTN::USIZE + group_set.0.len());
	// 		new_group_key.extend(group_set.0);
	// 		new_group_key.extend_from_slice(cset.as_slice());
	// 		(group_set.0.len(), new_group_key, group_set.1.clone())
	// 	};
	// 	let (new_group, map_idxs) = if let Some((group, _key, map_idxs)) = self.group_sets_to_maps.get_full(new_group_key.as_slice())
	// 	{
	// 		(group, map_idxs)
	// 	} else {
	// 		let mut map_idxs = Vec::with_capacity(CT::LenTN::USIZE + old_size);
	// 		map_idxs.extend(old_map_idxs.iter());
	// 		map_idxs.extend(CT::into_type_idx_vec(&mut *maps));
	// 		self.group_sets_to_maps
	// 			.insert(new_group_key, map_idxs);
	// 		entities.push(Vec::with_capacity(1));
	// 		let group = self.group_sets_to_maps.len() - 1;
	// 		for map in maps.values_mut() {
	// 			map.resize(group + 1);
	// 		}
	// 		Self::update_query_mappings(
	// 			&self.group_sets_to_maps,
	// 			&mut *self.query_mappings.borrow_mut(),
	// 			group,
	// 		);
	// 		(group, self.group_sets_to_maps.get_index(group).unwrap().1)
	// 	};
	// 	let old_group = location.group;
	// 	let old_index = location.index;
	// 	let entity = entities[old_group].swap_remove(old_index);
	// 	entities[new_group].push(entity);
	// 	location.group = new_group;
	// 	location.index = entities[new_group].len() - 1;
	// 	for map_idx in old_map_idxs {
	// 		let map = maps.get_index(map_idx).expect("Invalid storage state").1;
	// 		map.swap_to_group(old_group, old_index, new_group);
	// 	}
	// 	let start_at = map_idxs.len() - cset.len();
	// 	components.insert(&mut *maps, &map_idxs[start_at..], new_group);
	// 	if old_index < entities[old_group].len() {
	// 		let fixup_entity = entities[old_group][old_index];
	// 		let fixup_location = reverse.get_mut(fixup_entity)?;
	// 		fixup_location.index = old_index;
	// 	}
	// 	Ok(())
	// }

	// pub fn insert<CT: ComponentTuple<EntityType>>(
	// 	&mut self,
	// 	entity: EntityType,
	// 	components: CT,
	// ) -> Result<(), SparseTypedPagedMapErrors<EntityType>> {
	// 	let cset: generic_array::GenericArray<TypeId, CT::LenTN> = CT::get_tids();
	// 	let mut maps = self.maps.borrow_mut();
	// 	let (group, map_idxs) = if let Some((group, _key, map_idxs)) =
	// 		self.group_sets_to_maps.get_full(cset.as_slice())
	// 	{
	// 		(group, map_idxs)
	// 	} else {
	// 		self.group_sets_to_maps
	// 			.insert(cset.to_vec(), CT::into_type_idx_vec(&mut *maps));
	// 		self.entities.borrow_mut().push(Vec::with_capacity(1));
	// 		let group = self.group_sets_to_maps.len() - 1;
	// 		for map in maps.values_mut() {
	// 			map.resize(group + 1);
	// 		}
	// 		Self::update_query_mappings(
	// 			&self.group_sets_to_maps,
	// 			&mut *self.query_mappings.borrow_mut(),
	// 			group,
	// 		);
	// 		(group, self.group_sets_to_maps.get_index(group).unwrap().1)
	// 	};
	// 	Self::insert_valid_location_mut(
	// 		&mut *self.reverse.borrow_mut(),
	// 		&mut *self.entities.borrow_mut(),
	// 		entity,
	// 		group,
	// 	)?;
	// 	components.insert(&mut *maps, map_idxs, group);
	// 	Ok(())
	// }

	// pub fn extend_iter<CT: ComponentTuple<EntityType>, I: IntoIterator<Item = (EntityType, CT)>>(
	// 	&mut self,
	// 	iter: I,
	// ) -> Result<(), SparseTypedPagedMapErrors<EntityType>> {
	// 	let mut iter = iter.into_iter();
	// 	if let Some((entity, components)) = iter.next() {
	// 		let cset: generic_array::GenericArray<TypeId, CT::LenTN> = CT::get_tids();
	// 		let mut maps = self.maps.borrow_mut();
	// 		let (group, map_idxs) = if let Some((group, _key, map_idxs)) =
	// 			self.group_sets_to_maps.get_full(cset.as_slice())
	// 		{
	// 			(group, map_idxs)
	// 		} else {
	// 			self.group_sets_to_maps
	// 				.insert(cset.to_vec(), CT::into_type_idx_vec(&mut *maps));
	// 			self.entities
	// 				.borrow_mut()
	// 				.push(Vec::with_capacity(iter.size_hint().0));
	// 			let group = self.group_sets_to_maps.len() - 1;
	// 			for map in maps.values_mut() {
	// 				map.resize(group + 1);
	// 			}
	// 			Self::update_query_mappings(
	// 				&self.group_sets_to_maps,
	// 				&mut *self.query_mappings.borrow_mut(),
	// 				group,
	// 			);
	// 			(group, self.group_sets_to_maps.get_index(group).unwrap().1)
	// 		};
	// 		let mut storage_groups = CT::get_storages_group_mut(&*maps, map_idxs, group);
	// 		let mut reverse = self.reverse.borrow_mut();
	// 		let mut entities = self.entities.borrow_mut();
	// 		Self::insert_valid_location_mut(&mut *reverse, &mut *entities, entity, group)?;
	// 		components.insert_in_group(&mut storage_groups);
	// 		for (entity, components) in iter {
	// 			Self::insert_valid_location_mut(&mut *reverse, &mut *entities, entity, group)?;
	// 			components.insert_in_group(&mut storage_groups);
	// 		}
	// 		Ok(())
	// 	} else {
	// 		// Iterator passed in was empty?
	// 		Ok(())
	// 	}
	// }

	// pub fn extend_iters<C: ComponentSliceSet, EI: ExactSizeIterator<Item = EntityType>>(
	// 	&mut self,
	// 	entity_iter: EI,
	// 	component_slices: C,
	// ) -> Result<(), SparseTypedPagedMapErrors<EntityType>> {
	// 	let mut cset: generic_array::GenericArray<TypeId, C::LenTN> =
	// 		generic_array::GenericArray::from_exact_iter(C::iter_types()).unwrap();
	// 	C::populate_type_slice(cset.as_mut_slice());
	// 	let mut maps = self.maps.borrow_mut();
	// 	if !component_slices.all_same_len(entity_iter.len()) {
	// 		return Err(SparseTypedPagedMapErrors::IteratorsNotAllSameLength);
	// 	}
	// 	let mut entities = self.entities.borrow_mut();
	// 	let (group, map_idxs) = if let Some((group, _key, map_idxs)) =
	// 		self.group_sets_to_maps.get_full(cset.as_slice())
	// 	{
	// 		(group, map_idxs)
	// 	} else {
	// 		self.group_sets_to_maps.insert(
	// 			cset.to_vec(),
	// 			component_slices.into_type_idx_vec(&mut *maps),
	// 		);
	// 		let len = entities.len();
	// 		entities.push(Vec::with_capacity(len));
	// 		let group = self.group_sets_to_maps.len() - 1;
	// 		for map in maps.values_mut() {
	// 			map.resize(group + 1);
	// 		}
	// 		Self::update_query_mappings(
	// 			&self.group_sets_to_maps,
	// 			&mut *self.query_mappings.borrow_mut(),
	// 			group,
	// 		);
	// 		(group, self.group_sets_to_maps.get_index(group).unwrap().1)
	// 	};
	// 	let group_size = entities[group].len();
	// 	// let mut start_idx = component_slices.insert_all(&mut *maps, map_idxs, group);
	// 	component_slices.insert_all(&mut *maps, map_idxs, group);
	// 	let mut reverse = self.reverse.borrow_mut();
	// 	for entity in entity_iter {
	// 		match Self::insert_valid_location_mut(&mut *reverse, &mut *entities, entity, group) {
	// 			Ok(_location) => {
	// 				//location.group = group;
	// 				//location.index = start_idx;
	// 				//start_idx += 1;
	// 				//self.entities[group].push(entity);
	// 			}
	// 			Err(error) => {
	// 				// Truncate only after the error
	// 				//C::truncate(maps, map_idxs, group, start_idx);
	// 				// -- OR --
	// 				// Truncate all that was passed in
	// 				C::truncate(&mut *maps, map_idxs, group, group_size);
	// 				let to_clear: Vec<_> = entities[group].drain(group_size..).collect();
	// 				for entity in to_clear {
	// 					// unwrap should not fail as we just added these
	// 					*reverse.get_mut(entity).unwrap() = ComponentLocations::INVALID;
	// 					// Don't need to remove valid entities via locations because we already did via the drain above
	// 					// let _ =
	// 					// 	Self::remove_valid_location(&mut *reverse, &mut self.entities, entity);
	// 				}
	// 				//reverse.remove_iter(self.entities[group].drain(group_size..));
	// 				// Truncate choice end
	// 				return Err(error);
	// 			}
	// 		}
	// 	}
	// 	Ok(())
	// }

	// pub fn get<DataType: 'static>(
	// 	&self,
	// 	entity: EntityType,
	// ) -> Result<&DataType, SparseTypedPagedMapErrors<EntityType>> {
	// 	let location = self.reverse.get(entity)?;
	// 	if let Some(map) = self.maps.get(&TypeId::of::<DataType>()) {
	// 		let data_map = map.read()?;
	// 		let data_map = data_map.cast::<DataType>();
	// 		return Ok(data_map.get(location.group, location.index).unwrap());
	// 	// if let Some(data) = map
	// 	// 	.read()?
	// 	// 	.cast::<DataType>()
	// 	// 	.get::<DataType>(location.group, location.index)
	// 	// {
	// 	// 	Ok(data)
	// 	// } else {
	// 	// 	Err(SparseTypedPagedMapErrors::EntityDoesNotExistInStorage(
	// 	// 		entity,
	// 	// 		std::any::type_name::<DataType>(),
	// 	// 	))
	// 	// }
	// 	} else {
	// 		Err(SparseTypedPagedMapErrors::ComponentStorageDoesNotExist(
	// 			std::any::type_name::<DataType>(),
	// 		))
	// 	}
	// }
	//
	// pub fn get_mut<DataType: 'static>(
	// 	&mut self,
	// 	entity: EntityType,
	// ) -> Result<&mut DataType, SparseTypedPagedMapErrors<EntityType>> {
	// 	let location = self.reverse.get(entity)?;
	// 	if let Some(map) = self.maps.get_mut(&TypeId::of::<DataType>()) {
	// 		if let Some(data) = map
	// 			.write()?
	// 			.get_mut::<DataType>(location.group, location.index)
	// 		{
	// 			Ok(data)
	// 		} else {
	// 			Err(SparseTypedPagedMapErrors::EntityDoesNotExistInStorage(
	// 				entity,
	// 				std::any::type_name::<DataType>(),
	// 			))
	// 		}
	// 	} else {
	// 		Err(SparseTypedPagedMapErrors::ComponentStorageDoesNotExist(
	// 			std::any::type_name::<DataType>(),
	// 		))
	// 	}
	// }

	// pub fn query<'s, CT: ComponentTupleQuery<'s>>(
	// 	&'s self,
	// ) -> Result<CT::StorageSlices, SparseTypedPagedMapErrors<EntityType>> {
	// 	let include_tids: generic_array::GenericArray<TypeId, CT::LenIncludeTN> =
	// 		CT::get_include_tids();
	// 	let exclude_tids: generic_array::GenericArray<TypeId, CT::LenExcludeTN> =
	// 		CT::get_exclude_tids();
	// 	let query_key = QueryTypedPagedKey {
	// 		include: &include_tids,
	// 		exclude: &exclude_tids,
	// 	};
	// 	let link: &QueryTypedPagedLink = {
	// 		let mut query_mappings = self.query_mappings.borrow_mut();
	// 		query_mappings
	// 			.entry(query_key.to_box())
	// 			.or_insert_with(|| QueryTypedPagedLink {
	// 				include_groups: CT::get_include_matching_query_groups(
	// 					&self.group_sets_to_maps,
	// 					&include_tids,
	// 				),
	// 				// exclude_groups: CT::get_exclude_matching_query_groups(
	// 				// 	&self.group_sets_to_maps,
	// 				// 	&exclude_tids,
	// 				// ),
	// 				include_maps: CT::get_map_idxs(&self.maps, &include_tids),
	// 			})
	// 	};
	// 	if link.include_maps.is_empty() {
	// 		Ok(ComponentPagedIterator {
	// 			storages: None,
	// 			groups: tinyvec::TinyVec::new(),
	// 		})
	// 	} else {
	// 		let storages = CT::get_storages(&self.maps, &link.include_maps);
	// 		Ok(ComponentPagedIterator {
	// 			storages: Some(storages),
	// 			groups: link.include_groups.iter().copied().collect(),
	// 		})
	// 	}
	//
	// 	// let location = self.reverse.get(entity)?;
	// 	// let include_tids: generic_array::GenericArray<TypeId, CT::LenIncludeTN> =
	// 	// 	CT::get_include_tids();
	// 	// let map_idxs = CT::get_map_idxs(&self.maps, &include_tids);
	// 	// let storages = CT::get_storages(&self.maps, map_idxs.as_slice());
	// 	// let (_leftover_storages, slices) = CT::get_storage_slices_at(storages, location.group);
	// 	// let values
	// 	// todo!();
	// }

	pub fn query<CT: ComponentTupleQuery<'static, EntityType>>(
		&self,
	) -> Result<ComponentPagedQuery<EntityType, CT>, SparseTypedPagedMapErrors<EntityType>> {
		let include_tids: generic_array::GenericArray<TypeId, CT::LenIncludeTN> =
			CT::get_include_tids();
		let exclude_tids: generic_array::GenericArray<TypeId, CT::LenExcludeTN> =
			CT::get_exclude_tids();
		let query_key = QueryTypedPagedKey {
			include: &include_tids,
			exclude: &exclude_tids,
		};
		let mut query_mappings = self.query_mappings.borrow_mut();
		let mut group_sets_to_maps = self.group_sets_to_maps.borrow_mut();
		let group = if let Some(group) = group_sets_to_maps.get_index_of(include_tids.as_slice()) {
			group
		} else {
			let mut maps = self.maps.borrow_mut();
			let mut map_idxs = bitvec![0; maps.len()+CT::LenIncludeTN::USIZE];
			for map_idx in CT::get_map_idxs(&mut *maps) {
				map_idxs.set(map_idx, true);
			}
			let map_idxs = map_idxs.into_boxed_bitslice();
			group_sets_to_maps.insert(include_tids.to_vec().into_boxed_slice(), map_idxs);
			self.entities.borrow_mut().push(Vec::with_capacity(1));
			let group = group_sets_to_maps.len() - 1;
			for map in maps.values_mut() {
				map.resize(group + 1);
			}
			SparseTypedPagedMap::<EntityType>::update_query_mappings(
				&*group_sets_to_maps,
				&mut *query_mappings,
				group,
			);
			group
		};
		let link: &QueryTypedPagedLink = {
			query_mappings
				.entry(query_key.to_box())
				.or_insert_with(|| QueryTypedPagedLink {
					include_groups: Rc::new(RefCell::new(CT::get_include_matching_query_groups(
						&*group_sets_to_maps,
						&include_tids,
					))),
					// exclude_groups: CT::get_exclude_matching_query_groups(
					// 	&self.group_sets_to_maps,
					// 	&exclude_tids,
					// ),
					include_maps: CT::get_map_idxs(&mut *self.maps.borrow_mut()),
				})
		};
		Ok(ComponentPagedQuery {
			reverse: self.reverse.clone(),
			entities: self.entities.clone(),
			group_sets_to_maps: self.group_sets_to_maps.clone(),
			query_mappings: self.query_mappings.clone(),
			maps: self.maps.clone(),
			storages: CT::get_storages(&self.entities, &*self.maps.borrow(), &link.include_maps),
			group: group,
			groups: link.include_groups.clone(),
		})
	}
	/*
	pub fn iter<'a, CS: ComponentStorageSet<'a>>(
		&'a self,
		// ) -> Result<ComponentIterSetIntoIterator<CS>, SparseTypedPagedMapErrors<EntityType>> {
		// ) -> Result<
		// 	//std::iter::Map<std::iter::Rev<std::vec::IntoIter<usize>>, F>,
		// 	Box<dyn Iterator<Item = <CS::Storages as ComponentIteratorSet<'a>>::IteratorItem> + 'a>,
		// 	SparseTypedPagedMapErrors<EntityType>,
		// >
	) -> Result<
		StorageGroupIterator<'a, CS::Storages, std::slice::Iter<usize>>,
		SparseTypedPagedMapErrors<EntityType>,
	>
	where
		CS::Storages: ComponentIteratorSet<'a>,
		<CS::Storages as ComponentIteratorSet<'a>>::IteratorItem: 'a,
	{
		let include_cset: generic_array::GenericArray<TypeId, <CS::IncludeSet as TypeList>::LenTN> =
			generic_array::GenericArray::from_iter(
				(0..<CS::IncludeSet as TypeList>::LenTN::USIZE)
					.map(|i| CS::get_include_type_id_at(i).unwrap()),
			);
		let exclude_cset: generic_array::GenericArray<TypeId, <CS::ExcludeSet as TypeList>::LenTN> =
			generic_array::GenericArray::from_iter(
				(0..<CS::ExcludeSet as TypeList>::LenTN::USIZE)
					.map(|i| CS::get_exclude_type_id_at(i).unwrap()),
			);
		let query_key = QueryTypedPagedKey::<CS::IncludeSet, CS::ExcludeSet> {
			include: include_cset,
			exclude: exclude_cset,
		};

		let (storages, groups) = {
			let mut query_mappings = self.query_mappings.borrow_mut();
			if let Some((_idx, _key, link)) = query_mappings.get_full(&query_key) {
				let groups = link.include_groups.clone();
				(
					CS::get_storages(&self.maps, link.include_maps.as_slice()),
					groups,
				)
			} else {
				// let group_size = if let Some((_tid, map)) = self.maps.get_index(0) {
				// 	map.borrow().len_groups()
				// } else {
				// 	0
				// };
				let query_link = QueryTypedPagedLink {
					include_groups: CS::get_include_matching_query_groups(
						&self.group_sets_to_maps,
						vec![],
					),
					// exclude_groups: CS::get_exclude_matching_query_groups(
					// 	&self.group_sets_to_maps,
					// 	vec![],
					// ),
					include_maps: CS::get_map_idxs(&self.maps, vec![]),
				};
				query_mappings.insert(query_key.to_box(), query_link);
				let link = query_mappings
					.get_index(query_mappings.len() - 1)
					.unwrap()
					.1;
				let groups = link.include_groups.clone();
				(
					CS::get_storages(&self.maps, link.include_maps.as_slice()),
					groups,
				)
			}
		};

		if let Some(storages) = storages {
			// Ok(ComponentIterSetIntoIterator { storages, groups })
			// let iter = groups
			// 	.into_iter()
			// 	.rev()
			// 	.map(move |group| storages.get_group_slice(group));
			// Ok(Box::new(iter))
			Ok(StorageGroupIterator {
				//_phantom: Default::default(),
				groups: todo!(), //groups.as_slice().iter(),
				storages,
			})
		} else {
			todo!()
		}
	}
	*/

	// pub fn query<RO: ComponentSet, RW: ComponentSet, E: ComponentSet>(
	// 	&mut self,
	// ) -> Result<Query<EntityType, RO, RW, E>, SparseTypedPagedMapErrors<EntityType>> {
	// 	let query_key = QueryTypedPagedKey::<RO, RW, E>::new();
	//
	// 	if let Some((idx, _key, link)) = self.query_mappings.get_full(&query_key) {
	// 		Ok(Query::new(idx, link, &self.maps))
	// 	} else {
	// 		let group_size = if let Some((_tid, map)) = self.maps.get_index(0) {
	// 			map.borrow().len_groups()
	// 		} else {
	// 			0
	// 		};
	// 		RO::ensure_exists(&mut self.maps, group_size);
	// 		RW::ensure_exists(&mut self.maps, group_size);
	// 		E::ensure_exists(&mut self.maps, group_size);
	// 		let query_link = QueryTypedPagedLink {
	// 			read_only_groups: RO::get_matching_query_groups(&self.group_sets_to_maps),
	// 			read_write_groups: RW::get_matching_query_groups(&self.group_sets_to_maps),
	// 			except_groups: E::get_matching_query_groups(&self.group_sets_to_maps),
	// 			read_only_maps: RO::into_type_idx_vec(&mut self.maps),
	// 			read_write_maps: RW::into_type_idx_vec(&mut self.maps),
	// 		};
	// 		let idx = self.query_mappings.len();
	// 		self.query_mappings.insert(query_key.to_box(), query_link);
	// 		Ok(Query::new(
	// 			idx,
	// 			self.query_mappings.get_index(idx).unwrap().1,
	// 			&self.maps,
	// 		))
	// 	}
	// }
}

pub struct ComponentPagedQuery<EntityType: Entity, CT: ComponentTupleQuery<'static, EntityType>> {
	reverse: Rc<RefCell<SecondaryIndex<EntityType, ComponentLocations>>>,
	entities: Rc<RefCell<Vec<Vec<EntityType>>>>,
	group_sets_to_maps: Rc<RefCell<GroupTypeSetToMapSet>>,
	query_mappings: Rc<RefCell<IndexMap<QueryTypedPagedKeyBoxed, QueryTypedPagedLink>>>,
	maps: Rc<RefCell<MapIndexMap>>,
	storages: CT::Storages,
	group: usize,
	groups: Rc<RefCell<Vec<usize>>>,
}

pub struct ComponentPagedQueryLocked<
	'a,
	EntityType: Entity,
	CT: ComponentTupleQuery<'static, EntityType>,
> {
	reverse: RefMut<'a, SecondaryIndex<EntityType, ComponentLocations>>,
	entities: RefMut<'a, Vec<Vec<EntityType>>>,
	group_sets_to_maps: RefMut<'a, GroupTypeSetToMapSet>,
	query_mappings: &'a Rc<RefCell<IndexMap<QueryTypedPagedKeyBoxed, QueryTypedPagedLink>>>,
	maps: RefMut<'a, MapIndexMap>,
	storages: CT::StoragesLocked,
	group: usize,
	groups: &'a Rc<RefCell<Vec<usize>>>,
}

impl<'a, EntityType: Entity, CT: ComponentTupleQuery<'static, EntityType>>
	ComponentPagedQueryLocked<'a, EntityType, CT>
{
	pub fn insert(
		&mut self,
		entity: EntityType,
		components: CT::StorageMovedValues,
	) -> Result<(), SparseTypedPagedMapErrors<EntityType>> {
		SparseTypedPagedMap::insert_valid_location_mut(
			&mut *self.reverse,
			&mut *self.entities,
			entity,
			self.group,
		)?;
		CT::insert(components, &mut self.storages, self.group);
		Ok(())
	}

	pub fn extend(
		&mut self,
		entities: impl IntoIterator<Item = (EntityType, CT::StorageMovedValues)>,
	) -> Result<(), SparseTypedPagedMapErrors<EntityType>> {
		for (entity, components) in entities {
			SparseTypedPagedMap::insert_valid_location_mut(
				&mut *self.reverse,
				&mut *self.entities,
				entity,
				self.group,
			)?;
			CT::insert(components, &mut self.storages, self.group);
		}

		Ok(())
	}

	/// All of the components iterators must be no shorter than the `entities` iterator.
	pub fn extend_vecs<
		's,
		EI: IntoIterator<Item = EntityType>,
		//CTT: 's + ComponentTupleQuery<'s, EntityType>,
	>(
		&'s mut self,
		entities: EI,
		components: CT::StorageMovedVecs, //CTT::StorageMovedVecs,
	) -> Result<(), SparseTypedPagedMapErrors<EntityType>> {
		//let mut cast_storages = CTT::cast_locked_storages::<CT>(&mut self.storages);
		let mut count = 0;
		for entity in entities {
			SparseTypedPagedMap::insert_valid_location_mut(
				&mut *self.reverse,
				&mut *self.entities,
				entity,
				self.group,
			)?;
			count += 1;
		}
		//CTT::extend_vecs(
		CT::extend_vecs(
			components,
			//&mut cast_storages,
			&mut self.storages,
			self.group,
			count,
		);

		Ok(())
	}

	pub fn get<'s, CTT: 's + ComponentTupleQuery<'s, EntityType>>(
		&'s mut self,
		entity: EntityType,
	) -> Result<CTT::StorageValues, SparseTypedPagedMapErrors<EntityType>> {
		let location =
			SparseTypedPagedMap::get_valid_location(&*self.reverse, &*self.entities, entity)?;
		let mut cast_storages = CTT::cast_locked_storages::<CT>(&mut self.storages);
		if let Some(value) = CTT::get_storage_values_at(
			// TODO:  LACK OF GAT's IS SO PAINFUL!  FIX THIS WHEN GAT's EXIST!
			// This 'should' be safeish as it's just casting lifetimes to a more constrained lifetime
			unsafe { &mut *(&mut cast_storages as *mut CTT::StoragesLockedRef) },
			location.group,
			location.index,
		) {
			Ok(value)
		} else {
			Err(SparseTypedPagedMapErrors::EntityDoesNotExistInStorage(
				entity, "",
			))
		}
	}
}

impl<EntityType: Entity, CT: ComponentTupleQuery<'static, EntityType>>
	ComponentPagedQuery<EntityType, CT>
{
	pub fn lock<'s>(&'s mut self) -> ComponentPagedQueryLocked<'s, EntityType, CT> {
		ComponentPagedQueryLocked::<'s, EntityType, CT> {
			reverse: self.reverse.borrow_mut(),
			entities: self.entities.borrow_mut(),
			group_sets_to_maps: self.group_sets_to_maps.borrow_mut(),
			query_mappings: &self.query_mappings,
			maps: self.maps.borrow_mut(),
			// TODO:  NEED GAT's!  AAAAAHHHHHH!
			// This unsafe is 'fine' because we are 'constraining' the lifetime, not widening it
			storages: CT::get_locked_storages(&self.storages),
			group: self.group,
			groups: &self.groups,
		}
	}

	// pub fn insert(&mut self, entity: EntityType, data: CT) -> () {
	// 	SparseTypedPagedMap::insert_valid_location_mut()
	//
	// 	// 	Self::insert_valid_location_mut(
	// 	// 		&mut *self.reverse.borrow_mut(),
	// 	// 		&mut *self.entities.borrow_mut(),
	// 	// 		entity,
	// 	// 		group,
	// 	// 	)?;
	// 	// 	components.insert(&mut *maps, map_idxs, group);
	// }

	// pub fn get(&mut self, entity: EntityType) -> Option<CT::StorageValues> {
	// 	if let Ok(location) = self.reverse.borrow().get(entity) {
	// 		CT::get_storage_values_at(&mut self.storages, location.group, location.index)
	// 	} else {
	// 		None
	// 	}
	// }

	pub fn iter_slices(&self) -> ComponentPagedIterator<EntityType, CT> {
		ComponentPagedIterator {
			_phantom: PhantomData,
			//reverse: self.reverse.clone(),
			storages: self.storages.clone(),
			groups: self.groups.borrow().iter().copied().collect(),
		}
	}

	// pub fn iter(&self) -> ComponentPagedFlatIterator<EntityType, CT> {
	// 	ComponentPagedFlatIterator {
	// 		// _phantom: PhantomData,
	// 		//reverse: self.reverse.clone(),
	// 		// storages: self.storages.clone(),
	// 		iter: self.iter_slices(),
	// 		slices: None,
	// 		// groups: self.groups.borrow().iter().copied().collect(),
	// 	}
	// }
}

// impl<EntityType: Entity, CT: ComponentTupleQuery> IntoIterator
// 	for ComponentPagedQuery<EntityType, CT>
// {
// 	type Item = CT::StorageSlices;
// 	type IntoIter = ComponentPagedIterator<EntityType, CT>;
//
// 	fn into_iter(self) -> Self::IntoIter {
// 		ComponentPagedIterator {
// 			_phantom: PhantomData,
// 			//reverse: self.reverse.clone(),
// 			storages: self.storages,
// 			groups: self.groups.borrow().iter().copied().collect(),
// 		}
// 	}
// }

// pub struct ComponentPagedFlatIterator<EntityType: Entity, CT: ComponentTupleQuery> {
// 	iter: ComponentPagedIterator<EntityType, CT>,
// 	slices: CT::StorageSlices,
// }
//
// impl<EntityType: Entity, CT: ComponentTupleQuery> Iterator
// 	for ComponentPagedFlatIterator<EntityType, CT>
// {
// 	type Item = CT::StorageValues;
//
// 	fn next(&mut self) -> Option<Self::Item> {
// 		loop {
// 			if let Some(next) = CT::get_next_values_from_slices(&mut self.slices) {
// 				return Some(next);
// 			}
// 			if let Some(slices) = self.iter.next() {
// 				self.slices = slices;
// 			} else {
// 				return None;
// 			}
// 		}
// 	}
// }

pub struct ComponentPagedIterator<EntityType: Entity, CT: ComponentTupleQuery<'static, EntityType>>
{
	_phantom: PhantomData<EntityType>,
	//reverse: Rc<RefCell<SecondaryIndex<EntityType, ComponentLocations>>>,
	storages: CT::Storages,
	groups: tinyvec::TinyVec<[usize; 16]>,
}

impl<EntityType: Entity, CT: ComponentTupleQuery<'static, EntityType>> Iterator
	for ComponentPagedIterator<EntityType, CT>
{
	type Item = CT::StorageSlices;

	fn next(&mut self) -> Option<Self::Item> {
		while let Some(group) = self.groups.pop() {
			let next = CT::get_storage_slices_at(&self.storages, group);
			if next.is_some() {
				return next;
			}
		}
		None
	}
}

pub trait ComponentQuery<'a, EntityType> {
	type RawType: 'static;
	fn get_self_typeid() -> TypeId;

	type LenIncludeTN: generic_array::typenum::Unsigned + generic_array::ArrayLength<TypeId>;
	type LenExcludeTN: generic_array::typenum::Unsigned + generic_array::ArrayLength<TypeId>;
	type LenKeysetTN: generic_array::typenum::Unsigned + generic_array::ArrayLength<TypeId>;
	fn get_include_tid() -> Option<std::any::TypeId>;
	fn get_exclude_tid() -> Option<std::any::TypeId>;
	fn get_keyset_tid() -> Option<std::any::TypeId>;
	fn push_matching_include_query_group(
		groups_to_maps: &IndexMap<Vec<TypeId>, Vec<usize>>,
		out: &mut Vec<usize>,
	);
	fn push_matching_exclude_query_group(
		groups_to_maps: &IndexMap<Vec<TypeId>, Vec<usize>>,
		out: &mut Vec<usize>,
	);
	type Storage: Clone;
	fn get_storage(
		entities: &Rc<RefCell<Vec<Vec<EntityType>>>>,
		maps: &MapIndexMap,
		map_id: usize,
	) -> Self::Storage;
	type StorageSlice;
	fn get_storage_slice_at(storage: &Self::Storage, group: usize) -> Option<Self::StorageSlice>; //(Self::Storage, Self::StorageSlice);
	type StorageValue: 'a;
	fn get_storage_value_at(
		locked_storage: &'a mut Self::StorageLocked,
		group: usize,
		index: usize,
	) -> Option<Self::StorageValue>;
	type StorageLocked;
	fn get_locked_storage(storage: &Self::Storage) -> Self::StorageLocked;
	type StorageMovedValue: 'static;
	fn into_keyset_mapidx_vec(locked_storage: &Self::StorageLocked, out: &mut BitVec);
	fn resize_locked_storage_groups(locked_storage: &mut Self::StorageLocked, new_size: usize);
	fn insert(
		this: Self::StorageMovedValue,
		locked_storage: &mut Self::StorageLocked,
		group: usize,
	);
	fn extend_vec(
		this: Vec<Self::StorageMovedValue>,
		locked_storage: &mut Self::StorageLocked,
		group: usize,
		count: usize,
	);
}

pub struct EntityRef;

impl<'a, EntityType: Entity> ComponentQuery<'a, EntityType> for EntityRef {
	type RawType = EntityType;
	#[inline(always)]
	fn get_self_typeid() -> TypeId {
		TypeId::of::<EntityType>()
	}

	type LenIncludeTN = generic_array::typenum::U0;
	type LenExcludeTN = generic_array::typenum::U0;
	type LenKeysetTN = generic_array::typenum::U0;

	#[inline(always)]
	fn get_include_tid() -> Option<TypeId> {
		None
	}
	#[inline(always)]
	fn get_exclude_tid() -> Option<TypeId> {
		None
	}
	#[inline(always)]
	fn get_keyset_tid() -> Option<TypeId> {
		None
	}

	#[inline(always)]
	fn push_matching_include_query_group(
		_groups_to_maps: &IndexMap<Vec<TypeId>, Vec<usize>, RandomState>,
		_out: &mut Vec<usize>,
	) {
	}
	#[inline(always)]
	fn push_matching_exclude_query_group(
		_groups_to_maps: &IndexMap<Vec<TypeId>, Vec<usize>, RandomState>,
		_out: &mut Vec<usize>,
	) {
	}

	type Storage = Rc<RefCell<Vec<Vec<EntityType>>>>;
	#[inline(always)]
	fn get_storage(
		entities: &Rc<RefCell<Vec<Vec<EntityType>>>>,
		_maps: &MapIndexMap,
		_map_id: usize,
	) -> Self::Storage {
		entities.clone()
	}

	type StorageSlice = OwningRef<
		OwningHandle<Rc<RefCell<Vec<Vec<EntityType>>>>, Ref<'static, Vec<Vec<EntityType>>>>,
		[Self::RawType],
	>;
	#[inline(always)]
	fn get_storage_slice_at(storage: &Self::Storage, group: usize) -> Option<Self::StorageSlice> {
		let owned = OwningHandle::new(storage.clone());
		OwningRef::new(owned)
			.try_map(|s| match s[group].as_slice() {
				&[] => Err(()),
				slice => Ok(slice),
			})
			.ok()
	}

	type StorageValue = EntityType;
	#[inline(always)]
	fn get_storage_value_at(
		locked_storage: &'a mut Self::StorageLocked,
		group: usize,
		index: usize,
	) -> Option<Self::StorageValue> {
		let group = locked_storage.get(group)?;
		if group.len() > index {
			Some(group[index])
		} else {
			None
		}
	}

	type StorageLocked =
		OwningHandle<Rc<RefCell<Vec<Vec<EntityType>>>>, Ref<'static, Vec<Vec<EntityType>>>>;
	#[inline(always)]
	fn get_locked_storage(storage: &Self::Storage) -> Self::StorageLocked {
		OwningHandle::new(storage.clone())
	}

	type StorageMovedValue = ();
	#[inline(always)]
	fn into_keyset_mapidx_vec(
		_locked_storage: &Self::StorageLocked,
		_out: &mut BitVec<Lsb0, usize>,
	) {
	}
	#[inline(always)]
	fn resize_locked_storage_groups(_locked_storage: &mut Self::StorageLocked, _new_size: usize) {}
	#[inline(always)]
	fn insert(
		_this: Self::StorageMovedValue,
		_locked_storages: &mut Self::StorageLocked,
		_group: usize,
	) {
	}

	fn extend_vec(
		_this: Vec<Self::StorageMovedValue>,
		_locked_storage: &mut Self::StorageLocked,
		_group: usize,
		_count: usize,
	) {
		panic!("can't insert EntityRef's");
	}
}

impl<'a, EntityType: Entity, T: 'static> ComponentQuery<'a, EntityType> for &T {
	type RawType = T;
	#[inline(always)]
	fn get_self_typeid() -> TypeId {
		TypeId::of::<&Self::RawType>()
	}

	type LenIncludeTN = generic_array::typenum::U1;
	type LenExcludeTN = generic_array::typenum::U0;
	type LenKeysetTN = generic_array::typenum::U1;
	#[inline(always)]
	fn get_include_tid() -> Option<std::any::TypeId> {
		Some(std::any::TypeId::of::<T>())
	}
	#[inline(always)]
	fn get_exclude_tid() -> Option<std::any::TypeId> {
		None // Do nothing as this is not an exclude
	}
	#[inline(always)]
	fn get_keyset_tid() -> Option<TypeId> {
		Some(std::any::TypeId::of::<T>())
	}

	#[inline]
	fn push_matching_include_query_group(
		_groups_to_maps: &IndexMap<Vec<TypeId>, Vec<usize>>,
		_out: &mut Vec<usize>,
	) {
	}
	#[inline]
	fn push_matching_exclude_query_group(
		_groups_to_maps: &IndexMap<Vec<TypeId>, Vec<usize>>,
		_out: &mut Vec<usize>,
	) {
		// Do nothing as this is not an exclude
	}

	type Storage = Rc<RefCell<DensePagedDataActual<Self::RawType>>>;
	#[inline]
	fn get_storage(
		_entities: &Rc<RefCell<Vec<Vec<EntityType>>>>,
		maps: &MapIndexMap,
		map_id: usize,
	) -> Self::Storage {
		maps.get_index(map_id)
			.unwrap()
			.1
			.get_strong::<Self::RawType>()
	}

	type StorageSlice = OwningRef<
		OwningHandle<
			Rc<RefCell<DensePagedDataActual<Self::RawType>>>,
			Ref<'static, DensePagedDataActual<Self::RawType>>,
		>,
		[Self::RawType],
	>;
	#[inline]
	fn get_storage_slice_at(storage: &Self::Storage, group: usize) -> Option<Self::StorageSlice> {
		let owned = OwningHandle::new(storage.clone());
		OwningRef::new(owned)
			.try_map(|s| match s.data[group].as_slice() {
				&[] => Err(()),
				slice => Ok(slice),
			})
			.ok()
	}

	// type StorageValue = OwningRef<
	// 	OwningHandle<
	// 		Rc<RefCell<DensePagedDataActual<Self::RawType>>>,
	// 		Ref<'static, DensePagedDataActual<Self::RawType>>,
	// 	>,
	// 	Self::RawType,
	// >;
	type StorageValue = &'a T;
	#[inline]
	fn get_storage_value_at(
		locked_storage: &'a mut Self::StorageLocked,
		group: usize,
		index: usize,
	) -> Option<Self::StorageValue> {
		if let Some(group) = locked_storage.data.get(group) {
			group.get(index)
		} else {
			None
		}
		// 	OwningRef::new(locked_storage)
		// 		.try_map(|s| {
		// 			let slice = &s.data[group];
		// 			if slice.len() > index {
		// 				Ok(&slice[index])
		// 			} else {
		// 				Err(())
		// 			}
		// 		})
		// 		.ok()
	}

	type StorageLocked = OwningHandle<
		Rc<RefCell<DensePagedDataActual<Self::RawType>>>,
		Ref<'static, DensePagedDataActual<Self::RawType>>,
	>;
	#[inline(always)]
	fn get_locked_storage(storage: &Self::Storage) -> Self::StorageLocked {
		OwningHandle::new(storage.clone())
	}

	type StorageMovedValue = ();
	#[inline(always)]
	fn into_keyset_mapidx_vec(locked_storage: &Self::StorageLocked, out: &mut BitVec<Lsb0, usize>) {
		out.set(locked_storage.index, true);
	}
	#[inline(always)]
	fn resize_locked_storage_groups(_locked_storage: &mut Self::StorageLocked, _new_size: usize) {}
	#[inline(always)]
	fn insert(
		_this: Self::StorageMovedValue,
		_locked_storages: &mut Self::StorageLocked,
		_group: usize,
	) {
		panic!("cannot insert references");
	}

	fn extend_vec(
		_this: Vec<Self::StorageMovedValue>,
		_locked_storage: &mut Self::StorageLocked,
		_group: usize,
		_count: usize,
	) {
		panic!("cannot insert references");
	}
}

pub struct Exclude<T: 'static> {
	_phantom: PhantomData<T>,
}

impl<'a, EntityType: Entity, T: 'static> ComponentQuery<'a, EntityType> for Exclude<T> {
	type RawType = T;
	#[inline(always)]
	fn get_self_typeid() -> TypeId {
		TypeId::of::<Exclude<Self::RawType>>()
	}

	type LenIncludeTN = generic_array::typenum::U0;
	type LenExcludeTN = generic_array::typenum::U1;
	type LenKeysetTN = generic_array::typenum::U0;

	#[inline(always)]
	fn get_include_tid() -> Option<std::any::TypeId> {
		None // Do nothing as this is an exclude
	}
	#[inline(always)]
	fn get_exclude_tid() -> Option<std::any::TypeId> {
		Some(std::any::TypeId::of::<T>())
	}
	#[inline(always)]
	fn get_keyset_tid() -> Option<TypeId> {
		None
	}

	#[inline]
	fn push_matching_include_query_group(
		_groups_to_maps: &IndexMap<Vec<TypeId>, Vec<usize>>,
		_out: &mut Vec<usize>,
	) {
		// Do nothing as this is not an exclude
	}
	#[inline]
	fn push_matching_exclude_query_group(
		_groups_to_maps: &IndexMap<Vec<TypeId>, Vec<usize>>,
		_out: &mut Vec<usize>,
	) {
	}

	type Storage = Rc<RefCell<DensePagedDataActual<Self::RawType>>>;
	#[inline]
	fn get_storage(
		_entities: &Rc<RefCell<Vec<Vec<EntityType>>>>,
		maps: &MapIndexMap,
		map_id: usize,
	) -> Self::Storage {
		maps.get_index(map_id)
			.unwrap()
			.1
			.get_strong::<Self::RawType>()
	}

	type StorageSlice = ();
	#[inline]
	fn get_storage_slice_at(storage: &Self::Storage, group: usize) -> Option<Self::StorageSlice> {
		if storage.borrow().data[group].is_empty() {
			Some(())
		} else {
			None
		}
	}

	type StorageValue = ();
	#[inline]
	fn get_storage_value_at(
		locked_storage: &'a mut Self::StorageLocked,
		group: usize,
		index: usize,
	) -> Option<Self::StorageValue> {
		if let Some(group) = locked_storage.data.get(group) {
			if group.get(index).is_some() {
				return None;
			}
		}
		Some(())
	}

	type StorageLocked = OwningHandle<
		Rc<RefCell<DensePagedDataActual<Self::RawType>>>,
		Ref<'static, DensePagedDataActual<Self::RawType>>,
	>;
	#[inline(always)]
	fn get_locked_storage(storage: &Self::Storage) -> Self::StorageLocked {
		OwningHandle::new(storage.clone())
	}

	type StorageMovedValue = ();
	#[inline(always)]
	fn into_keyset_mapidx_vec(
		_locked_storage: &Self::StorageLocked,
		_out: &mut BitVec<Lsb0, usize>,
	) {
	}
	#[inline(always)]
	fn resize_locked_storage_groups(_locked_storage: &mut Self::StorageLocked, _new_size: usize) {}
	#[inline(always)]
	fn insert(
		_this: Self::StorageMovedValue,
		_locked_storages: &mut Self::StorageLocked,
		_group: usize,
	) {
		panic!("cannot insert excludes");
	}

	fn extend_vec(
		_this: Vec<Self::StorageMovedValue>,
		_locked_storage: &mut Self::StorageLocked,
		_group: usize,
		_count: usize,
	) {
		panic!("cannot insert excludes");
	}
}

impl<'a, EntityType: Entity, T: 'static> ComponentQuery<'a, EntityType> for Option<&T> {
	type RawType = T;
	#[inline(always)]
	fn get_self_typeid() -> TypeId {
		TypeId::of::<Option<&Self::RawType>>()
	}

	type LenIncludeTN = generic_array::typenum::U1;
	type LenExcludeTN = generic_array::typenum::U0;
	type LenKeysetTN = generic_array::typenum::U1;

	#[inline(always)]
	fn get_include_tid() -> Option<std::any::TypeId> {
		Some(std::any::TypeId::of::<T>())
	}
	#[inline(always)]
	fn get_exclude_tid() -> Option<std::any::TypeId> {
		None
	}
	#[inline(always)]
	fn get_keyset_tid() -> Option<TypeId> {
		Some(std::any::TypeId::of::<T>())
	}

	#[inline]
	fn push_matching_include_query_group(
		_groups_to_maps: &IndexMap<Vec<TypeId>, Vec<usize>>,
		_out: &mut Vec<usize>,
	) {
	}
	#[inline]
	fn push_matching_exclude_query_group(
		_groups_to_maps: &IndexMap<Vec<TypeId>, Vec<usize>>,
		_out: &mut Vec<usize>,
	) {
		// Do nothing as this is not an exclude
	}
	type Storage = Rc<RefCell<DensePagedDataActual<Self::RawType>>>;
	#[inline]
	fn get_storage(
		_entities: &Rc<RefCell<Vec<Vec<EntityType>>>>,
		maps: &MapIndexMap,
		map_id: usize,
	) -> Self::Storage {
		maps.get_index(map_id)
			.unwrap()
			.1
			.get_strong::<Self::RawType>()
	}

	type StorageSlice = Option<
		OwningRef<
			OwningHandle<
				Rc<RefCell<DensePagedDataActual<Self::RawType>>>,
				Ref<'static, DensePagedDataActual<Self::RawType>>,
			>,
			[Self::RawType],
		>,
	>;
	#[inline]
	fn get_storage_slice_at(storage: &Self::Storage, group: usize) -> Option<Self::StorageSlice> {
		let owned = OwningHandle::new(storage.clone());
		OwningRef::new(owned)
			.try_map(|s| {
				let slice = s.data[group].as_slice();
				if slice.len() > 0 {
					Ok(slice)
				} else {
					Err(())
				}
			})
			.map_or(Some(None), |slice| Some(Some(slice)))
	}

	// type StorageValue = Option<
	// 	OwningRef<
	// 		OwningHandle<
	// 			Rc<RefCell<DensePagedDataActual<Self::RawType>>>,
	// 			Ref<'static, DensePagedDataActual<Self::RawType>>,
	// 		>,
	// 		Self::RawType,
	// 	>,
	// >;
	type StorageValue = Option<&'a T>;

	#[inline]
	fn get_storage_value_at(
		locked_storage: &'a mut Self::StorageLocked,
		group: usize,
		index: usize,
	) -> Option<Self::StorageValue> {
		if let Some(group) = locked_storage.data.get(group) {
			Some(group.get(index))
		} else {
			Some(None)
		}
		// OwningRef::new(locked_storage)
		// 	.try_map(|s| {
		// 		let slice = &s.data[group];
		// 		if slice.len() > index {
		// 			Ok(&slice[index])
		// 		} else {
		// 			Err(())
		// 		}
		// 	})
		// 	.map_or(Some(None), |slice| Some(Some(slice)))
	}

	type StorageLocked = OwningHandle<
		Rc<RefCell<DensePagedDataActual<Self::RawType>>>,
		Ref<'static, DensePagedDataActual<Self::RawType>>,
	>;
	#[inline(always)]
	fn get_locked_storage(storage: &Self::Storage) -> Self::StorageLocked {
		OwningHandle::new(storage.clone())
	}

	type StorageMovedValue = ();
	#[inline(always)]
	fn into_keyset_mapidx_vec(locked_storage: &Self::StorageLocked, out: &mut BitVec<Lsb0, usize>) {
		out.set(locked_storage.index, true);
	}
	#[inline(always)]
	fn resize_locked_storage_groups(_locked_storage: &mut Self::StorageLocked, _new_size: usize) {}
	#[inline(always)]
	fn insert(
		_this: Self::StorageMovedValue,
		_locked_storages: &mut Self::StorageLocked,
		_group: usize,
	) {
		panic!("cannot insert references");
	}

	fn extend_vec(
		_this: Vec<Self::StorageMovedValue>,
		_locked_storage: &mut Self::StorageLocked,
		_group: usize,
		_count: usize,
	) {
		panic!("cannot insert references");
	}
}

impl<'a, EntityType: Entity, T: 'static> ComponentQuery<'a, EntityType> for &mut T {
	type RawType = T;
	#[inline(always)]
	fn get_self_typeid() -> TypeId {
		TypeId::of::<&mut Self::RawType>()
	}

	type LenIncludeTN = generic_array::typenum::U1;
	type LenExcludeTN = generic_array::typenum::U0;
	type LenKeysetTN = generic_array::typenum::U1;

	#[inline(always)]
	fn get_include_tid() -> Option<std::any::TypeId> {
		Some(std::any::TypeId::of::<T>())
	}
	#[inline(always)]
	fn get_exclude_tid() -> Option<std::any::TypeId> {
		None // Do nothing as this is not an exclude
	}
	#[inline(always)]
	fn get_keyset_tid() -> Option<TypeId> {
		Some(std::any::TypeId::of::<T>())
	}

	#[inline]
	fn push_matching_include_query_group(
		_groups_to_maps: &IndexMap<Vec<TypeId>, Vec<usize>>,
		_out: &mut Vec<usize>,
	) {
		todo!();
	}
	#[inline]
	fn push_matching_exclude_query_group(
		_groups_to_maps: &IndexMap<Vec<TypeId>, Vec<usize>>,
		_out: &mut Vec<usize>,
	) {
		// Do nothing as this is not an exclude
	}
	type Storage = Rc<RefCell<DensePagedDataActual<Self::RawType>>>;
	#[inline]
	fn get_storage(
		_entities: &Rc<RefCell<Vec<Vec<EntityType>>>>,
		maps: &MapIndexMap,
		map_id: usize,
	) -> Self::Storage {
		maps.get_index(map_id)
			.unwrap()
			.1
			.get_strong::<Self::RawType>()
	}

	type StorageSlice = OwningRefMut<
		OwningHandle<
			Rc<RefCell<DensePagedDataActual<Self::RawType>>>,
			RefMut<'static, DensePagedDataActual<Self::RawType>>,
		>,
		[Self::RawType],
	>;
	#[inline]
	fn get_storage_slice_at(storage: &Self::Storage, group: usize) -> Option<Self::StorageSlice> {
		let owned = OwningHandle::new_mut(storage.clone());
		OwningRefMut::new(owned)
			.try_map_mut(|s| match s.data[group].as_mut_slice() {
				&mut [] => Err(()),
				slice => Ok(slice),
			})
			.ok()
	}

	// type StorageValue = OwningRefMut<
	// 	OwningHandle<
	// 		Rc<RefCell<DensePagedDataActual<Self::RawType>>>,
	// 		RefMut<'static, DensePagedDataActual<Self::RawType>>,
	// 	>,
	// 	Self::RawType,
	// >;
	type StorageValue = &'a mut T;
	#[inline]
	fn get_storage_value_at(
		locked_storage: &'a mut Self::StorageLocked,
		group: usize,
		index: usize,
	) -> Option<Self::StorageValue> {
		if let Some(group) = locked_storage.data.get_mut(group) {
			group.get_mut(index)
		} else {
			None
		}
		// OwningRefMut::new(locked_storage)
		// 	.try_map_mut(|s| {
		// 		let slice = &mut s.data[group];
		// 		if slice.len() > index {
		// 			Ok(&mut slice[index])
		// 		} else {
		// 			Err(())
		// 		}
		// 	})
		// 	.ok()
	}

	type StorageLocked = OwningHandle<
		Rc<RefCell<DensePagedDataActual<Self::RawType>>>,
		RefMut<'static, DensePagedDataActual<Self::RawType>>,
	>;
	#[inline(always)]
	fn get_locked_storage(storage: &Self::Storage) -> Self::StorageLocked {
		OwningHandle::new_mut(storage.clone())
	}

	type StorageMovedValue = T;
	#[inline(always)]
	fn into_keyset_mapidx_vec(locked_storage: &Self::StorageLocked, out: &mut BitVec<Lsb0, usize>) {
		out.set(locked_storage.index, true);
	}
	#[inline(always)]
	fn resize_locked_storage_groups(locked_storage: &mut Self::StorageLocked, new_size: usize) {
		locked_storage.data.resize_with(new_size, Vec::new);
	}
	#[inline(always)]
	fn insert(
		this: Self::StorageMovedValue,
		locked_storages: &mut Self::StorageLocked,
		group: usize,
	) {
		locked_storages.data[group].push(this);
	}

	fn extend_vec(
		this: Vec<Self::StorageMovedValue>,
		locked_storage: &mut Self::StorageLocked,
		group: usize,
		count: usize,
	) {
		let data = &mut locked_storage.data[group];
		let len = data.len();
		data.extend(this);
		assert_eq!(data.len() - len, count);
	}
}

impl<'a, EntityType: Entity, T: 'static> ComponentQuery<'a, EntityType> for Option<&mut T> {
	type RawType = T;
	#[inline(always)]
	fn get_self_typeid() -> TypeId {
		TypeId::of::<Option<&mut Self::RawType>>()
	}

	type LenIncludeTN = generic_array::typenum::U1;
	type LenExcludeTN = generic_array::typenum::U0;
	type LenKeysetTN = generic_array::typenum::U1;

	#[inline(always)]
	fn get_include_tid() -> Option<std::any::TypeId> {
		Some(std::any::TypeId::of::<T>())
	}
	#[inline(always)]
	fn get_exclude_tid() -> Option<std::any::TypeId> {
		None
	}
	#[inline(always)]
	fn get_keyset_tid() -> Option<TypeId> {
		Some(std::any::TypeId::of::<T>())
	}

	#[inline]
	fn push_matching_include_query_group(
		_groups_to_maps: &IndexMap<Vec<TypeId>, Vec<usize>>,
		_out: &mut Vec<usize>,
	) {
		todo!();
	}
	#[inline]
	fn push_matching_exclude_query_group(
		_groups_to_maps: &IndexMap<Vec<TypeId>, Vec<usize>>,
		_out: &mut Vec<usize>,
	) {
		// Do nothing as this is not an exclude
	}
	type Storage = Rc<RefCell<DensePagedDataActual<Self::RawType>>>;
	#[inline]
	fn get_storage(
		_entities: &Rc<RefCell<Vec<Vec<EntityType>>>>,
		maps: &MapIndexMap,
		map_id: usize,
	) -> Self::Storage {
		maps.get_index(map_id)
			.unwrap()
			.1
			.get_strong::<Self::RawType>()
	}

	type StorageSlice = Option<
		OwningRefMut<
			OwningHandle<
				Rc<RefCell<DensePagedDataActual<Self::RawType>>>,
				RefMut<'static, DensePagedDataActual<Self::RawType>>,
			>,
			[Self::RawType],
		>,
	>;
	#[inline]
	fn get_storage_slice_at(storage: &Self::Storage, group: usize) -> Option<Self::StorageSlice> {
		let owned = OwningHandle::new_mut(storage.clone());
		OwningRefMut::new(owned)
			.try_map_mut(|s| {
				let slice = s.data[group].as_mut_slice();
				if slice.len() > 0 {
					Ok(slice)
				} else {
					Err(())
				}
			})
			.map_or(Some(None), |slice| Some(Some(slice)))
	}

	// type StorageValue = Option<
	// 	OwningRefMut<
	// 		OwningHandle<
	// 			Rc<RefCell<DensePagedDataActual<Self::RawType>>>,
	// 			RefMut<'static, DensePagedDataActual<Self::RawType>>,
	// 		>,
	// 		Self::RawType,
	// 	>,
	// >;
	type StorageValue = Option<&'a mut T>;
	#[inline]
	fn get_storage_value_at(
		locked_storage: &'a mut Self::StorageLocked,
		group: usize,
		index: usize,
	) -> Option<Self::StorageValue> {
		if let Some(group) = locked_storage.data.get_mut(group) {
			Some(group.get_mut(index))
		} else {
			Some(None)
		}
		// OwningRefMut::new(locked_storage)
		// 	.try_map_mut(|s| {
		// 		let slice = &mut s.data[group];
		// 		if slice.len() > index {
		// 			Ok(&mut slice[index])
		// 		} else {
		// 			Err(())
		// 		}
		// 	})
		// 	.map_or(Some(None), |slice| Some(Some(slice)))
	}

	type StorageLocked = OwningHandle<
		Rc<RefCell<DensePagedDataActual<Self::RawType>>>,
		RefMut<'static, DensePagedDataActual<Self::RawType>>,
	>;
	#[inline(always)]
	fn get_locked_storage(storage: &Self::Storage) -> Self::StorageLocked {
		OwningHandle::new_mut(storage.clone())
	}

	type StorageMovedValue = T;
	#[inline(always)]
	fn into_keyset_mapidx_vec(locked_storage: &Self::StorageLocked, out: &mut BitVec<Lsb0, usize>) {
		out.set(locked_storage.index, true);
	}
	#[inline(always)]
	fn resize_locked_storage_groups(locked_storage: &mut Self::StorageLocked, new_size: usize) {
		locked_storage.data.resize_with(new_size, Vec::new);
	}
	#[inline(always)]
	fn insert(
		this: Self::StorageMovedValue,
		locked_storages: &mut Self::StorageLocked,
		group: usize,
	) {
		locked_storages.data[group].push(this);
	}

	fn extend_vec(
		this: Vec<Self::StorageMovedValue>,
		locked_storage: &mut Self::StorageLocked,
		group: usize,
		count: usize,
	) {
		let data = &mut locked_storage.data[group];
		let len = data.len();
		data.extend(this);
		assert_eq!(data.len() - len, count);
	}
}

pub trait ComponentTuple<EntityType: Entity>: Sized {
	type LenTN: generic_array::typenum::Unsigned
		+ generic_array::ArrayLength<TypeId>
		+ generic_array::ArrayLength<(usize, TypeId)>;
	fn get_tids() -> generic_array::GenericArray<TypeId, Self::LenTN>;
	fn get_sorted_indexed_tids() -> generic_array::GenericArray<(usize, TypeId), Self::LenTN>;
	#[inline]
	fn into_type_idx_vec(maps: &mut MapIndexMap) -> Vec<usize> {
		let mut idxs = Vec::with_capacity(Self::LenTN::USIZE);
		Self::populate_type_idx_vec(&mut idxs, maps);
		idxs
	}
	fn populate_type_idx_vec(idxs: &mut Vec<usize>, maps: &mut MapIndexMap);
	type StoragesMut;
	fn get_storages_mut(maps: &MapIndexMap, map_idxs: &[usize]) -> Self::StoragesMut;
	type StorageGroupsMut;
	fn get_storages_group_mut(
		maps: &MapIndexMap,
		map_idxs: &[usize],
		group: usize,
	) -> Self::StorageGroupsMut;
	fn insert(self, maps: &mut MapIndexMap, map_idxs: &[usize], group: usize);
	fn insert_in_group(self, groups: &mut Self::StorageGroupsMut);
}

impl<EntityType: Entity> ComponentTuple<EntityType> for () {
	type LenTN = generic_array::typenum::U0;
	#[inline]
	fn get_tids() -> GenericArray<TypeId, Self::LenTN> {
		generic_array::GenericArray::clone_from_slice(&[])
	}
	#[inline]
	fn get_sorted_indexed_tids() -> GenericArray<(usize, TypeId), Self::LenTN> {
		generic_array::GenericArray::clone_from_slice(&[])
	}
	#[inline]
	fn populate_type_idx_vec(_idxs: &mut Vec<usize>, _maps: &mut MapIndexMap) {}
	type StoragesMut = ();
	#[inline]
	fn get_storages_mut(_maps: &MapIndexMap, _map_idxs: &[usize]) -> Self::StoragesMut {}
	type StorageGroupsMut = ();
	#[inline]
	fn get_storages_group_mut(
		_maps: &MapIndexMap,
		_map_idxs: &[usize],
		_group: usize,
	) -> Self::StorageGroupsMut {
	}
	#[inline]
	fn insert(self, _maps: &mut MapIndexMap, _map_idxs: &[usize], _group: usize) {}
	#[inline]
	fn insert_in_group(self, _groups: &mut Self::StorageGroupsMut) {}
}

// impl<EntityType: Entity, A: 'static> ComponentTuple<EntityType> for (A,) {
// 	type LenTN = generic_array::typenum::U1;
// 	#[inline]
// 	fn get_tids() -> GenericArray<TypeId, Self::LenTN> {
// 		generic_array::GenericArray::clone_from_slice(&[std::any::TypeId::of::<A>()])
// 	}
// 	#[inline]
// 	fn populate_type_idx_vec(idxs: &mut Vec<usize>, maps: &mut MapIndexMap) {
// 		{
// 			let entry = maps.entry(std::any::TypeId::of::<A>());
// 			idxs.push(entry.index());
// 			entry.or_insert_with(|| Box::new(DensePagedDataInstance::<A>::new()));
// 		}
// 	}
// 	type StoragesMut = (Rc<RefCell<DensePagedDataActual<A>>>,);
// 	#[inline]
// 	fn get_storages_mut(maps: &MapIndexMap, map_idxs: &[usize]) -> Self::StoragesMut {
// 		let a = {
// 			let (_type_id, a) = maps
// 				.get_index(map_idxs[0])
// 				.expect("Map is in invalid state!  Shouldn't happen!");
// 			a.get_strong::<A>()
// 		};
// 		(a,)
// 	}
// 	type StorageGroupsMut = (
// 		OwningRefMut<
// 			OwningHandle<
// 				Rc<RefCell<DensePagedDataActual<A>>>,
// 				RefMut<'static, DensePagedDataActual<A>>,
// 			>,
// 			Vec<A>,
// 		>,
// 	);
// 	#[inline]
// 	fn get_storages_group_mut(
// 		maps: &MapIndexMap,
// 		map_idxs: &[usize],
// 		group: usize,
// 	) -> Self::StorageGroupsMut {
// 		let a = {
// 			let (_type_id, map) = maps
// 				.get_index(map_idxs[0])
// 				.expect("Map is in invalid state!  Shouldn't happen!");
// 			OwningRefMut::new(OwningHandle::new_mut(map.get_strong::<A>()))
// 				.map_mut(|s| &mut s.data[group])
// 		};
// 		(a,)
// 	}
// 	#[inline]
// 	fn insert(self, maps: &mut MapIndexMap, map_idxs: &[usize], group: usize) {
// 		{
// 			let (_type_id, map) = maps
// 				.get_index_mut(map_idxs[0])
// 				.expect("Map is in invalid state!  Shouldn't happen!");
// 			map.get_refmut::<A>().push(group, self.0);
// 		}
// 	}
// 	#[inline]
// 	fn insert_in_group(self, mut groups: &mut Self::StorageGroupsMut) {
// 		let (ca,) = self;
// 		let (a,) = &mut groups;
// 		a.push(ca);
// 	}
// }

macro_rules! impl_ComponentTuple {
	($GALEN:path, $(($ID:ident $IDX:tt)),+) => {
		impl<EntityType: Entity, $($ID: 'static),+> ComponentTuple<EntityType> for ($($ID,)+) {
			type LenTN = $GALEN;
			#[inline]
			fn get_tids() -> GenericArray<TypeId, Self::LenTN> {
				generic_array::GenericArray::clone_from_slice(&[$(std::any::TypeId::of::<$ID>()),+])
			}
			#[inline]
			fn get_sorted_indexed_tids() -> GenericArray<(usize, TypeId), Self::LenTN> {
				let mut array = generic_array::GenericArray::clone_from_slice(&[$(($IDX, std::any::TypeId::of::<$ID>())),+]);
				array.sort_by(|l, r| l.1.cmp(&r.1));
				array
			}
			#[inline]
			fn populate_type_idx_vec(idxs: &mut Vec<usize>, maps: &mut MapIndexMap) {
				$({
					let entry = maps.entry(std::any::TypeId::of::<$ID>());
					let index = entry.index();
					idxs.push(index);
					entry.or_insert_with(|| Box::new(DensePagedDataInstance::<$ID>::with_groups(index, 0)));
				})+
			}
			type StoragesMut = ($(Rc<RefCell<DensePagedDataActual<$ID>>>,)+);
			#[inline]
			fn get_storages_mut(maps: &MapIndexMap, map_idxs: &[usize]) -> Self::StoragesMut {
				($({
					let (_type_id, map) = maps
							.get_index(map_idxs[$IDX])
							.expect("Map is in invalid state!  Shouldn't happen!");
					map.get_strong::<$ID>()
				},)+)
			}
			type StorageGroupsMut = (
				$(OwningRefMut<
					OwningHandle<
						Rc<RefCell<DensePagedDataActual<$ID>>>,
						RefMut<'static, DensePagedDataActual<$ID>>,
					>,
					Vec<$ID>,
				>,)+
			);
			#[inline]
			fn get_storages_group_mut(
				maps: &MapIndexMap,
				map_idxs: &[usize],
				group: usize,
			) -> Self::StorageGroupsMut {
				($({
					let (_type_id, map) = maps
						.get_index(map_idxs[$IDX])
						.expect("Map is in invalid state!  Shouldn't happen!");
					OwningRefMut::new(OwningHandle::new_mut(map.get_strong::<$ID>())).map_mut(|s| &mut s.data[group])
				},)+)
			}
			#[inline]
			fn insert(self, maps: &mut MapIndexMap, map_idxs: &[usize], group: usize) {
				$({
					let (_type_id, map) = maps
						.get_index_mut(map_idxs[$IDX])
						.expect("Map is in invalid state!  Shouldn't happen!");
					map.get_refmut::<$ID>().push(group, self.$IDX);
				})+
			}
			#[inline]
			fn insert_in_group(self, groups: &mut Self::StorageGroupsMut) {
				$(groups.$IDX.push(self.$IDX);)+
			}
		}
	};
}

impl_ComponentTuple!(generic_array::typenum::U1, (A 0));
impl_ComponentTuple!(generic_array::typenum::U2, (A 0), (B 1));
impl_ComponentTuple!(generic_array::typenum::U3, (A 0), (B 1), (C 2));
impl_ComponentTuple!(generic_array::typenum::U4, (A 0), (B 1), (C 2), (D 3));
impl_ComponentTuple!(generic_array::typenum::U4, (A 0), (B 1), (C 2), (D 3), (E 4));
impl_ComponentTuple!(generic_array::typenum::U4, (A 0), (B 1), (C 2), (D 3), (E 4), (F 5));
impl_ComponentTuple!(generic_array::typenum::U4, (A 0), (B 1), (C 2), (D 3), (E 4), (F 5), (G 6));

pub mod indices {
	pub struct Here {
		_priv: (),
	}

	pub struct There<T> {
		_marker: std::marker::PhantomData<T>,
	}
}

// trait ComponentTupleQueryPtrSelector<
// 	'a,
// 	EntityType: Entity,
// 	T: 'static + ComponentQuery<'a, EntityType>,
// 	Index,
// >: ComponentTupleQuery<'static, EntityType>
// {
// 	unsafe fn get_locked_storage_ptr(storages: &mut Self::StoragesLocked) -> &mut T::StorageLocked;
// }
//
// impl<
// 		'a,
// 		EntityType: 'static + Entity,
// 		HEAD: 'static + ComponentQuery<'a, EntityType>,
// 		TAIL: ComponentTupleQuery<'static, EntityType>,
// 	> ComponentTupleQueryPtrSelector<'a, EntityType, HEAD, indices::Here> for (HEAD, TAIL)
// where
// 	<HEAD as ComponentQuery<'a, EntityType>>::LenExcludeTN:
// 		std::ops::Add<<TAIL as ComponentTupleQuery<'static, EntityType>>::LenExcludeTN>,
// 	<<HEAD as ComponentQuery<'a, EntityType>>::LenExcludeTN as std::ops::Add<
// 		<TAIL as ComponentTupleQuery<'static, EntityType>>::LenExcludeTN,
// 	>>::Output: generic_array::ArrayLength<std::any::TypeId>,
// 	<HEAD as ComponentQuery<'a, EntityType>>::LenIncludeTN:
// 		std::ops::Add<<TAIL as ComponentTupleQuery<'static, EntityType>>::LenIncludeTN>,
// 	<<HEAD as ComponentQuery<'a, EntityType>>::LenIncludeTN as std::ops::Add<
// 		<TAIL as ComponentTupleQuery<'static, EntityType>>::LenIncludeTN,
// 	>>::Output: generic_array::ArrayLength<std::any::TypeId>,
// 	<HEAD as ComponentQuery<'a, EntityType>>::LenKeysetTN:
// 		std::ops::Add<<TAIL as ComponentTupleQuery<'static, EntityType>>::LenKeysetTN>,
// 	<<HEAD as ComponentQuery<'a, EntityType>>::LenKeysetTN as std::ops::Add<
// 		<TAIL as ComponentTupleQuery<'static, EntityType>>::LenKeysetTN,
// 	>>::Output: generic_array::ArrayLength<std::any::TypeId>,
// {
// 	unsafe fn get_locked_storage_ptr(
// 		storages: &mut Self::StoragesLocked,
// 	) -> &mut HEAD::StorageLocked {
// 		todo!()
// 	}
// }
//
// impl<
// 		'a,
// 		EntityType: Entity,
// 		HEAD: 'static + ComponentQuery<'a, EntityType>,
// 		FromTail: 'static + ComponentQuery<'a, EntityType>,
// 		TailIndex,
// 		TAIL,
// 	> ComponentTupleQueryPtrSelector<'a, EntityType, FromTail, indices::There<TailIndex>>
// 	for (HEAD, TAIL)
// where
// 	TAIL: ComponentTupleQueryPtrSelector<'a, EntityType, FromTail, TailIndex>,
// 	<HEAD as ComponentQuery<'a, EntityType>>::LenExcludeTN:
// 		std::ops::Add<<TAIL as ComponentTupleQuery<'a, EntityType>>::LenExcludeTN>,
// 	<<HEAD as ComponentQuery<'a, EntityType>>::LenExcludeTN as std::ops::Add<
// 		<TAIL as ComponentTupleQuery<'a, EntityType>>::LenExcludeTN,
// 	>>::Output: generic_array::ArrayLength<std::any::TypeId>,
// 	<HEAD as ComponentQuery<'a, EntityType>>::LenIncludeTN:
// 		std::ops::Add<<TAIL as ComponentTupleQuery<'a, EntityType>>::LenIncludeTN>,
// 	<<HEAD as ComponentQuery<'a, EntityType>>::LenIncludeTN as std::ops::Add<
// 		<TAIL as ComponentTupleQuery<'a, EntityType>>::LenIncludeTN,
// 	>>::Output: generic_array::ArrayLength<std::any::TypeId>,
// 	<HEAD as ComponentQuery<'a, EntityType>>::LenKeysetTN:
// 		std::ops::Add<<TAIL as ComponentTupleQuery<'a, EntityType>>::LenKeysetTN>,
// 	<<HEAD as ComponentQuery<'a, EntityType>>::LenKeysetTN as std::ops::Add<
// 		<TAIL as ComponentTupleQuery<'a, EntityType>>::LenKeysetTN,
// 	>>::Output: generic_array::ArrayLength<std::any::TypeId>,
// {
// 	unsafe fn get_locked_storage_ptr(
// 		storages: &mut Self::StoragesLocked,
// 	) -> &mut FromTail::StorageLocked {
// 		TAIL::get_locked_storage_ptr(&mut storages.1)
// 	}
// }

pub trait ComponentTupleQuery<'a, EntityType: Entity> {
	type LenIncludeTN: generic_array::typenum::Unsigned + generic_array::ArrayLength<TypeId>;
	type LenExcludeTN: generic_array::typenum::Unsigned + generic_array::ArrayLength<TypeId>;
	type LenKeysetTN: generic_array::typenum::Unsigned + generic_array::ArrayLength<TypeId>;
	fn get_include_tids() -> generic_array::GenericArray<TypeId, Self::LenIncludeTN>;
	fn get_exclude_tids() -> generic_array::GenericArray<TypeId, Self::LenExcludeTN>;
	fn get_keyset_tids() -> generic_array::GenericArray<TypeId, Self::LenKeysetTN>;
	#[inline]
	fn get_include_matching_query_groups(
		groups_to_maps: &GroupTypeSetToMapSet,
		include_tids: &GenericArray<TypeId, Self::LenIncludeTN>,
	) -> Vec<usize> {
		let mut out = Vec::with_capacity(Self::LenIncludeTN::USIZE);
		if include_tids.is_empty() {
			return out;
		}
		for (idx, type_ids) in groups_to_maps.keys().enumerate() {
			//if type_ids.iter().all(|t| include_tids.contains(t)) {
			if include_tids.iter().all(|t| type_ids.contains(t)) {
				out.push(idx);
			}
		}
		out
	}
	#[inline]
	fn get_exclude_matching_query_groups(
		groups_to_maps: &IndexMap<Vec<TypeId>, Vec<usize>>,
		exclude_tids: &GenericArray<TypeId, Self::LenExcludeTN>,
	) -> Vec<usize> {
		let mut out = Vec::with_capacity(Self::LenExcludeTN::USIZE);
		if exclude_tids.is_empty() {
			return out;
		}
		for (idx, type_ids) in groups_to_maps.keys().enumerate() {
			if type_ids.iter().any(|t| exclude_tids.contains(t)) {
				out.push(idx);
			}
		}
		out
	}
	fn get_map_idxs(maps: &mut MapIndexMap) -> Vec<usize>;
	type Storages: Clone;
	fn get_storages(
		entities: &Rc<RefCell<Vec<Vec<EntityType>>>>,
		maps: &MapIndexMap,
		map_ids: &[usize],
	) -> Self::Storages;
	type StorageSlices;
	fn get_storage_slices_at(
		storages: &Self::Storages,
		group: usize,
	) -> Option<Self::StorageSlices>;
	// type StorageSlicesRef: 'a;
	// fn get_storage_slices_ref_at(
	// 	storages: &'a mut Self::Storages,
	// 	group: usize,
	// ) -> Self::StorageSlices;
	type StorageValues: 'a;
	fn get_storage_values_at(
		locked_storages: &'a mut Self::StoragesLockedRef,
		group: usize,
		index: usize,
	) -> Option<Self::StorageValues>;
	type StoragesLocked: Sized;
	type StoragesLockedRef: Sized;
	fn get_locked_storages(storages: &Self::Storages) -> Self::StoragesLocked;
	// Uuuugh lack of GATs...
	fn get_locked_storage_ptr<'s, TT: 'static + ComponentQuery<'s, EntityType>>(
		storages: &mut Self::StoragesLocked,
	) -> &'s mut TT::StorageLocked;
	fn cast_locked_storages<CT: ComponentTupleQuery<'static, EntityType>>(
		storages: &mut CT::StoragesLocked,
	) -> Self::StoragesLockedRef;
	type StorageMovedValues;
	fn into_keyset_mapidx_vec(locked_storages: &Self::StoragesLocked, out: &mut BitVec);
	fn resize_locked_storages_groups(locked_storages: &mut Self::StoragesLocked, new_size: usize);
	fn insert(
		this: Self::StorageMovedValues,
		locked_storages: &mut Self::StoragesLocked,
		group: usize,
	);
	type StorageMovedVecs;
	fn extend_vecs(
		this: Self::StorageMovedVecs,
		locked_storages: &mut Self::StoragesLocked,
		group: usize,
		count: usize,
	);
}

impl<'a, EntityType: Entity> ComponentTupleQuery<'a, EntityType> for () {
	type LenIncludeTN = generic_array::typenum::U0;
	type LenExcludeTN = generic_array::typenum::U0;
	type LenKeysetTN = generic_array::typenum::U0;
	#[inline]
	fn get_include_tids() -> GenericArray<TypeId, Self::LenIncludeTN> {
		generic_array::GenericArray::clone_from_slice(&[])
	}
	#[inline]
	fn get_exclude_tids() -> GenericArray<TypeId, Self::LenExcludeTN> {
		generic_array::GenericArray::clone_from_slice(&[])
	}
	#[inline]
	fn get_keyset_tids() -> GenericArray<TypeId, Self::LenKeysetTN> {
		generic_array::GenericArray::clone_from_slice(&[])
	}

	#[inline]
	fn get_map_idxs(_maps: &mut MapIndexMap) -> Vec<usize> {
		vec![]
	}

	type Storages = ();
	#[inline]
	fn get_storages(
		_entities: &Rc<RefCell<Vec<Vec<EntityType>>>>,
		_maps: &MapIndexMap,
		_map_ids: &[usize],
	) -> Self::Storages {
	}

	type StorageSlices = ();
	#[inline]
	fn get_storage_slices_at(
		_storages: &Self::Storages,
		_group: usize,
	) -> Option<Self::StorageSlices> {
		Some(())
	}

	type StorageValues = ();
	#[inline]
	fn get_storage_values_at(
		_locked_storages: &'a mut Self::StoragesLockedRef,
		_group: usize,
		_index: usize,
	) -> Option<Self::StorageValues> {
		Some(())
	}

	type StoragesLocked = ();
	type StoragesLockedRef = ();
	#[inline]
	fn get_locked_storages(_storages: &Self::Storages) -> Self::StoragesLocked {}
	#[inline]
	fn get_locked_storage_ptr<'s, TT: 'static + ComponentQuery<'s, EntityType>>(
		_storages: &mut Self::StoragesLocked,
	) -> &'s mut TT::StorageLocked {
		panic!(
			"requested a component type that does not exist in this storage: {}",
			std::any::type_name::<TT>()
		)
	}
	#[inline]
	fn cast_locked_storages<CT: ComponentTupleQuery<'static, EntityType>>(
		_storages: &mut CT::StoragesLocked,
	) -> Self::StoragesLockedRef {
	}

	type StorageMovedValues = ();
	#[inline]
	fn into_keyset_mapidx_vec(_locked_storages: &Self::StoragesLocked, _out: &mut BitVec) {}
	#[inline]
	fn resize_locked_storages_groups(
		_locked_storages: &mut Self::StoragesLocked,
		_new_size: usize,
	) {
	}
	#[inline]
	fn insert(
		_this: Self::StorageMovedValues,
		_locked_storages: &mut Self::StoragesLocked,
		_group: usize,
	) {
	}

	type StorageMovedVecs = ();
	#[inline]
	fn extend_vecs(
		_this: Self::StorageMovedVecs,
		_locked_storages: &mut Self::StoragesLocked,
		_group: usize,
		_count: usize,
	) {
	}
}

// impl<EntityType: Entity, A: 'static + ComponentQuery<EntityType>> ComponentTupleQuery<EntityType>
// 	for (A,)
// {
// 	type LenIncludeTN = A::LenIncludeTN;
// 	type LenExcludeTN = A::LenExcludeTN;
// 	#[inline]
// 	fn get_include_tids() -> GenericArray<TypeId, Self::LenIncludeTN> {
// 		generic_array::GenericArray::from_exact_iter(
// 			[A::get_include_tid()].iter().copied().filter_map(|tid| tid),
// 		)
// 		.unwrap()
// 	}
// 	#[inline]
// 	fn get_exclude_tids() -> GenericArray<TypeId, Self::LenExcludeTN> {
// 		generic_array::GenericArray::from_exact_iter(
// 			[A::get_exclude_tid()].iter().copied().filter_map(|tid| tid),
// 		)
// 		.unwrap()
// 	}
// 	#[inline]
// 	fn get_map_idxs(maps: &mut MapIndexMap) -> Vec<usize> {
// 		let a: usize = {
// 			let entry = maps.entry(std::any::TypeId::of::<A::RawType>());
// 			let index = entry.index();
// 			entry.or_insert_with(|| Box::new(DensePagedDataInstance::<A::RawType>::new()));
// 			index
// 		};
// 		vec![a]
// 	}
//
// 	type Storages = (A::Storage,);
//
// 	#[inline]
// 	fn get_storages(
// 		entities: &Rc<RefCell<Vec<Vec<EntityType>>>>,
// 		maps: &MapIndexMap,
// 		map_ids: &[usize],
// 	) -> Self::Storages {
// 		let a = A::get_storage(entities, maps, map_ids[0]);
// 		(a,)
// 	}
//
// 	type StorageSlices = (A::StorageSlice,);
// 	#[inline]
// 	fn get_storage_slices_at(
// 		storages: &Self::Storages,
// 		group: usize,
// 	) -> Option<Self::StorageSlices> {
// 		let (a,) = storages;
// 		let a = A::get_storage_slice_at(&a, group)?;
// 		Some((a,))
// 	}
//
// 	type StorageValues = (A::StorageValue,);
// 	#[inline]
// 	fn get_storage_values_at(
// 		storages: &Self::Storages,
// 		group: usize,
// 		index: usize,
// 	) -> Option<Self::StorageValues> {
// 		let (a,) = storages;
// 		let a = A::get_storage_value_at(a, group, index)?;
// 		Some((a,))
// 	}
//
// 	type StoragesLocked = (A::StorageLocked,);
// 	#[inline]
// 	fn get_locked_storages(storages: &Self::Storages) -> Self::StoragesLocked {
// 		let a = A::get_locked_storage(&storages.0);
// 		(a,)
// 	}
// }

impl<
		'a,
		EntityType: Entity,
		HEAD: 'static + ComponentQuery<'a, EntityType>,
		TAIL: 'static + ComponentTupleQuery<'a, EntityType>,
	> ComponentTupleQuery<'a, EntityType> for (HEAD, TAIL)
where
	<HEAD as ComponentQuery<'a, EntityType>>::LenExcludeTN:
		std::ops::Add<<TAIL as ComponentTupleQuery<'a, EntityType>>::LenExcludeTN>,
	<<HEAD as ComponentQuery<'a, EntityType>>::LenExcludeTN as std::ops::Add<
		<TAIL as ComponentTupleQuery<'a, EntityType>>::LenExcludeTN,
	>>::Output: generic_array::ArrayLength<std::any::TypeId>,
	<HEAD as ComponentQuery<'a, EntityType>>::LenIncludeTN:
		std::ops::Add<<TAIL as ComponentTupleQuery<'a, EntityType>>::LenIncludeTN>,
	<<HEAD as ComponentQuery<'a, EntityType>>::LenIncludeTN as std::ops::Add<
		<TAIL as ComponentTupleQuery<'a, EntityType>>::LenIncludeTN,
	>>::Output: generic_array::ArrayLength<std::any::TypeId>,
	<HEAD as ComponentQuery<'a, EntityType>>::LenKeysetTN:
		std::ops::Add<<TAIL as ComponentTupleQuery<'a, EntityType>>::LenKeysetTN>,
	<<HEAD as ComponentQuery<'a, EntityType>>::LenKeysetTN as std::ops::Add<
		<TAIL as ComponentTupleQuery<'a, EntityType>>::LenKeysetTN,
	>>::Output: generic_array::ArrayLength<std::any::TypeId>,
{
	type LenIncludeTN = generic_array::typenum::Sum<HEAD::LenIncludeTN, TAIL::LenIncludeTN>;
	type LenExcludeTN = generic_array::typenum::Sum<HEAD::LenExcludeTN, TAIL::LenExcludeTN>;
	type LenKeysetTN = generic_array::typenum::Sum<HEAD::LenKeysetTN, TAIL::LenKeysetTN>;
	#[inline]
	fn get_include_tids() -> GenericArray<TypeId, Self::LenIncludeTN> {
		generic_array::GenericArray::from_exact_iter(
			std::iter::once(HEAD::get_include_tid())
				.filter_map(|tid| tid)
				.chain(TAIL::get_include_tids()),
		)
		.unwrap()
	}
	#[inline]
	fn get_exclude_tids() -> GenericArray<TypeId, Self::LenExcludeTN> {
		generic_array::GenericArray::from_exact_iter(
			std::iter::once(HEAD::get_exclude_tid())
				.filter_map(|tid| tid)
				.chain(TAIL::get_exclude_tids()),
		)
		.unwrap()
	}
	#[inline]
	fn get_keyset_tids() -> GenericArray<TypeId, Self::LenKeysetTN> {
		generic_array::GenericArray::from_exact_iter(
			std::iter::once(HEAD::get_keyset_tid())
				.filter_map(|tid| tid)
				.chain(TAIL::get_keyset_tids()),
		)
		.unwrap()
	}

	#[inline]
	fn get_map_idxs(maps: &mut MapIndexMap) -> Vec<usize> {
		let mut vec = Vec::with_capacity(Self::LenIncludeTN::USIZE);

		let entry = maps.entry(std::any::TypeId::of::<HEAD::RawType>());
		let index = entry.index();
		vec.push(index);
		entry.or_insert_with(|| {
			Box::new(DensePagedDataInstance::<HEAD::RawType>::with_groups(
				index, 0,
			))
		});

		vec.extend(TAIL::get_map_idxs(maps));
		vec
	}

	type Storages = (HEAD::Storage, TAIL::Storages);
	#[inline]
	fn get_storages(
		entities: &Rc<RefCell<Vec<Vec<EntityType>>>>,
		maps: &MapIndexMap,
		map_ids: &[usize],
	) -> Self::Storages {
		(
			HEAD::get_storage(entities, maps, map_ids[0]),
			TAIL::get_storages(entities, maps, &map_ids[1..]),
		)
	}

	type StorageSlices = (HEAD::StorageSlice, TAIL::StorageSlices);
	#[inline]
	fn get_storage_slices_at(
		storages: &Self::Storages,
		group: usize,
	) -> Option<Self::StorageSlices> {
		Some((
			HEAD::get_storage_slice_at(&storages.0, group)?,
			TAIL::get_storage_slices_at(&storages.1, group)?,
		))
	}

	type StorageValues = (HEAD::StorageValue, TAIL::StorageValues);
	#[inline]
	fn get_storage_values_at(
		locked_storages: &'a mut Self::StoragesLockedRef,
		group: usize,
		index: usize,
	) -> Option<Self::StorageValues> {
		Some((
			HEAD::get_storage_value_at(&mut locked_storages.0, group, index)?,
			TAIL::get_storage_values_at(&mut locked_storages.1, group, index)?,
		))
	}

	type StoragesLocked = (HEAD::StorageLocked, TAIL::StoragesLocked);
	type StoragesLockedRef = (&'a mut HEAD::StorageLocked, TAIL::StoragesLockedRef);
	#[inline]
	fn get_locked_storages(storages: &Self::Storages) -> Self::StoragesLocked {
		(
			HEAD::get_locked_storage(&storages.0),
			TAIL::get_locked_storages(&storages.1),
		)
	}
	#[inline]
	fn get_locked_storage_ptr<'s, TT: 'static + ComponentQuery<'s, EntityType>>(
		storages: &mut Self::StoragesLocked,
	) -> &'s mut TT::StorageLocked {
		// Hopefully optimizes out since it's very constant...
		if TT::get_self_typeid() == HEAD::get_self_typeid() {
			// TODO:  Lack of GATs sucks...  This unsafe can be removed once they exist.
			// This unsafe 'should' be safeish considering the type is the same and we are just
			// constraining, not widening, the lifetime.
			unsafe { &mut *(&mut storages.0 as *mut HEAD::StorageLocked as *mut TT::StorageLocked) }
		} else {
			TAIL::get_locked_storage_ptr::<TT>(&mut storages.1)
		}
	}
	#[inline]
	fn cast_locked_storages<CT: ComponentTupleQuery<'static, EntityType>>(
		storages: &mut CT::StoragesLocked,
	) -> Self::StoragesLockedRef {
		//let thing: &mut HEAD::StorageLocked = CT::get_locked_storage_ptr(storages);
		(
			CT::get_locked_storage_ptr::<HEAD>(storages),
			// ComponentTupleQueryPtrSelector::<'a, EntityType, CT, HEAD, Index>::get_locked_storage_ptr(
			// 	storages,
			// ),
			//thing,
			TAIL::cast_locked_storages::<CT>(storages),
		)
	}

	type StorageMovedValues = (HEAD::StorageMovedValue, TAIL::StorageMovedValues);
	#[inline]
	fn into_keyset_mapidx_vec(locked_storages: &Self::StoragesLocked, out: &mut BitVec) {
		HEAD::into_keyset_mapidx_vec(&locked_storages.0, out);
		TAIL::into_keyset_mapidx_vec(&locked_storages.1, out)
	}
	#[inline]
	fn resize_locked_storages_groups(locked_storages: &mut Self::StoragesLocked, new_size: usize) {
		HEAD::resize_locked_storage_groups(&mut locked_storages.0, new_size);
		TAIL::resize_locked_storages_groups(&mut locked_storages.1, new_size);
	}
	#[inline]
	fn insert(
		this: Self::StorageMovedValues,
		locked_storages: &mut Self::StoragesLocked,
		group: usize,
	) {
		HEAD::insert(this.0, &mut locked_storages.0, group);
		TAIL::insert(this.1, &mut locked_storages.1, group);
	}

	// TODO:  Ugh, need GAT's then can make an iterator version...
	type StorageMovedVecs = (
		// Iterator<
		// 	Item = HEAD::StorageMovedValue,
		// >,
		Vec<HEAD::StorageMovedValue>,
		TAIL::StorageMovedVecs,
	);

	fn extend_vecs(
		this: Self::StorageMovedVecs,
		locked_storages: &mut Self::StoragesLocked,
		group: usize,
		count: usize,
	) {
		HEAD::extend_vec(this.0, &mut locked_storages.0, group, count);
		TAIL::extend_vecs(this.1, &mut locked_storages.1, group, count);
	}
}

// impl<
// 		EntityType: Entity,
// 		A: 'static + ComponentQuery<EntityType>,
// 		B: 'static + ComponentQuery<EntityType>,
// 	> ComponentTupleQuery<EntityType> for (A, B)
// where
// 	<A as ComponentQuery<EntityType>>::LenIncludeTN:
// 		std::ops::Add<<B as ComponentQuery<EntityType>>::LenIncludeTN>,
// 	<<A as ComponentQuery<EntityType>>::LenIncludeTN as std::ops::Add<
// 		<B as ComponentQuery<EntityType>>::LenIncludeTN,
// 	>>::Output: generic_array::ArrayLength<TypeId>,
// 	<A as ComponentQuery<EntityType>>::LenExcludeTN:
// 		std::ops::Add<<B as ComponentQuery<EntityType>>::LenExcludeTN>,
// 	<<A as ComponentQuery<EntityType>>::LenExcludeTN as std::ops::Add<
// 		<B as ComponentQuery<EntityType>>::LenExcludeTN,
// 	>>::Output: generic_array::ArrayLength<TypeId>,
// {
// 	type LenIncludeTN = generic_array::typenum::Sum<
// 		A::LenIncludeTN,
// 		<(B,) as ComponentTupleQuery<EntityType>>::LenIncludeTN,
// 	>;
// 	type LenExcludeTN = generic_array::typenum::Sum<
// 		A::LenExcludeTN,
// 		<(B,) as ComponentTupleQuery<EntityType>>::LenExcludeTN,
// 	>;
// 	#[inline]
// 	fn get_include_tids() -> GenericArray<TypeId, Self::LenIncludeTN> {
// 		generic_array::GenericArray::from_exact_iter(
// 			[A::get_include_tid(), B::get_include_tid()]
// 				.iter()
// 				.copied()
// 				.filter_map(|tid| tid),
// 		)
// 		.unwrap()
// 	}
// 	#[inline]
// 	fn get_exclude_tids() -> GenericArray<TypeId, Self::LenExcludeTN> {
// 		generic_array::GenericArray::from_exact_iter(
// 			[A::get_exclude_tid(), B::get_exclude_tid()]
// 				.iter()
// 				.copied()
// 				.filter_map(|tid| tid),
// 		)
// 		.unwrap()
// 	}
//
// 	#[inline]
// 	fn get_map_idxs(maps: &mut MapIndexMap) -> Vec<usize> {
// 		let a: usize = {
// 			let entry = maps.entry(std::any::TypeId::of::<A::RawType>());
// 			let index = entry.index();
// 			entry.or_insert_with(|| Box::new(DensePagedDataInstance::<A::RawType>::new()));
// 			index
// 		};
// 		let b: usize = {
// 			let entry = maps.entry(std::any::TypeId::of::<B::RawType>());
// 			let index = entry.index();
// 			entry.or_insert_with(|| Box::new(DensePagedDataInstance::<B::RawType>::new()));
// 			index
// 		};
// 		vec![a, b]
// 	}
//
// 	type Storages = (A::Storage, B::Storage);
// 	#[inline]
// 	fn get_storages(
// 		entities: &Rc<RefCell<Vec<Vec<EntityType>>>>,
// 		maps: &MapIndexMap,
// 		map_ids: &[usize],
// 	) -> Self::Storages {
// 		let a = A::get_storage(entities, maps, map_ids[0]);
// 		let b = B::get_storage(entities, maps, map_ids[1]);
// 		(a, b)
// 	}
//
// 	type StorageSlices = (A::StorageSlice, B::StorageSlice);
// 	#[inline]
// 	fn get_storage_slices_at(
// 		storages: &Self::Storages,
// 		group: usize,
// 	) -> Option<Self::StorageSlices> {
// 		let (a, b) = storages;
// 		let a = A::get_storage_slice_at(a, group)?;
// 		let b = B::get_storage_slice_at(b, group)?;
// 		Some((a, b))
// 	}
//
// 	type StorageValues = (A::StorageValue, B::StorageValue);
// 	#[inline]
// 	fn get_storage_values_at(
// 		storages: &Self::Storages,
// 		group: usize,
// 		index: usize,
// 	) -> Option<Self::StorageValues> {
// 		let (a, b) = storages;
// 		let a = A::get_storage_value_at(a, group, index)?;
// 		let b = B::get_storage_value_at(b, group, index)?;
// 		Some((a, b))
// 	}
// }

// macro_rules! impl_ComponentTupleQuery {
// 	(LENTN, $NAME:ident, $ID:ident) => {
// 		$ID::$NAME
// 	};
// 	(LENTN, $NAME:ident, $ID:ident, $($IDs:ident),+) => {
// 		generic_array::typenum::Sum<$ID::$NAME, impl_ComponentTupleQuery!(LENTN, $NAME, $($IDs),+)>
// 	};
// 	($(($IDs:ident $IDXs:tt)),*, {$($WHERE:tt)+}) => {
// 		impl<EntityType: Entity, $($IDs: 'static + ComponentQuery<EntityType>),+> ComponentTupleQuery<EntityType>
// 		for ($($IDs),*)
// 		where $($WHERE)+ // can't build this directly because can't call a macro in a where-clause
// 		{
// 			type LenIncludeTN = impl_ComponentTupleQuery!(LENTN, LenIncludeTN, $($IDs),+);
// 			type LenExcludeTN = impl_ComponentTupleQuery!(LENTN, LenExcludeTN, $($IDs),+);
// 			#[inline]
// 			fn get_include_tids() -> GenericArray<TypeId, Self::LenIncludeTN> {
// 				generic_array::GenericArray::from_exact_iter(
// 					[$($IDs::get_include_tid()),+].iter().copied().filter_map(|tid| tid),
// 				).unwrap()
// 			}
// 			#[inline]
// 			fn get_exclude_tids() -> GenericArray<TypeId, Self::LenExcludeTN> {
// 				generic_array::GenericArray::from_exact_iter(
// 					[$($IDs::get_exclude_tid()),+].iter().copied().filter_map(|tid| tid),
// 				).unwrap()
// 			}
//
// 			#[inline]
// 			fn get_map_idxs(maps: &mut MapIndexMap) -> Vec<usize> {
// 				vec![$({
// 					let entry = maps.entry(std::any::TypeId::of::<$IDs::RawType>());
// 					let index = entry.index();
// 					entry.or_insert_with(|| Box::new(DensePagedDataInstance::<$IDs::RawType>::new()));
// 					index
// 				}),+]
// 			}
//
// 			type Storages = ($($IDs::Storage,)+);
// 			#[inline]
// 			fn get_storages(
// 				entities: &Rc<RefCell<Vec<Vec<EntityType>>>>,
// 				maps: &MapIndexMap,
// 				map_ids: &[usize],
// 			) -> Self::Storages {
// 				($($IDs::get_storage(entities, maps, map_ids[$IDXs]),)+)
// 			}
//
// 			type StorageSlices = ($($IDs::StorageSlice,)+);
// 			#[inline]
// 			fn get_storage_slices_at(
// 				storages: &Self::Storages,
// 				group: usize,
// 			) -> Option<Self::StorageSlices> {
// 				Some(($(
// 					$IDs::get_storage_slice_at(&storages.$IDXs, group)?
// 				,)+))
// 			}
//
// 			type StorageValues = ($($IDs::StorageValue,)+);
// 			#[inline]
// 			fn get_storage_values_at(
// 				storages: &Self::Storages,
// 				group: usize,
// 				index: usize,
// 			) -> Option<Self::StorageValues> {
// 				Some(($(
// 					$IDs::get_storage_value_at(&storages.$IDXs, group, index)?
// 				,)+))
// 			}
//
// 			type StoragesLocked = ($($IDs::StorageLocked,)+);
// 			#[inline]
// 			fn get_locked_storages(storages: &Self::Storages) -> Self::StoragesLocked {
// 				($(
// 					$IDs::get_locked_storage(&storages.$IDXs),
// 				)+)
// 			}
// 		}
// 	};
// }

// // Useful to generate the where clause to pass into the normal `impl_ComponentTupleQuery` call
// macro_rules! hack_ComponentTupleQuery_hack {
// 	(@ADD, $ENTITYTYPE:ident, $NAME:ident, $ID:ident) => {
// 		<$ID as ComponentQuery<$ENTITYTYPE>>::$NAME
// 	};
// 	(@ADD, $ENTITYTYPE:ident, $NAME:ident, $ID:ident, $($IDs:ident),+) => {
// 		<<$ID as ComponentQuery<$ENTITYTYPE>>::$NAME as std::ops::Add<hack_ComponentTupleQuery_hack!(@ADD $ENTITYTYPE, $NAME, $($IDs),+)>>::Output
// 	};
// 	(@MAIN, $ENTITYTYPE:ident, $NAME:ident, $ID:ident) => {};
// 	(@MAIN, $ENTITYTYPE:ident, $NAME:ident, $ID:ident, $($IDs:ident),+) => {
// 		//hack_ComponentTupleQuery_hack!(FIRST, $NAME, $ID, $($IDs),+)
// 		hack_ComponentTupleQuery_hack!(MAIN, $NAME, $($IDs),+)
// 		<$ID as ComponentQuery<EntityType>>::$NAME: hack_ComponentTupleQuery_hack!(@ADD, $ENTITYTYPE, $NAME, $($IDs),+)
// 	};
// }
//
// const _: usize = hack_ComponentTupleQuery_hack!(@ADD, EntityType, LenIncludeTN, B, C);
// const _I: usize = hack_ComponentTupleQuery_hack!(@MAIN, EntityType, LenIncludeTN, B, C);
// const _X: usize = hack_ComponentTupleQuery_hack!(@MAIN, EntityType, LenExcludeTN, A, B, C);

// This is horrifyingly bad and only exists because of both of two reasons:
// 1. Cannot call a macro within a where-clause
// 2. Do not have const generics
// If either of these were fixed then this where-clause horror could be gone.
// impl_ComponentTupleQuery!((A 0), (B 1), (C 2), {
//    <B as ComponentQuery<EntityType>>::LenExcludeTN:
// 	   std::ops::Add<<C as ComponentQuery<EntityType>>::LenExcludeTN>,
//    <A as ComponentQuery<EntityType>>::LenExcludeTN: std::ops::Add<
// 	   <<B as ComponentQuery<EntityType>>::LenExcludeTN as std::ops::Add<
// 		   <C as ComponentQuery<EntityType>>::LenExcludeTN,
// 	   >>::Output,
//    >,
//    <<A as ComponentQuery<EntityType>>::LenExcludeTN as std::ops::Add<
// 	   <<B as ComponentQuery<EntityType>>::LenExcludeTN as std::ops::Add<
// 		   <C as ComponentQuery<EntityType>>::LenExcludeTN,
// 	   >>::Output,
//    >>::Output: generic_array::ArrayLength<std::any::TypeId>,
//    <B as ComponentQuery<EntityType>>::LenIncludeTN:
// 	   std::ops::Add<<C as ComponentQuery<EntityType>>::LenIncludeTN>,
//    <A as ComponentQuery<EntityType>>::LenIncludeTN: std::ops::Add<
// 	   <<B as ComponentQuery<EntityType>>::LenIncludeTN as std::ops::Add<
// 		   <C as ComponentQuery<EntityType>>::LenIncludeTN,
// 	   >>::Output,
//    >,
//    <<A as ComponentQuery<EntityType>>::LenIncludeTN as std::ops::Add<
// 	   <<B as ComponentQuery<EntityType>>::LenIncludeTN as std::ops::Add<
// 		   <C as ComponentQuery<EntityType>>::LenIncludeTN,
// 	   >>::Output,
//    >>::Output: generic_array::ArrayLength<std::any::TypeId>,
// });

/*
impl<
		EntityType: Entity,
		A: 'static + ComponentQuery<EntityType>,
		B: 'static + ComponentQuery<EntityType>,
		C: 'static + ComponentQuery<EntityType>,
	> ComponentTupleQuery<EntityType> for (A, B, C)
where
	<B as ComponentQuery<EntityType>>::LenExcludeTN:
		std::ops::Add<<C as ComponentQuery<EntityType>>::LenExcludeTN>,
	<A as ComponentQuery<EntityType>>::LenExcludeTN: std::ops::Add<
		<<B as ComponentQuery<EntityType>>::LenExcludeTN as std::ops::Add<
			<C as ComponentQuery<EntityType>>::LenExcludeTN,
		>>::Output,
	>,
	<<A as ComponentQuery<EntityType>>::LenExcludeTN as std::ops::Add<
		<<B as ComponentQuery<EntityType>>::LenExcludeTN as std::ops::Add<
			<C as ComponentQuery<EntityType>>::LenExcludeTN,
		>>::Output,
	>>::Output: generic_array::ArrayLength<std::any::TypeId>,
	<B as ComponentQuery<EntityType>>::LenIncludeTN:
		std::ops::Add<<C as ComponentQuery<EntityType>>::LenIncludeTN>,
	<A as ComponentQuery<EntityType>>::LenIncludeTN: std::ops::Add<
		<<B as ComponentQuery<EntityType>>::LenIncludeTN as std::ops::Add<
			<C as ComponentQuery<EntityType>>::LenIncludeTN,
		>>::Output,
	>,
	<<A as ComponentQuery<EntityType>>::LenIncludeTN as std::ops::Add<
		<<B as ComponentQuery<EntityType>>::LenIncludeTN as std::ops::Add<
			<C as ComponentQuery<EntityType>>::LenIncludeTN,
		>>::Output,
	>>::Output: generic_array::ArrayLength<std::any::TypeId>,
{
	type LenIncludeTN = generic_array::typenum::Sum<
		A::LenIncludeTN,
		generic_array::typenum::Sum<B::LenIncludeTN, C::LenIncludeTN>,
	>;

	type LenExcludeTN = generic_array::typenum::Sum<
		A::LenExcludeTN,
		generic_array::typenum::Sum<B::LenExcludeTN, C::LenExcludeTN>,
	>;

	#[inline]
	fn get_include_tids() -> GenericArray<TypeId, Self::LenIncludeTN> {
		generic_array::GenericArray::from_exact_iter(
			[A::get_include_tid(), B::get_include_tid()]
				.iter()
				.copied()
				.filter_map(|tid| tid),
		)
		.unwrap()
	}
	#[inline]
	fn get_exclude_tids() -> GenericArray<TypeId, Self::LenExcludeTN> {
		generic_array::GenericArray::from_exact_iter(
			[A::get_exclude_tid(), B::get_exclude_tid()]
				.iter()
				.copied()
				.filter_map(|tid| tid),
		)
		.unwrap()
	}

	#[inline]
	fn get_map_idxs(maps: &mut MapIndexMap) -> Vec<usize> {
		todo!()
	}

	type Storages = (A::Storage, B::Storage);
	#[inline]
	fn get_storages(
		entities: &Rc<RefCell<Vec<Vec<EntityType>>>>,
		maps: &MapIndexMap,
		map_ids: &[usize],
	) -> Self::Storages {
		let a = A::get_storage(entities, maps, map_ids[0]);
		let b = B::get_storage(entities, maps, map_ids[1]);
		(a, b)
	}

	type StorageSlices = (A::StorageSlice, B::StorageSlice);
	#[inline]
	fn get_storage_slices_at(
		storages: &Self::Storages,
		group: usize,
	) -> Option<Self::StorageSlices> {
		let (a, b) = storages;
		let a = A::get_storage_slice_at(a, group)?;
		let b = B::get_storage_slice_at(b, group)?;
		Some((a, b))
	}

	type StorageValues = (A::StorageValue, B::StorageValue);
	#[inline]
	fn get_storage_values_at(
		storages: &Self::Storages,
		group: usize,
		index: usize,
	) -> Option<Self::StorageValues> {
		let (a, b) = storages;
		let a = A::get_storage_value_at(a, group, index)?;
		let b = B::get_storage_value_at(b, group, index)?;
		Some((a, b))
	}
}
*/

/*
pub struct ComponentSetIterExactTypes<C: ComponentSet>(usize, PhantomData<C>);

impl<C: ComponentSet> Iterator for ComponentSetIterExactTypes<C> {
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

impl<C: ComponentSet> ExactSizeIterator for ComponentSetIterExactTypes<C> {
	fn len(&self) -> usize {
		C::LEN
	}
}

pub struct ComponentSliceSetIterExactTypes<C: ComponentSliceSet>(usize, PhantomData<C>);

impl<C: ComponentSliceSet> Iterator for ComponentSliceSetIterExactTypes<C> {
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

impl<C: ComponentSliceSet> ExactSizeIterator for ComponentSliceSetIterExactTypes<C> {
	fn len(&self) -> usize {
		C::LEN
	}
}
*/
pub trait ComponentSet: 'static + TypeList {
	#[inline]
	fn into_type_idx_vec(maps: &mut MapIndexMap) -> Vec<usize> {
		let mut idxs = Vec::with_capacity(Self::LEN);
		Self::populate_type_idx_vec(&mut idxs, maps);
		idxs
	}
	fn populate_type_idx_vec(idxs: &mut Vec<usize>, maps: &mut MapIndexMap);
	#[inline]
	fn insert(self, maps: &mut MapIndexMap, map_idxs: &[usize], group: usize) {
		self.do_insert(maps, map_idxs, group, 0, 0);
	}
	fn do_insert(
		self,
		maps: &mut MapIndexMap,
		map_idxs: &[usize],
		group: usize,
		map_idx_idx: usize,
		data_index: usize,
	);
	fn ensure_exists(maps: &mut MapIndexMap, group_size: usize);
	#[inline]
	fn get_matching_query_groups(group_sets: &IndexMap<Vec<TypeId>, Vec<usize>>) -> Vec<usize> {
		let mut matching = Vec::new();
		for (idx, type_ids) in group_sets.keys().enumerate() {
			if type_ids.iter().copied().all(Self::contains_type_id) {
				matching.push(idx);
			}
		}
		matching
	}
	// type AsMaps: 'static;
	// fn get_maps<'a>(maps: &MapIndexMap, ids: &[usize]) -> Self::AsMaps;
}

trait ComponentMapSet<'a>: TypeList {
	type Blah: 'a;
}

impl ComponentSet for HNil {
	#[inline]
	fn populate_type_idx_vec(_idxs: &mut Vec<usize>, _maps: &mut MapIndexMap) {}
	#[inline]
	fn do_insert(
		self,
		_maps: &mut MapIndexMap,
		_map_idxs: &[usize],
		_group: usize,
		_map_idx_idx: usize,
		_data_index: usize,
	) {
		//ComponentLocations::new(group, data_index)
	}
	#[inline]
	fn ensure_exists(_maps: &mut MapIndexMap, _group_size: usize) {}
	// type AsMaps = HNil;
	// #[inline]
	// fn get_maps<'a>(maps: &MapIndexMap, ids: &[usize]) -> Self::AsMaps {
	// 	HNil
	// }
}

impl<H: 'static, T: ComponentSet> ComponentSet for HCons<H, T>
where
	<T as TypeList>::LenTN: std::ops::Add<generic_array::typenum::B1>,
	<<T as TypeList>::LenTN as std::ops::Add<generic_array::typenum::B1>>::Output:
		generic_array::typenum::Unsigned,
	<<T as TypeList>::LenTN as std::ops::Add<generic_array::typenum::B1>>::Output:
		generic_array::ArrayLength<std::any::TypeId>,
{
	#[inline]
	fn populate_type_idx_vec(idxs: &mut Vec<usize>, maps: &mut MapIndexMap) {
		let entry = maps.entry(std::any::TypeId::of::<H>());
		let index = entry.index();
		idxs.push(index);
		entry.or_insert_with(|| Box::new(DensePagedDataInstance::<H>::with_groups(index, 0)));
		T::populate_type_idx_vec(idxs, maps);
	}

	#[inline]
	fn do_insert(
		self,
		maps: &mut MapIndexMap,
		map_idxs: &[usize],
		group: usize,
		map_idx_idx: usize,
		_data_index: usize,
	) {
		let map_idx = map_idxs[map_idx_idx];
		let (_type_id, map) = maps
			.get_index_mut(map_idx)
			.expect("Map is in invalid state!  Shouldn't happen!");
		let data_index = map.get_refmut::<H>().push(group, self.head);
		//let data_index = map.get_mut().cast_mut::<H>().push(group, self.head);
		// let data_index = map.write()?.ca.push(group, self.head);
		self.tail
			.do_insert(maps, map_idxs, group, map_idx_idx + 1, data_index);
	}

	fn ensure_exists(maps: &mut MapIndexMap, group_size: usize) {
		let tid = std::any::TypeId::of::<H>();
		let index = maps.len();
		maps.entry(tid).or_insert_with(|| {
			Box::new(DensePagedDataInstance::<H>::with_groups(index, group_size))
		});
		T::ensure_exists(maps, group_size);
	}

	// type AsMaps = HCons<Arc<DensePagedDataInstance<H>>, T::AsMaps>;
	//
	// #[inline]
	// fn get_maps(maps: &MapIndexMap, ids: &[usize]) -> Self::AsMaps {
	// 	let (_key, map) = maps.get_index(ids[0]).unwrap();
	// 	HCons {
	// 		head: DensePagedData::cast_arc(map.as_ref()),
	// 		tail: T::get_maps(maps, &ids[1..]),
	// 	}
	// }
}

pub trait ComponentSliceSet: HList + TypeList {
	fn all_same_len(&self, len: usize) -> bool;
	#[inline]
	fn into_type_idx_vec(&self, maps: &mut MapIndexMap) -> Vec<usize> {
		let mut idxs = Vec::with_capacity(Self::LEN);
		self.populate_type_idx_vec(&mut idxs, maps);
		idxs
	}
	fn populate_type_idx_vec(&self, idxs: &mut Vec<usize>, maps: &mut MapIndexMap);
	#[inline]
	fn insert_all(self, maps: &mut MapIndexMap, map_idxs: &[usize], group: usize) -> usize {
		self.do_insert_all(maps, map_idxs, group, 0, 0)
	}
	fn do_insert_all(
		self,
		maps: &mut MapIndexMap,
		map_idxs: &[usize],
		group: usize,
		map_idx_idx: usize,
		start_index: usize,
	) -> usize;
	#[inline]
	fn truncate(maps: &mut MapIndexMap, map_idxs: &[usize], group: usize, size: usize) {
		Self::do_truncate(maps, map_idxs, group, size, 0);
	}
	fn do_truncate(
		maps: &mut MapIndexMap,
		map_idxs: &[usize],
		group: usize,
		size: usize,
		map_idxs_idx: usize,
	);
}

impl ComponentSliceSet for HNil {
	#[inline]
	fn all_same_len(&self, _len: usize) -> bool {
		true
	}
	#[inline]
	fn populate_type_idx_vec(&self, _idxs: &mut Vec<usize>, _maps: &mut MapIndexMap) {}
	#[inline]
	fn do_insert_all(
		self,
		_maps: &mut MapIndexMap,
		_map_idxs: &[usize],
		_group: usize,
		_map_idx_idx: usize,
		start_index: usize,
	) -> usize {
		start_index
	}
	#[inline]
	fn do_truncate(
		_maps: &mut MapIndexMap,
		_map_idxs: &[usize],
		_group: usize,
		_size: usize,
		_map_idxs_idx: usize,
	) {
	}
}

impl<'a, H: 'static, HI: 'static + ExactSizeIterator<Item = H>, T: ComponentSliceSet>
	ComponentSliceSet for HCons<HI, T>
where
	<T as TypeList>::LenTN: std::ops::Add<generic_array::typenum::B1>,
	<<T as TypeList>::LenTN as std::ops::Add<generic_array::typenum::B1>>::Output:
		generic_array::typenum::Unsigned,
	<<T as TypeList>::LenTN as std::ops::Add<generic_array::typenum::B1>>::Output:
		generic_array::ArrayLength<std::any::TypeId>,
{
	#[inline]
	fn all_same_len(&self, len: usize) -> bool {
		self.head.len() == len && self.tail.all_same_len(len)
	}

	#[inline]
	fn populate_type_idx_vec(&self, idxs: &mut Vec<usize>, maps: &mut MapIndexMap) {
		let entry = maps.entry(std::any::TypeId::of::<H>());
		let index = entry.index();
		idxs.push(index);
		entry.or_insert_with(|| Box::new(DensePagedDataInstance::<H>::with_groups(index, 0)));
		self.tail.populate_type_idx_vec(idxs, maps);
	}

	fn do_insert_all(
		self,
		maps: &mut MapIndexMap,
		map_idxs: &[usize],
		group: usize,
		map_idx_idx: usize,
		_start_index: usize,
	) -> usize {
		let map_idx = map_idxs[map_idx_idx];
		let (_type_id, map) = maps
			.get_index_mut(map_idx)
			.expect("Map is in invalid state!  Shouldn't happen!");
		let start_index = map.get_refmut::<H>().push_all(group, self.head);
		self.tail
			.do_insert_all(maps, map_idxs, group, map_idx_idx + 1, start_index)
	}

	#[inline]
	fn do_truncate(
		maps: &mut MapIndexMap,
		map_idxs: &[usize],
		group: usize,
		size: usize,
		map_idxs_idx: usize,
	) {
		let map_idx = map_idxs[map_idxs_idx];
		let (_type_id, map) = maps
			.get_index_mut(map_idx)
			.expect("Map is in invalid state!  Shouldn't happen!");
		map.get_refmut::<H>().truncate_group(group, size);
		T::do_truncate(maps, map_idxs, group, size, map_idxs_idx + 1);
	}
}
/*
#[derive(Clone, Copy, PartialEq)]
pub struct Query<EntityType: Entity, RO: ComponentSet, RW: ComponentSet, E: ComponentSet> {
	query_idx: usize,
	_read_only_maps: PhantomData<RO>,
	_read_write_maps: PhantomData<RW>,
	_except: PhantomData<E>,
	_entity_type: PhantomData<EntityType>,
}

impl<EntityType: Entity, RO: ComponentSet, RW: ComponentSet, E: ComponentSet>
	Query<EntityType, RO, RW, E>
{
	fn new(idx: usize, _link: &QueryTypedPagedLink, _maps: &MapIndexMap) -> Self {
		// let read_only_maps = RO::get_maps(maps, &link.read_only_maps);
		Self {
			query_idx: idx,
			_read_only_maps: Default::default(),
			_read_write_maps: Default::default(),
			_except: Default::default(),
			_entity_type: Default::default(),
		}
	}

	pub fn connect<'a, CQS: ComponentQuerySet<'a>>(&self, maps: &MapIndexMap) -> () {}

	// pub fn iter<'a, RefRO: ComponentRefSet<'a>, RefRW: ComponentRefMutSet<'a>>(
	// 	&self,
	// 	ro: RefRO,
	// 	rw: RefRW,
	// ) -> QueryIterator<'a, EntityType, RO, RW> {
	// 	// let (_key, links) = map
	// 	// 	.query_mappings
	// 	// 	.get_index(self.query_idx)
	// 	// 	.expect("Map is in bad state");
	// 	QueryIterator {
	// 		_blah: Default::default(),
	// 	}
	// }
}

pub struct QueryIterator<'a, EntityType: Entity, RO: ComponentSet, RW: ComponentSet> {
	_blah: PhantomData<(&'a EntityType, RO, RW)>,
}

impl<'a, EntityType: Entity, RO: ComponentSet, RW: ComponentSet> Iterator
	for QueryIterator<'a, EntityType, RO, RW>
{
	type Item = &'a ();

	fn next(&mut self) -> Option<Self::Item> {
		unimplemented!()
	}
}

pub trait ComponentQuerySet<'a>: HList + TypeList {}

impl<'a> ComponentQuerySet<'a> for HNil {}

impl<'a, H: 'static, T: ComponentQuerySet<'a>> ComponentQuerySet<'a> for HCons<H, T>
where
	<T as TypeList>::LenTN: std::ops::Add<generic_array::typenum::B1>,
	<<T as TypeList>::LenTN as std::ops::Add<generic_array::typenum::B1>>::Output:
		generic_array::typenum::Unsigned,
	<<T as TypeList>::LenTN as std::ops::Add<generic_array::typenum::B1>>::Output:
		generic_array::ArrayLength<std::any::TypeId>,
{
}

pub struct StorageGroupIterator<
	'a,
	CS: 'a + ComponentIteratorSet<'a>,
	G: Iterator<Item = &'a usize>,
> where
	Self: 'a,
{
	groups: G,
	storages: CS,
	// _phantom: PhantomData<&'a ()>,
}

impl<'a, CS: ComponentIteratorSet<'a>, G: Iterator<Item = &'a usize>> Iterator
	for StorageGroupIterator<'a, CS, G>
{
	type Item = CS::IteratorItem;

	fn next(&mut self) -> Option<Self::Item> {
		let group = self.groups.next()?;
		//Some(self.storages.get_group_slice(*group))
		todo!()
	}
}

pub trait ComponentIteratorSet<'a>: HList {
	type IteratorItem: 'a;
	fn get_group_slice(&'a self, last_group: usize) -> Self::IteratorItem;
}

impl<'a> ComponentIteratorSet<'a> for HNil {
	type IteratorItem = HNil;
	#[inline]
	fn get_group_slice(&'a self, _last_group: usize) -> Self::IteratorItem {
		HNil
	}
}

impl<'a, H: 'static, T: ComponentIteratorSet<'a>> ComponentIteratorSet<'a>
	for HCons<Ref<'a, DensePagedDataInstance<H>>, T>
{
	type IteratorItem = HCons<&'a [H], T::IteratorItem>;

	#[inline]
	fn get_group_slice(&'a self, last_group: usize) -> Self::IteratorItem {
		HCons {
			head: self.head.data[last_group].as_slice(),
			tail: self.tail.get_group_slice(last_group),
		}
	}
}

pub trait ComponentStorageSet<'a>: HList {
	type GroupSlice: 'a;
	fn contains_type_id(tid: TypeId) -> bool;
	type IncludeLenTN: generic_array::typenum::Unsigned + generic_array::ArrayLength<TypeId>;
	fn get_include_type_id_at(idx: usize) -> Option<TypeId>;
	type ExcludeLenTN: generic_array::typenum::Unsigned + generic_array::ArrayLength<TypeId>;
	fn get_exclude_type_id_at(idx: usize) -> Option<TypeId>;
	type IncludeSet: ComponentSet;
	type ExcludeSet: ComponentSet;
	type Storages: 'a + ComponentIteratorSet<'a>;
	fn get_storages(maps: &'a MapIndexMap, groups: &[usize]) -> Option<Self::Storages>;
	type IteratorItem: 'a;
	fn get_include_matching_query_groups(
		groups_to_maps: &IndexMap<Vec<TypeId>, Vec<usize>>,
		out: Vec<usize>,
	) -> Vec<usize>;
	fn get_exclude_matching_query_groups(
		groups_to_maps: &IndexMap<Vec<TypeId>, Vec<usize>>,
		out: Vec<usize>,
	) -> Vec<usize>;
	fn get_map_idxs(maps: &MapIndexMap, out: Vec<usize>) -> Vec<usize>;
}

impl<'a> ComponentStorageSet<'a> for HNil {
	type GroupSlice = HNil;
	#[inline]
	fn contains_type_id(tid: TypeId) -> bool {
		false
	}
	type IncludeLenTN = generic_array::typenum::U0;
	#[inline]
	fn get_include_type_id_at(idx: usize) -> Option<TypeId> {
		None
	}
	type ExcludeLenTN = generic_array::typenum::U0;
	#[inline]
	fn get_exclude_type_id_at(idx: usize) -> Option<TypeId> {
		None
	}
	type IncludeSet = HNil;
	type ExcludeSet = HNil;
	type Storages = HNil;
	#[inline]
	fn get_storages(_maps: &'a MapIndexMap, groups: &[usize]) -> Option<Self::Storages> {
		Some(HNil)
	}
	type IteratorItem = HNil;
	#[inline]
	fn get_include_matching_query_groups(
		_groups_to_maps: &IndexMap<Vec<TypeId>, Vec<usize>, RandomState>,
		out: Vec<usize>,
	) -> Vec<usize> {
		out
	}
	#[inline]
	fn get_exclude_matching_query_groups(
		_groups_to_maps: &IndexMap<Vec<TypeId>, Vec<usize>, RandomState>,
		out: Vec<usize>,
	) -> Vec<usize> {
		out
	}
	#[inline]
	fn get_map_idxs(_maps: &MapIndexMap, out: Vec<usize>) -> Vec<usize> {
		out
	}
}

impl<'a, H: 'static, T: ComponentStorageSet<'a>> ComponentStorageSet<'a> for HCons<&H, T>
where
	<T as ComponentStorageSet<'a>>::IncludeLenTN: std::ops::Add<generic_array::typenum::B1>,
	<<T as ComponentStorageSet<'a>>::IncludeLenTN as std::ops::Add<generic_array::typenum::B1>>::Output:
		generic_array::typenum::Unsigned,
	<<T as ComponentStorageSet<'a>>::IncludeLenTN as std::ops::Add<generic_array::typenum::B1>>::Output:
		generic_array::ArrayLength<std::any::TypeId>,
	<T as ComponentStorageSet<'a>>::ExcludeLenTN: std::ops::Add<generic_array::typenum::B1>,
	<<T as ComponentStorageSet<'a>>::ExcludeLenTN as std::ops::Add<generic_array::typenum::B1>>::Output:
	generic_array::typenum::Unsigned,
	<<T as ComponentStorageSet<'a>>::ExcludeLenTN as std::ops::Add<generic_array::typenum::B1>>::Output:
	generic_array::ArrayLength<std::any::TypeId>,
	<<T as ComponentStorageSet<'a>>::IncludeSet as TypeList>::LenTN: std::ops::Add<generic_array::typenum::B1>,
	<<<T as ComponentStorageSet<'a>>::IncludeSet as TypeList>::LenTN as std::ops::Add<generic_array::typenum::B1>>::Output: generic_array::ArrayLength<std::any::TypeId>
{
	type GroupSlice = &'a [H];

	#[inline]
	fn contains_type_id(tid: TypeId) -> bool {
		tid == std::any::TypeId::of::<H>() || T::contains_type_id(tid)
	}

	type IncludeLenTN = generic_array::typenum::Add1<T::IncludeLenTN>;
	#[inline]
	fn get_include_type_id_at(idx: usize) -> Option<TypeId> {
		if idx == 0 {
			Some(TypeId::of::<H>())
		} else {
			T::get_include_type_id_at(idx - 1)
		}
	}

	type ExcludeLenTN = generic_array::typenum::Add1<T::ExcludeLenTN>;
	#[inline]
	fn get_exclude_type_id_at(idx: usize) -> Option<TypeId> {
			T::get_exclude_type_id_at(idx )
	}

	type IncludeSet = HCons<H, T::IncludeSet>;
	type ExcludeSet = T::ExcludeSet;

	type Storages = HCons<Ref<'a, DensePagedDataInstance<H>>, T::Storages>;

	#[inline]
	fn get_storages(maps: &'a MapIndexMap, groups: &[usize]) -> Option<Self::Storages> {
		// let head: Ref<'a, std::boxed::Box<(dyn DensePagedData + 'static)>> = maps.get(&TypeId::of::<H>())?.borrow();
		let head: Ref<'a, DensePagedDataInstance<H>> = Ref::map(maps.get(&TypeId::of::<H>())?.borrow(), |data| {
			data.cast::<H>()
			// let d = data.cast::<H>().data;
			// &groups.into_iter().map(|&group| d[group].as_slice())
		});
		let tail = T::get_storages(maps, groups)?;
		Some(HCons { head, tail })
		// None
	}

	type IteratorItem = HCons<&'a H, T::IteratorItem>;

	#[inline]
	fn get_include_matching_query_groups(groups_to_maps: &IndexMap<Vec<TypeId>, Vec<usize>, RandomState>, mut out: Vec<usize>) -> Vec<usize> {
		for (idx, type_ids) in groups_to_maps.keys().enumerate() {
			if type_ids.iter().copied().all(Self::contains_type_id) {
				out.push(idx);
			}
		}
		T::get_include_matching_query_groups(groups_to_maps, out)
	}

	#[inline]
	fn get_exclude_matching_query_groups(groups_to_maps: &IndexMap<Vec<TypeId>, Vec<usize>, RandomState>, out: Vec<usize>) -> Vec<usize> {
		todo!()
	}

	#[inline]
	fn get_map_idxs(maps: &MapIndexMap, mut out: Vec<usize>) -> Vec<usize> {
		if let Some((idx, _k, _v)) = maps.get_full(&std::any::TypeId::of::<H>()) {
			out.push(idx); T::get_map_idxs(maps, out)
		} else {
			vec![]
		}
	}
}

pub struct ComponentIterSetIterator<'a, E: ComponentStorageSet<'a>> {
	storages: E::Storages,
	groups: Vec<usize>,
	_phantom: PhantomData<&'a E>,
}

impl<'a, E: ComponentStorageSet<'a>> Iterator for ComponentIterSetIterator<'a, E> {
	type Item = <E::Storages as ComponentIteratorSet<'a>>::IteratorItem;

	fn next(&mut self) -> Option<Self::Item> {
		// let group = self.groups.pop()?;
		//Some(self.storages.get_group_slice(group))
		todo!()
	}
}

pub struct ComponentIterSetIntoIterator<'a, E: ComponentStorageSet<'a>> {
	storages: E::Storages,
	groups: Vec<usize>,
}

impl<'a, E: 'static + ComponentStorageSet<'a>> IntoIterator
	for ComponentIterSetIntoIterator<'a, E>
{
	type Item = <E::Storages as ComponentIteratorSet<'a>>::IteratorItem;
	type IntoIter = ComponentIterSetIterator<'a, E>;

	fn into_iter(self) -> Self::IntoIter {
		ComponentIterSetIterator {
			storages: self.storages,
			groups: self.groups,
			_phantom: Default::default(),
		}
	}
}

// impl<'a, E: ComponentStorageSet<'a>> Iterator for ComponentIterSetIterator<'a, E> {
// 	type Item = <E::Storages as ComponentIteratorSet<'a>>::IteratorItem;
//
// 	fn next(&mut self) -> Option<Self::Item> {
// 		let items = self.storages.get_group_slice(self.groups.pop()?);
// 		dbg!(items);
// 		unimplemented!()
// 	}
// }

pub trait ComponentRefSet<'a>: HList + TypeList {}

impl<'a> ComponentRefSet<'a> for HNil {}

impl<'a, H: 'static, T: ComponentRefSet<'a>> ComponentRefSet<'a> for HCons<H, T>
where
	<T as TypeList>::LenTN: std::ops::Add<generic_array::typenum::B1>,
	<<T as TypeList>::LenTN as std::ops::Add<generic_array::typenum::B1>>::Output:
		generic_array::typenum::Unsigned,
	<<T as TypeList>::LenTN as std::ops::Add<generic_array::typenum::B1>>::Output:
		generic_array::ArrayLength<std::any::TypeId>,
{
}

pub trait ComponentRefMutSet<'a>: HList + ComponentRefSet<'a> {}

impl<'a> ComponentRefMutSet<'a> for HNil {}

impl<'a, H: 'static, T: ComponentRefMutSet<'a>> ComponentRefMutSet<'a> for HCons<H, T>
where
	<T as TypeList>::LenTN: std::ops::Add<generic_array::typenum::B1>,
	<<T as TypeList>::LenTN as std::ops::Add<generic_array::typenum::B1>>::Output:
		generic_array::typenum::Unsigned,
	<<T as TypeList>::LenTN as std::ops::Add<generic_array::typenum::B1>>::Output:
		generic_array::ArrayLength<std::any::TypeId>,
{
}
*/

#[cfg(test)]
mod tests {
	//use frunk::hlist;

	use crate::{tl, tlp, TL};

	use super::*;

	#[test]
	fn sparse_typed_page_multimap_tests() {
		// let mut map = SparseTypedPagedMap::<u64>::new();
		// assert_eq!(map.insert(1, (21usize, 6.28f32, true)), Ok(()));
		//
		// assert!(map.remove(2).is_err());
		//
		// let inserts: Vec<_> = (2..10u64).map(|i| (i, (21usize, 6.28f32, true))).collect();
		// assert_eq!(map.extend_iter(inserts), Ok(()));
		//
		// assert_eq!(map.contains(2), true);
		// map.remove(2).unwrap();
		// assert_eq!(map.contains(2), false);
		// assert!(map.remove(2).is_err());
		//
		// assert_eq!(
		// 	map.extend_iters(
		// 		vec![11u64, 12u64, 13u64].into_iter(),
		// 		hlist!(
		// 			vec![1usize, 2usize, 3usize].into_iter(),
		// 			vec![1.0f32, 2f32, 3f32].into_iter(),
		// 			vec![true, false, true].into_iter(),
		// 		)
		// 	),
		// 	Ok(())
		// );

		// assert_eq!(map.insert(22, (21usize, 6.28f32, true)), Ok(()));
		// assert_eq!(
		// 	map.extend_iters(
		// 		vec![21u64, 22u64, 23u64].into_iter(),
		// 		hlist!(
		// 			vec![1usize, 2usize, 3usize].into_iter(),
		// 			vec![1.0f32, 2f32, 3f32].into_iter(),
		// 			vec![true, false, true].into_iter(),
		// 		)
		// 	),
		// 	Err(SparseTypedPagedMapErrors::SecondaryIndexError(
		// 		SecondaryIndexErrors::IndexAlreadyExists(22)
		// 	))
		// );
		// assert_eq!(
		// 	map.extend_iters(
		// 		vec![22u64, 23u64].into_iter(),
		// 		hlist!(
		// 			vec![2usize, 3usize].into_iter(),
		// 			vec![2f32, 3f32].into_iter(),
		// 			vec![false, true].into_iter(),
		// 		)
		// 	),
		// 	Err(SparseTypedPagedMapErrors::SecondaryIndexError(
		// 		SecondaryIndexErrors::IndexAlreadyExists(22)
		// 	))
		// );
	}

	#[test]
	fn empty_entities() {
		// let mut map = SparseTypedPagedMap::<u64>::new();
		// assert_eq!(map.insert(1, ()), Ok(()));
		// assert_eq!(map.contains(1), true);
	}

	#[test]
	fn get() {
		// let mut map = SparseTypedPagedMap::<u64>::new();
		// let mut query = map.query::<TL![&mut usize, &u16]>().unwrap();
		// let got = query.get(1);
		// assert!(got.is_none());
		// map.insert(1, (21usize, 2u16)).unwrap();
		// {
		// 	let got = query.get(1);
		// 	assert!(got.is_some());
		// 	let (mut first, (second, ())) = got.unwrap();
		// 	assert_eq!(*first, 21);
		// 	*first *= *second as usize;
		// }
		// {
		// 	let got = query.get(1);
		// 	assert!(got.is_some());
		// 	let (first, _second) = got.unwrap();
		// 	assert_eq!(*first, 42);
		// }
		// map.insert(2, (2usize,)).unwrap();
		// map.insert(3, (2usize,)).unwrap();
		// map.insert(4, (2usize,)).unwrap();
		// {
		// 	let got = query.get(2);
		// 	assert!(got.is_none());
		// }
		// map.insert_components(2, (2u16,)).unwrap();
		// {
		// 	let got = query.get(2);
		// 	assert!(got.is_some());
		// 	let (_first, (second, ())) = got.unwrap();
		// 	assert_eq!(*second, 2);
		// }
		// map.insert_components(3, (2u16,)).unwrap();
		// map.insert_components(4, (2u16,)).unwrap();
		// map.remove_components::<(u16,)>(3).unwrap();
		// map.remove_components::<(u16,)>(4).unwrap();
		// map.remove_components::<(u16,)>(2).unwrap();
		// map.remove_components::<(u16,)>(1).unwrap();
	}

	#[test]
	fn queries_none() {
		// let mut map = SparseTypedPagedMap::<u64>::new();
		// map.extend_iter((1..=2).map(|e| (e, (e as usize, format!("test: {}", e)))))
		// 	.unwrap();
		// assert!(map.query::<TL![]>().unwrap().iter_slices().next().is_none());
	}

	#[test]
	fn queries_empty() {
		let map = SparseTypedPagedMap::<u64>::new();
		assert!(map
			.query::<TL![&usize, &u32]>()
			.unwrap()
			.iter_slices()
			.next()
			.is_none());
	}

	#[test]
	fn query_iter() {
		// let mut map = SparseTypedPagedMap::<u64>::new();
		// map.extend_iter((1..=2).map(|e| (e, (e as usize, format!("test: {}", e)))))
		// 	.unwrap();
		// let mut query = map.query::<TL![&mut usize]>().unwrap();
		// assert_eq!(*query.get(1).unwrap().0, 1);
		// for tlp![mut us] in query.iter_slices() {
		// 	for u in us.iter_mut() {
		// 		*u += 1;
		// 	}
		// }
		// assert_eq!(*query.get(1).unwrap().0, 2);
		// for tlp![mut us] in query.iter_slices() {
		// 	for u in us.iter_mut() {
		// 		*u *= 2;
		// 	}
		// }
		// assert_eq!(*query.get(1).unwrap().0, 4);
	}

	#[test]
	fn queries_opt() {
		// let mut map = SparseTypedPagedMap::<u64>::new();
		// map.insert(1, (1usize,)).unwrap();
		// map.insert(2, (2usize, 2u16)).unwrap();
		// let mut query = map.query::<TL![&usize, Option<&u16>]>().unwrap();
		// assert_eq!(*query.get(1).unwrap().0, 1);
		// assert!(query.get(1).unwrap().1.0.is_none());
		// assert_eq!(*query.get(2).unwrap().0, 2);
		// assert_eq!(*query.get(2).unwrap().1.0.unwrap(), 2);
		// map.insert(3, (1usize,)).unwrap();
		// map.insert(4, (2usize, 2u16)).unwrap();
		// assert_eq!(*query.get(3).unwrap().0, 1);
		// assert!(query.get(3).unwrap().1.0.is_none());
		// assert_eq!(*query.get(4).unwrap().0, 2);
		// assert_eq!(*query.get(4).unwrap().1.0.unwrap(), 2);
		//
		// let mut query = map.query::<TL![&isize, Option<&i16>]>().unwrap();
		// map.insert(5, (1isize,)).unwrap();
		// map.insert(6, (2isize, 2i16)).unwrap();
		// assert_eq!(*query.get(5).unwrap().0, 1);
		// assert!(query.get(5).unwrap().1.0.is_none());
		// assert_eq!(*query.get(6).unwrap().0, 2);
		// assert_eq!(*query.get(6).unwrap().1.0.unwrap(), 2);
	}

	#[test]
	fn queries_mut_opt() {
		// let mut map = SparseTypedPagedMap::<u64>::new();
		// map.insert(1, (1usize,)).unwrap();
		// map.insert(2, (2usize, 2u16)).unwrap();
		// let mut query = map.query::<TL![&usize, Option<&mut u16>]>().unwrap();
		// assert_eq!(*query.get(1).unwrap().0, 1);
		// assert!(query.get(1).unwrap().1.0.is_none());
		// assert_eq!(*query.get(2).unwrap().0, 2);
		// assert_eq!(*query.get(2).unwrap().1.0.unwrap(), 2);
		//
		// *query.get(2).unwrap().1.0.unwrap() += 1;
		// assert_eq!(*query.get(2).unwrap().1.0.unwrap(), 3);
	}

	#[test]
	fn queries_opt_iter() {
		// let mut map = SparseTypedPagedMap::<u64>::new();
		// let query = map.query::<TL![&usize, Option<&u16>]>().unwrap();
		// map.insert(1, (1usize,)).unwrap();
		// map.insert(2, (2usize, 2u16)).unwrap();
		// let mut iter = query.iter_slices();
		// assert_eq!(*iter.next().unwrap().1.0.unwrap(), [2]);
		// assert!(iter.next().unwrap().1.0.is_none());
		// assert!(iter.next().is_none());
	}

	#[test]
	fn queries_with_entity() {
		// let mut map = SparseTypedPagedMap::<u64>::new();
		// let mut query = map.query::<TL![&usize, EntityRef]>().unwrap();
		// map.insert(1, (1usize,)).unwrap();
		// map.insert(2, (2usize, 2u16)).unwrap();
		// map.insert(3, (3usize,)).unwrap();
		// for tlp![us, es] in query.iter_slices() {
		// 	for (u, e) in us.iter().zip(es.iter()) {
		// 		assert_eq!(*u, *e as usize);
		// 	}
		// }
		// assert_eq!(query.get(1).unwrap().1.0, 1);
	}

	#[test]
	fn queries_skip_empty() {
		// let mut map = SparseTypedPagedMap::<u64>::new();
		// let mut query = map.query::<TL![&usize]>().unwrap();
		// map.insert(1, (1usize,)).unwrap();
		// map.insert(2, (2usize, 2u16)).unwrap();
		// map.insert(3, (1usize,)).unwrap();
		// assert_eq!(query.iter_slices().fold(0, |a, tlp![s]| a + s.len()), 3);
		// assert_eq!(query.iter_slices().fold(0, |a, _| a + 1), 2);
		// map.remove(2).unwrap();
		// assert!(query.get(1).is_some());
		// assert!(query.get(2).is_none());
		// assert!(query.get(3).is_some());
		// assert_eq!(query.iter_slices().fold(0, |a, tlp![s]| a + s.len()), 2);
		// assert_eq!(query.iter_slices().fold(0, |a, _| a + 1), 1);
	}

	#[test]
	fn queries_exclude() {
		// let mut map = SparseTypedPagedMap::<u64>::new();
		// map.insert(1, (1usize,)).unwrap();
		// map.insert(2, (2usize, 2u16)).unwrap();
		// let mut query = map.query::<TL![&usize, Exclude<u16>]>().unwrap();
		// assert!(query.get(1).is_some());
		// assert!(query.get(2).is_none());
		// assert_eq!(query.iter_slices().map(|tlp![s, ()]| s.len()).sum::<usize>(), 1);
	}

	#[test]
	fn queries_ref() {
		let mut map = SparseTypedPagedMap::<u64>::new();
		map.query::<TL![&mut usize, &mut String]>()
			.unwrap()
			.lock()
			.insert(1, tl![1, format!("test: {}", 1)])
			.unwrap();
		map.query::<TL![&mut usize, &mut String]>()
			.unwrap()
			.lock()
			.insert(2, tl![2, format!("test: {}", 2)])
			.unwrap();
		// map.extend_iter((1..=2).map(|e| (e, (e as usize, format!("test: {}", e)))))
		// 	.unwrap();
		assert_eq!(
			map.query::<TL![&usize]>()
				.unwrap()
				.iter_slices()
				.next()
				.map(|tlp![usizes]| usizes.iter().sum()),
			Some(3)
		);
		assert_eq!(
			map.query::<TL![&usize,]>()
				.unwrap()
				.iter_slices()
				.next()
				.map(|tlp![usizes]| usizes.iter().sum()),
			Some(3)
		);
		assert_eq!(
			map.query::<TL![&usize, &String]>()
				.unwrap()
				.iter_slices()
				.next()
				.map(|tlp![usizes, _string]| usizes.iter().sum()),
			Some(3)
		);
	}

	#[test]
	fn mut_queries_mut() {
		let map = SparseTypedPagedMap::<u64>::new();
		{
			let mut query = map.query::<TL![&mut usize, &mut u16]>().unwrap();
			let mut query = query.lock();
			assert!(query.get::<TL![&mut usize, &mut u16]>(1).is_err());
			query.insert(1, tl!(21, 2)).unwrap();
			assert_eq!(*query.get::<TL![&mut usize, &mut u16]>(1).unwrap().0, 21);
			assert_eq!(*query.get::<TL![&mut usize, &mut u16], >(1).unwrap().1.0, 2);
		}
	}

	#[test]
	fn queries_get_reduced() {
		let map = SparseTypedPagedMap::<u64>::new();
		{
			let mut query = map.query::<TL![&mut usize, &mut u16]>().unwrap();
			let mut query = query.lock();
			assert!(query.get::<TL![&mut usize]>(1).is_err());
			query.insert(1, tl!(21, 2)).unwrap();
			let thing0: usize = *query.get::<TL![&mut usize]>(1).unwrap().0;
			assert_eq!(thing0, 21);
			let thing1: u16 = *query.get::<TL![&mut u16]>(1).unwrap().0;
			assert_eq!(thing1, 2);
		}
	}

	#[test]
	fn queries_extend_vecs() {
		let map = SparseTypedPagedMap::<u64>::new();
		let mut query = map.query::<TL![&mut usize, &mut u16]>().unwrap();
		let mut query = query.lock();
		query
			.extend_vecs(0..10, tl![vec![1; 10], vec![2; 10]])
			.unwrap();
		assert_eq!(*query.get::<TL![&mut usize, &mut u16]>(1).unwrap().0, 1);
		assert_eq!(*query.get::<TL![&mut usize, &mut u16]>(9).unwrap().0, 1);
	}

	#[test]
	fn queries_iter_multiple() {
		let map = SparseTypedPagedMap::<u64>::new();
		map.query::<TL![&mut usize, &mut u8]>()
			.unwrap()
			.lock()
			.insert(1, tl![1, 1])
			.unwrap();
		map.query::<TL![&mut usize, &mut u16]>()
			.unwrap()
			.lock()
			.insert(2, tl![2, 2])
			.unwrap();
		map.query::<TL![&mut usize, &mut u32]>()
			.unwrap()
			.lock()
			.insert(3, tl![3, 3])
			.unwrap();
		assert_eq!(
			map.query::<TL![&usize]>()
				.unwrap()
				.iter_slices()
				.map(|tlp![s]| s[0])
				.sum::<usize>(),
			6
		);
	}

	// TODO:  Figure out how to enforce this get type stuff at compile time, frunk can do it...
	#[test]
	#[should_panic]
	fn should_panic_get_incorrect_type() {
		let map = SparseTypedPagedMap::<u64>::new();
		let mut query = map.query::<TL![&mut usize, &mut u16]>().unwrap();
		let mut query = query.lock();
		query.insert(1, tl!(21, 2)).unwrap();
		query.get::<TL![&isize]>(1).unwrap(); // should panic because `&isize` is not one of `[&mut usize, &mut u16]`
	}

	#[test]
	#[should_panic]
	fn should_panic_get_incorrect_mut() {
		let map = SparseTypedPagedMap::<u64>::new();
		let mut query = map.query::<TL![&mut usize, &mut u16]>().unwrap();
		let mut query = query.lock();
		query.insert(1, tl!(21, 2)).unwrap();
		query.get::<TL![&usize]>(1).unwrap(); // should panic because `&usize` is not one of `[&mut usize, &mut u16]`
	}

	#[test]
	#[should_panic]
	fn should_panic_get_incorrect_ref() {
		let map = SparseTypedPagedMap::<u64>::new();
		{
			let mut query = map.query::<TL![&mut usize, &mut u16]>().unwrap();
			let mut query = query.lock();
			query.insert(1, tl!(21, 2)).unwrap();
		}
		let mut query = map.query::<TL![&usize, &mut u16]>().unwrap();
		let mut query = query.lock();
		query.get::<TL![&mut usize]>(1).unwrap(); // should panic because `&usize` is not one of `[&mut usize, &mut u16]`
	}

	#[test]
	fn queries_mut() {
		// let mut map = SparseTypedPagedMap::<u64>::new();
		// map.extend_iter((1..=2).map(|e| (e, (e as usize, e as u16))))
		// 	.unwrap();
		// assert_eq!(
		// 	map.query::<TL![&mut usize]>()
		// 		.unwrap()
		// 		.iter_slices()
		// 		.next()
		// 		.map(|tlp![mut usizes]| {
		// 			usizes.iter_mut().for_each(|u| *u *= 2);
		// 			usizes.iter().sum()
		// 		}),
		// 	Some(6)
		// );
		// // usizes here are still mutated from the prior one, thus [2, 4], plus [1, 2], is [3, 6]
		// assert_eq!(
		// 	map.query::<TL![&mut usize, &u16]>()
		// 		.unwrap()
		// 		.iter_slices()
		// 		.next()
		// 		.map(|tlp![mut usizes, u16s]| {
		// 			usizes
		// 				.iter_mut()
		// 				.zip(u16s.iter())
		// 				.for_each(|(us, u16)| *us += *u16 as usize);
		// 			usizes.iter().sum()
		// 		}),
		// 	Some(9)
		// );
	}

	#[test]
	fn queries() {
		// let mut map = SparseTypedPagedMap::<u64>::new();
		// map.extend_iter((1..=2).map(|e| (e, (e as usize, format!("test: {}", e)))))
		// 	.unwrap();
		//
		// for tlp![usizes] in map.query::<TL![&usize]>().unwrap().iter_slices() {
		// 	assert_eq!(&*usizes, &[1, 2]);
		// }
		// map.query::<TL![&usize, &String]>().unwrap();
		// for tlp![s0, s1, s2] in map.query::<TL![&i8, &i16, &i32]>().unwrap().iter_slices() {
		// 	assert_eq!(
		// 		*s0.first().unwrap() as usize
		// 			+ *s1.first().unwrap() as usize
		// 			+ *s2.first().unwrap() as usize,
		// 		usize::MAX,
		// 		"Should never be run"
		// 	);
		// }

		{
			// let query = map.query::<Hlist![], Hlist![], Hlist![]>().unwrap();
			//for () in query.iter() {}
		}

		{
			// let query = map.query::<Hlist![usize], Hlist![], Hlist![]>();
			//for () in query.iter() {}
		}
	}
}
