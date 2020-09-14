use std::any::{Any, TypeId};
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

use indexmap::IndexMap;

use crate::entity::Entity;
use crate::frunk::{prelude::HList, HCons, HNil};
use crate::storages::secondary_index::{SecondaryIndex, SecondaryIndexErrors};
use crate::storages::TypeList;
use crate::utils::unique_hasher::UniqueHasherBuilder;
use std::cell::RefCell;
use std::sync::PoisonError;

/// Possible Errors given by a SparsePageMap's operation.
#[derive(Debug, PartialEq, Eq)]
pub enum SparseTypedPagedMapErrors<EntityType: Entity> {
	PoisonError,
	SecondaryIndexError(SecondaryIndexErrors<EntityType>),
	ComponentStorageDoesNotExist(&'static str),
	EntityDoesNotExistInStorage(EntityType, &'static str),
	IteratorsNotAllSameLength,
}

impl<EntityType: Entity> std::error::Error for SparseTypedPagedMapErrors<EntityType> {
	fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
		use SparseTypedPagedMapErrors::*;
		match self {
			PoisonError => None,
			SecondaryIndexError(source) => Some(source),
			ComponentStorageDoesNotExist(_name) => None,
			EntityDoesNotExistInStorage(_entity, _name) => None,
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
			ComponentStorageDoesNotExist(name) => {
				write!(f, "Component Static Storage does not exist for: {:?}", name)
			}
			EntityDoesNotExistInStorage(entity, name) => write!(
				f,
				"Entity `{:?}` does not exist in component static storage: {}",
				entity, name
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
	fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
	fn len_groups(&self) -> usize;
	fn resize(&mut self, new_len: usize);
	fn truncate_group(&mut self, group: usize, len: usize);
	fn swap_remove(&mut self, group: usize, index: u8);
}

impl dyn DensePagedData {
	fn cast<DataType: 'static>(&self) -> &DensePagedDataInstance<DataType> {
		self.as_any()
			.downcast_ref()
			.expect("Type mismatch in map!  Shouldn't happen!")
	}

	fn cast_mut<DataType: 'static>(&mut self) -> &mut DensePagedDataInstance<DataType> {
		self.as_any_mut()
			.downcast_mut()
			.expect("Type mismatch in map!  Shouldn't happen!")
	}

	fn get<DataType: 'static>(&self, group: usize, idx: usize) -> Option<&DataType> {
		let data_storage: &DensePagedDataInstance<DataType> = self.cast();
		let group_storage = &data_storage.data[group];
		group_storage.get(idx)
	}

	fn get_mut<DataType: 'static>(&mut self, group: usize, idx: usize) -> Option<&mut DataType> {
		let data_storage: &mut DensePagedDataInstance<DataType> = self.cast_mut();
		let group_storage = &mut data_storage.data[group];
		group_storage.get_mut(idx)
	}
}

struct DensePagedDataInstance<DataType: 'static> {
	data: Vec<Vec<DataType>>,
}

impl<DataType: 'static> DensePagedDataInstance<DataType> {
	fn new() -> Self {
		Self::with_groups(0)
	}

	fn with_groups(group_size: usize) -> Self {
		Self {
			data: (0..group_size).map(|_| vec![]).collect(),
		}
	}

	fn push(&mut self, group: usize, data: DataType) -> usize {
		let group_storage = &mut self.data[group];
		group_storage.push(data);
		group_storage.len() - 1
	}

	fn push_all<I: IntoIterator<Item = DataType>>(&mut self, group: usize, data: I) -> usize {
		let group_storage = &mut self.data[group];
		let start_idx = group_storage.len();
		group_storage.extend(data);
		start_idx
	}
	fn get(&self, group: usize, idx: usize) -> Option<&DataType> {
		self.data[group].get(idx)
	}

	fn get_mut(&mut self, group: usize, idx: usize) -> Option<&mut DataType> {
		self.data[group].get_mut(idx)
	}
}

impl<DataType: 'static> private::Sealed for DensePagedDataInstance<DataType> {}

impl<DataType: 'static> DensePagedData for DensePagedDataInstance<DataType> {
	#[inline]
	fn as_any(&self) -> &dyn Any {
		self
	}
	#[inline]
	fn as_any_mut(&mut self) -> &mut dyn Any {
		self
	}
	#[inline]
	fn len_groups(&self) -> usize {
		self.data.len()
	}
	#[inline]
	fn resize(&mut self, new_len: usize) {
		self.data.resize_with(new_len, Vec::new);
	}
	#[inline]
	fn truncate_group(&mut self, group: usize, len: usize) {
		self.data[group].truncate(len);
	}
	#[inline]
	fn swap_remove(&mut self, group: usize, index: u8) {
		self.data[group].swap_remove(index as usize);
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

	fn new(group: usize, index: usize) -> Self {
		Self { group, index }
	}
}

#[derive(PartialEq, Eq)]
struct QueryTypedPagedKey<RO: ComponentSet, RW: ComponentSet, E: ComponentSet> {
	read_only: generic_array::GenericArray<TypeId, RO::LenTN>,
	read_write: generic_array::GenericArray<TypeId, RW::LenTN>,
	except: generic_array::GenericArray<TypeId, E::LenTN>,
}

#[derive(PartialEq, Eq)]
struct QueryTypedPagedKeyBoxed {
	read_only: Box<[TypeId]>,
	read_write: Box<[TypeId]>,
	except: Box<[TypeId]>,
}

impl<RO: ComponentSet, RW: ComponentSet, E: ComponentSet> QueryTypedPagedKey<RO, RW, E> {
	fn new() -> Self {
		Self {
			read_only: generic_array::GenericArray::from_exact_iter(RO::iter_types()).unwrap(),
			read_write: generic_array::GenericArray::from_exact_iter(RW::iter_types()).unwrap(),
			except: generic_array::GenericArray::from_exact_iter(E::iter_types()).unwrap(),
		}
	}

	fn to_box(self) -> QueryTypedPagedKeyBoxed {
		QueryTypedPagedKeyBoxed {
			read_only: self.read_only.to_vec().into_boxed_slice(),
			read_write: self.read_write.to_vec().into_boxed_slice(),
			except: self.except.to_vec().into_boxed_slice(),
		}
	}
}

impl<RO: ComponentSet, RW: ComponentSet, E: ComponentSet> Hash for QueryTypedPagedKey<RO, RW, E> {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.read_only.as_slice().hash(state);
		self.read_write.as_slice().hash(state);
		self.except.as_slice().hash(state);
	}
}

impl Hash for QueryTypedPagedKeyBoxed {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.read_only.as_ref().hash(state);
		self.read_write.as_ref().hash(state);
		self.except.as_ref().hash(state);
	}
}

impl<RO: ComponentSet, RW: ComponentSet, E: ComponentSet>
	indexmap::Equivalent<QueryTypedPagedKeyBoxed> for QueryTypedPagedKey<RO, RW, E>
{
	fn equivalent(&self, key: &QueryTypedPagedKeyBoxed) -> bool {
		key.read_only.as_ref() == self.read_only.as_slice()
			&& key.read_write.as_ref() == self.read_write.as_slice()
			&& key.except.as_ref() == self.except.as_slice()
	}
}

/// These are the indexes to the `group_sets_to_maps`
struct QueryTypedPagedLink {
	read_only_groups: Vec<usize>,
	read_write_groups: Vec<usize>,
	except_groups: Vec<usize>,
	read_only_maps: Vec<usize>,
	read_write_maps: Vec<usize>,
}

type MapIndexMap = IndexMap<TypeId, RefCell<Box<dyn DensePagedData>>, UniqueHasherBuilder>;

pub struct SparseTypedPagedMap<EntityType: Entity> {
	reverse: SecondaryIndex<EntityType, ComponentLocations>,
	entities: Vec<Vec<EntityType>>,
	maps: MapIndexMap,
	group_sets_to_maps: IndexMap<Vec<TypeId>, Vec<usize>>,
	query_mappings: IndexMap<QueryTypedPagedKeyBoxed, QueryTypedPagedLink>,
}

impl<EntityType: Entity> Default for SparseTypedPagedMap<EntityType> {
	fn default() -> Self {
		Self::new()
	}
}

impl<EntityType: Entity> SparseTypedPagedMap<EntityType> {
	// private

	fn update_query_mappings(
		_sets_to_maps: &IndexMap<Vec<TypeId>, Vec<usize>>,
		query_mappings: &mut IndexMap<QueryTypedPagedKeyBoxed, QueryTypedPagedLink>,
		_group: usize,
	) {
		for (_query, _link) in query_mappings.iter() {
			todo!();
		}
	}

	// public
	pub fn new() -> Self {
		Self {
			reverse: SecondaryIndex::new(ComponentLocations::INVALID),
			entities: Default::default(),
			maps: IndexMap::with_hasher(UniqueHasherBuilder),
			group_sets_to_maps: Default::default(),
			query_mappings: Default::default(),
		}
	}

	pub fn contains(&self, entity: EntityType) -> bool {
		self.reverse.get(entity).is_ok()
	}

	pub fn insert<C: ComponentSet>(
		&mut self,
		entity: EntityType,
		components: C,
	) -> Result<(), SparseTypedPagedMapErrors<EntityType>> {
		let location = self.reverse.insert_mut(entity)?;
		let mut cset: generic_array::GenericArray<TypeId, C::LenTN> =
			generic_array::GenericArray::from_exact_iter(C::iter_types()).unwrap();
		C::populate_type_slice(cset.as_mut_slice());
		let maps = &mut self.maps;
		let prior_group_size = self.group_sets_to_maps.len();
		let (group, map_idxs) = if let Some((group, _key, map_idxs)) =
			self.group_sets_to_maps.get_full(cset.as_slice())
		{
			(group, map_idxs)
		} else {
			self.group_sets_to_maps
				.insert(cset.to_vec(), C::into_type_idx_vec(maps));
			self.entities.push(Vec::with_capacity(1));
			let group = self.group_sets_to_maps.len() - 1;
			Self::update_query_mappings(&self.group_sets_to_maps, &mut self.query_mappings, group);
			(group, self.group_sets_to_maps.get_index(group).unwrap().1)
		};
		if group >= prior_group_size {
			for map in self.maps.values_mut() {
				map.get_mut().resize(group + 1);
			}
		}
		*location = components.insert(&mut self.maps, map_idxs, group);
		self.entities[group].push(entity);
		Ok(())
	}

	pub fn extend_iter<C: ComponentSet, I: IntoIterator<Item = (EntityType, C)>>(
		&mut self,
		iter: I,
	) -> Result<(), SparseTypedPagedMapErrors<EntityType>> {
		let mut iter = iter.into_iter();
		if let Some((entity, components)) = iter.next() {
			let mut cset: generic_array::GenericArray<TypeId, C::LenTN> =
				generic_array::GenericArray::from_exact_iter(C::iter_types()).unwrap();
			C::populate_type_slice(cset.as_mut_slice());
			let maps = &mut self.maps;
			let prior_group_size = self.group_sets_to_maps.len();
			let (group, map_idxs) = if let Some((group, _key, map_idxs)) =
				self.group_sets_to_maps.get_full(cset.as_slice())
			{
				(group, map_idxs)
			} else {
				self.group_sets_to_maps
					.insert(cset.to_vec(), C::into_type_idx_vec(maps));
				self.entities.push(Vec::with_capacity(iter.size_hint().0));
				let group = self.group_sets_to_maps.len() - 1;
				Self::update_query_mappings(
					&self.group_sets_to_maps,
					&mut self.query_mappings,
					group,
				);
				(group, self.group_sets_to_maps.get_index(group).unwrap().1)
			};
			if group >= prior_group_size {
				for map in maps.values_mut() {
					map.get_mut().resize(group + 1);
				}
			}
			let location = self.reverse.insert_mut(entity)?;
			*location = components.insert(maps, map_idxs, group);
			self.entities[group].push(entity);
			for (entity, components) in iter {
				let location = self.reverse.insert_mut(entity)?;
				*location = components.insert(maps, map_idxs, group);
				self.entities[group].push(entity);
			}
			Ok(())
		} else {
			// Iterator passed in was empty?
			Ok(())
		}
	}

	pub fn extend_iters<C: ComponentSliceSet, EI: ExactSizeIterator<Item = EntityType>>(
		&mut self,
		entities: EI,
		component_slices: C,
	) -> Result<(), SparseTypedPagedMapErrors<EntityType>> {
		let mut cset: generic_array::GenericArray<TypeId, C::LenTN> =
			generic_array::GenericArray::from_exact_iter(C::iter_types()).unwrap();
		C::populate_type_slice(cset.as_mut_slice());
		let maps = &mut self.maps;
		let prior_group_size = self.group_sets_to_maps.len();
		if !component_slices.all_same_len(entities.len()) {
			return Err(SparseTypedPagedMapErrors::IteratorsNotAllSameLength);
		}
		let (group, map_idxs) = if let Some((group, _key, map_idxs)) =
			self.group_sets_to_maps.get_full(cset.as_slice())
		{
			(group, map_idxs)
		} else {
			self.group_sets_to_maps
				.insert(cset.to_vec(), component_slices.into_type_idx_vec(maps));
			self.entities.push(Vec::with_capacity(entities.len()));
			let group = self.group_sets_to_maps.len() - 1;
			Self::update_query_mappings(&self.group_sets_to_maps, &mut self.query_mappings, group);
			(group, self.group_sets_to_maps.get_index(group).unwrap().1)
		};
		if group >= prior_group_size {
			for map in maps.values_mut() {
				map.get_mut().resize(group + 1);
			}
		}
		let group_size = self.entities[group].len();
		let mut start_idx = component_slices.insert_all(maps, map_idxs, group);
		for entity in entities {
			match self.reverse.insert_mut(entity) {
				Ok(location) => {
					location.group = group;
					location.index = start_idx;
					start_idx += 1;
					self.entities[group].push(entity);
				}
				Err(error) => {
					// Truncate only after the error
					//C::truncate(maps, map_idxs, group, start_idx);
					// -- OR --
					// Truncate all that was passed in
					C::truncate(maps, map_idxs, group, group_size);
					self.reverse
						.remove_iter(self.entities[group].drain(group_size..));
					// Truncate choice end
					return Err(error.into());
				}
			}
		}
		Ok(())
	}

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

	pub fn query<RO: ComponentSet, RW: ComponentSet, E: ComponentSet>(
		&mut self,
	) -> Result<Query<EntityType, RO, RW, E>, SparseTypedPagedMapErrors<EntityType>> {
		let query_key = QueryTypedPagedKey::<RO, RW, E>::new();

		if let Some((idx, _key, link)) = self.query_mappings.get_full(&query_key) {
			Ok(Query::new(idx, link, &self.maps))
		} else {
			let group_size = if let Some((_tid, map)) = self.maps.get_index(0) {
				map.borrow().len_groups()
			} else {
				0
			};
			RO::ensure_exists(&mut self.maps, group_size);
			RW::ensure_exists(&mut self.maps, group_size);
			E::ensure_exists(&mut self.maps, group_size);
			let query_link = QueryTypedPagedLink {
				read_only_groups: RO::get_matching_query_groups(&self.group_sets_to_maps),
				read_write_groups: RW::get_matching_query_groups(&self.group_sets_to_maps),
				except_groups: E::get_matching_query_groups(&self.group_sets_to_maps),
				read_only_maps: RO::into_type_idx_vec(&mut self.maps),
				read_write_maps: RW::into_type_idx_vec(&mut self.maps),
			};
			let idx = self.query_mappings.len();
			self.query_mappings.insert(query_key.to_box(), query_link);
			Ok(Query::new(
				idx,
				self.query_mappings.get_index(idx).unwrap().1,
				&self.maps,
			))
		}
	}
}

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

pub trait ComponentSet: 'static + TypeList {
	#[inline]
	fn into_type_idx_vec(maps: &mut MapIndexMap) -> Vec<usize> {
		let mut idxs = Vec::with_capacity(Self::LEN);
		Self::populate_type_idx_vec(&mut idxs, maps);
		idxs
	}
	fn populate_type_idx_vec(idxs: &mut Vec<usize>, maps: &mut MapIndexMap);
	#[inline]
	fn insert(
		self,
		maps: &mut MapIndexMap,
		map_idxs: &[usize],
		group: usize,
	) -> ComponentLocations {
		self.do_insert(maps, map_idxs, group, 0, 0)
	}
	fn do_insert(
		self,
		maps: &mut MapIndexMap,
		map_idxs: &[usize],
		group: usize,
		map_idx_idx: usize,
		data_index: usize,
	) -> ComponentLocations;
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
		group: usize,
		_map_idx_idx: usize,
		data_index: usize,
	) -> ComponentLocations {
		ComponentLocations::new(group, data_index)
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
		idxs.push(entry.index());
		entry.or_insert_with(|| RefCell::new(Box::new(DensePagedDataInstance::<H>::new())));
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
	) -> ComponentLocations {
		let map_idx = map_idxs[map_idx_idx];
		let (_type_id, map) = maps
			.get_index_mut(map_idx)
			.expect("Map is in invalid state!  Shouldn't happen!");
		let data_index = map.get_mut().cast_mut::<H>().push(group, self.head);
		// let data_index = map.write()?.ca.push(group, self.head);
		self.tail
			.do_insert(maps, map_idxs, group, map_idx_idx + 1, data_index)
	}

	fn ensure_exists(maps: &mut MapIndexMap, group_size: usize) {
		let tid = std::any::TypeId::of::<H>();
		maps.entry(tid).or_insert_with(|| {
			RefCell::new(Box::new(DensePagedDataInstance::<H>::with_groups(
				group_size,
			)))
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
		idxs.push(entry.index());
		entry.or_insert_with(|| RefCell::new(Box::new(DensePagedDataInstance::<H>::new())));
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
		let start_index = map.get_mut().cast_mut::<H>().push_all(group, self.head);
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
		map.get_mut().cast_mut::<H>().truncate_group(group, size);
		T::do_truncate(maps, map_idxs, group, size, map_idxs_idx + 1);
	}
}

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

#[cfg(test)]
mod tests {
	use frunk::{hlist, Hlist};

	use super::*;

	#[test]
	fn sparse_typed_page_multimap_tests() {
		let mut map = SparseTypedPagedMap::<u64>::new();
		//assert_eq!(map.len_entities(), 0);
		assert_eq!(map.insert(1, hlist![21usize, 6.28f32, true]), Ok(()));
		// assert_eq!(map.get::<usize>(1), Ok(&21));
		// map.get_mut::<usize>(1).map(|i| *i *= 2).unwrap();
		// assert_eq!(map.get::<usize>(1), Ok(&42));
		let inserts: Vec<_> = (2..10u64)
			.map(|i| (i, hlist![21usize, 6.28f32, true]))
			.collect();

		assert_eq!(map.extend_iter(inserts), Ok(()));
		// assert_eq!(map.get::<usize>(2), Ok(&21));
		// assert_eq!(map.get::<usize>(3), Ok(&21));
		// assert_eq!(map.get::<usize>(4), Ok(&21));

		assert_eq!(
			map.extend_iters(
				vec![11u64, 12u64, 13u64].into_iter(),
				hlist!(
					vec![1usize, 2usize, 3usize].into_iter(),
					vec![1.0f32, 2f32, 3f32].into_iter(),
					vec![true, false, true].into_iter(),
				)
			),
			Ok(())
		);
		// assert_eq!(map.get::<usize>(11), Ok(&1));
		// assert_eq!(map.get::<usize>(12), Ok(&2));
		// assert_eq!(map.get::<usize>(13), Ok(&3));
		// assert_eq!(
		// 	map.get::<usize>(14),
		// 	Err(SparseTypedPagedMapErrors::SecondaryIndexError(
		// 		SecondaryIndexErrors::IndexDoesNotExist(14)
		// 	))
		// );
		// assert_eq!(map.get::<f32>(11), Ok(&1.0));
		// assert_eq!(map.get::<f32>(12), Ok(&2.0));
		// assert_eq!(map.get::<f32>(13), Ok(&3.0));
		// assert_eq!(
		// 	map.get::<f32>(14),
		// 	Err(SparseTypedPagedMapErrors::SecondaryIndexError(
		// 		SecondaryIndexErrors::IndexDoesNotExist(14)
		// 	))
		// );
		// assert_eq!(map.get::<bool>(11), Ok(&true));
		// assert_eq!(map.get::<bool>(12), Ok(&false));
		// assert_eq!(map.get::<bool>(13), Ok(&true));
		// assert_eq!(
		// 	map.get::<bool>(14),
		// 	Err(SparseTypedPagedMapErrors::SecondaryIndexError(
		// 		SecondaryIndexErrors::IndexDoesNotExist(14)
		// 	))
		// );

		assert_eq!(map.insert(22, hlist![21usize, 6.28f32, true]), Ok(()));
		assert_eq!(
			map.extend_iters(
				vec![21u64, 22u64, 23u64].into_iter(),
				hlist!(
					vec![1usize, 2usize, 3usize].into_iter(),
					vec![1.0f32, 2f32, 3f32].into_iter(),
					vec![true, false, true].into_iter(),
				)
			),
			Err(SparseTypedPagedMapErrors::SecondaryIndexError(
				SecondaryIndexErrors::IndexAlreadyExists(22)
			))
		);
		// assert_eq!(
		// 	map.get::<usize>(21),
		// 	Err(SparseTypedPagedMapErrors::SecondaryIndexError(
		// 		SecondaryIndexErrors::IndexDoesNotExist(21)
		// 	))
		// );
		// assert_eq!(map.get::<usize>(22), Ok(&21));
		// assert_eq!(
		// 	map.get::<usize>(23),
		// 	Err(SparseTypedPagedMapErrors::SecondaryIndexError(
		// 		SecondaryIndexErrors::IndexDoesNotExist(23)
		// 	))
		// );
		assert_eq!(
			map.extend_iters(
				vec![22u64, 23u64].into_iter(),
				hlist!(
					vec![2usize, 3usize].into_iter(),
					vec![2f32, 3f32].into_iter(),
					vec![false, true].into_iter(),
				)
			),
			Err(SparseTypedPagedMapErrors::SecondaryIndexError(
				SecondaryIndexErrors::IndexAlreadyExists(22)
			))
		);
		// assert_eq!(map.get::<usize>(22), Ok(&21));
		// assert_eq!(
		// 	map.get::<usize>(23),
		// 	Err(SparseTypedPagedMapErrors::SecondaryIndexError(
		// 		SecondaryIndexErrors::IndexDoesNotExist(23)
		// 	))
		// );
	}

	#[test]
	fn empty_entities() {
		let mut map = SparseTypedPagedMap::<u64>::new();
		assert_eq!(map.insert(1, hlist![]), Ok(()));
		assert_eq!(map.contains(1), true);
	}

	#[test]
	fn queries() {
		let mut map = SparseTypedPagedMap::<u64>::new();
		map.extend_iter((1..=2).map(|e| (e, hlist![e as usize, format!("test: {}", e)])))
			.unwrap();
		// let entries = map.maps.as_entries_mut();
		// let entries = Entries::as_entries_mut(&mut map.maps);
		// dbg!(&entries);
		// let blah0 = map.maps[0].cast_mut::<u64>();
		// let blah1 = map.maps[1].cast_mut::<u64>();
		// dbg!(&blah0.data);
		// dbg!(&blah1.data);

		{
			let query = map.query::<Hlist![], Hlist![], Hlist![]>().unwrap();
			//for () in query.iter() {}
		}

		{
			let query = map.query::<Hlist![usize], Hlist![], Hlist![]>();
			//for () in query.iter() {}
		}
	}
}
