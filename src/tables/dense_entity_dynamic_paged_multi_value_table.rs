use crate::database::{DatabaseId, TableId};
use crate::entity::Entity;
use crate::table::{Table, TableBuilder, TableCastable};
use crate::tables::{EntityTable, ValidEntity};
use crate::utils::secondary_entity_index::{SecondaryEntityIndex, SecondaryEntityIndexErrors};
use crate::utils::unique_hasher::UniqueHasherBuilder;
use arrayvec::ArrayVec;
use indexmap::map::IndexMap;
use owning_ref::OwningHandle;
use smallvec::SmallVec;
use smol_str::SmolStr;
use std::any::{Any, TypeId};
use std::cell::{BorrowMutError, Ref, RefCell, RefMut};
use std::marker::PhantomData;
use std::rc::{Rc, Weak};

#[derive(Debug)]
pub enum DenseEntityDynamicPagedMultiValueTableErrors<EntityType: Entity> {
	SecondaryIndexError(SecondaryEntityIndexErrors<EntityType>),
	BorrowMutError(BorrowMutError),
	StorageDoesNotExistInGroup(usize, TypeId),
	StorageAlreadyExistsInGroup(usize, TypeId),
	EntityAlreadyExistsInStorage,
	ComponentStorageDoesNotExist(&'static str),
	EntityDoesNotExistInStorage(EntityType, &'static str),
	EntityGenerationMismatch(EntityType, EntityType),
	IteratorsNotAllSameLength,
}

impl<EntityType: Entity> std::error::Error
	for DenseEntityDynamicPagedMultiValueTableErrors<EntityType>
{
	fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
		use DenseEntityDynamicPagedMultiValueTableErrors::*;
		match self {
			SecondaryIndexError(source) => Some(source),
			BorrowMutError(source) => Some(source),
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

impl<EntityType: Entity> std::fmt::Display
	for DenseEntityDynamicPagedMultiValueTableErrors<EntityType>
{
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
		use DenseEntityDynamicPagedMultiValueTableErrors::*;
		match self {
			SecondaryIndexError(_source) => write!(f, "SecondaryIndexError"),
			BorrowMutError(_source) => write!(f, "already borrowed"),
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

impl<EntityType: Entity> From<SecondaryEntityIndexErrors<EntityType>>
	for DenseEntityDynamicPagedMultiValueTableErrors<EntityType>
{
	fn from(source: SecondaryEntityIndexErrors<EntityType>) -> Self {
		DenseEntityDynamicPagedMultiValueTableErrors::SecondaryIndexError(source)
	}
}

impl<EntityType: Entity> From<BorrowMutError>
	for DenseEntityDynamicPagedMultiValueTableErrors<EntityType>
{
	fn from(source: BorrowMutError) -> Self {
		DenseEntityDynamicPagedMultiValueTableErrors::BorrowMutError(source)
	}
}

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
}

pub trait DynDensePagedData {
	fn get_type_id(&self) -> TypeId;
	fn as_any(&self) -> &dyn std::any::Any;
	fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
	fn get_strong(&self) -> Rc<RefCell<dyn DynDensePagedData>>;
	fn get_idx(&self) -> usize;
	fn ensure_group_count(&mut self, group_count: usize);
	fn swap_remove(&mut self, group: usize, index: usize);
	fn move_groups(&mut self, group: usize, index: usize, new_group: usize);
}

trait DynDensePagedDataCastable: 'static {
	fn get_strong_self(&self) -> Rc<RefCell<Self>>;
}

pub struct DensePagedData<ValueType: 'static> {
	this: Weak<RefCell<Self>>,
	idx: usize,
	data: Vec<Vec<ValueType>>,
}

impl<ValueType: 'static> DensePagedData<ValueType> {
	pub fn new(idx: usize) -> Rc<RefCell<Self>> {
		let this = Rc::new(RefCell::new(DensePagedData {
			this: Weak::new(),
			idx,
			data: vec![],
		}));
		this.borrow_mut().this = Rc::downgrade(&this);
		this
	}

	pub fn push(&mut self, group: usize, data: ValueType) {
		self.data[group].push(data);
	}

	pub fn extend(&mut self, group: usize, data: impl IntoIterator<Item = ValueType>) {
		self.data[group].extend(data);
	}
}

impl<ValueType: 'static> DynDensePagedData for DensePagedData<ValueType> {
	fn get_type_id(&self) -> TypeId {
		TypeId::of::<ValueType>()
	}

	fn as_any(&self) -> &dyn Any {
		self
	}

	fn as_any_mut(&mut self) -> &mut dyn Any {
		self
	}

	fn get_strong(&self) -> Rc<RefCell<dyn DynDensePagedData>> {
		self.get_strong_self()
	}

	fn get_idx(&self) -> usize {
		self.idx
	}

	fn ensure_group_count(&mut self, group_count: usize) {
		self.data.resize_with(group_count, || Vec::new());
	}

	fn swap_remove(&mut self, group: usize, index: usize) {
		self.data[group].swap_remove(index);
	}

	fn move_groups(&mut self, group: usize, index: usize, new_group: usize) {
		let value = self.data[group].swap_remove(index);
		self.data[new_group].push(value);
	}
}

impl<ValueType: 'static> DynDensePagedDataCastable for DensePagedData<ValueType> {
	fn get_strong_self(&self) -> Rc<RefCell<Self>> {
		self.this.upgrade().unwrap() // It's obviously valid since it's obviously self
	}
}

trait DynGroup {
	fn as_any(&self) -> &dyn std::any::Any;
	fn get_idx(&self) -> usize;
}

pub struct GroupQuery<EntityType: Entity, VTs: ValueTypes> {
	group: usize,
	storage: VTs::Storage,
	_phantom: PhantomData<EntityType>,
}

impl<EntityType: Entity, VTs: ValueTypes> Clone for GroupQuery<EntityType, VTs> {
	fn clone(&self) -> Self {
		GroupQuery {
			group: self.group,
			storage: self.storage.clone(),
			_phantom: PhantomData,
		}
	}
}

pub struct GroupInsert<EntityType: Entity, VTs: InsertValueTypes> {
	group: usize,
	storage: VTs::Storage,
	storage_idxs: Box<[usize]>,
	_phantom: PhantomData<EntityType>,
}

impl<EntityType: Entity, VTs: InsertValueTypes> Clone for GroupInsert<EntityType, VTs> {
	fn clone(&self) -> Self {
		GroupInsert {
			group: self.group,
			storage: self.storage.clone(),
			storage_idxs: self.storage_idxs.clone(),
			_phantom: PhantomData,
		}
	}
}

impl<EntityType: Entity, VTs: ValueTypes> GroupQuery<EntityType, VTs> {
	pub fn try_lock<'a, 't>(
		&'a mut self,
		table: &'t DenseEntityDynamicPagedMultiValueTable<EntityType>,
	) -> Option<GroupQueryLock<'a, 't, EntityType, VTs>> {
		if let Ok(storage_locked) = VTs::try_storage_locked(&self.storage) {
			Some(GroupQueryLock {
				group: self.group,
				storage_locked,
				table,
				_phantom: PhantomData,
			})
		} else {
			None
		}
	}

	pub fn lock<'a, 't>(
		&'a mut self,
		table: &'t DenseEntityDynamicPagedMultiValueTable<EntityType>,
	) -> GroupQueryLock<'a, 't, EntityType, VTs> {
		self.try_lock(table).expect("unable to lock GroupQuery")
	}
}

impl<EntityType: Entity, VTs: InsertValueTypes> GroupInsert<EntityType, VTs> {
	pub fn try_lock<'a, 's>(
		&'a mut self,
		table: &'s mut DenseEntityDynamicPagedMultiValueTable<EntityType>,
	) -> Option<GroupInsertLock<'a, 's, EntityType, VTs>> {
		if let Ok(storage_locked) = VTs::try_storage_locked(&self.storage) {
			Some(GroupInsertLock {
				group: self.group,
				storage_locked,
				table,
				_phantom: PhantomData,
			})
		} else {
			None
		}
	}

	pub fn lock<'a, 's>(
		&'a mut self,
		table: &'s mut DenseEntityDynamicPagedMultiValueTable<EntityType>,
	) -> GroupInsertLock<'a, 's, EntityType, VTs> {
		self.try_lock(table).expect("unable to lock GroupInsert")
	}
}

pub struct GroupQueryLock<'a, 's, EntityType: Entity, VTs: ValueTypes> {
	group: usize,
	storage_locked: VTs::StorageLocked, // When GAT's exist then pass `'a` into StorageLocked
	table: &'s DenseEntityDynamicPagedMultiValueTable<EntityType>,
	_phantom: PhantomData<&'a EntityType>,
}

pub struct GroupInsertLock<'a, 's, EntityType: Entity, VTs: InsertValueTypes> {
	group: usize,
	storage_locked: VTs::StorageLocked, // When GAT's exist then pass `'a` into StorageLocked
	table: &'s mut DenseEntityDynamicPagedMultiValueTable<EntityType>,
	_phantom: PhantomData<&'a ()>,
}

impl<'a, 's, EntityType: Entity, VTs: ValueTypes> GroupQueryLock<'a, 's, EntityType, VTs> {
	pub fn get_all(&'a mut self, entity: ValidEntity<EntityType>) -> Option<VTs::GetRef>
	where
		VTs: GetValueTypes<'a>,
	{
		if let Ok(location) =
			DenseEntityDynamicPagedMultiValueTable::<EntityType>::get_valid_location(
				&self.table.reverse,
				&self.table.entities,
				entity.raw(),
			) {
			let mut cast_storages = VTs::cast_locked_storages::<VTs>(&mut self.storage_locked);
			VTs::get::<EntityType>(
				// TODO:  LACK OF GAT's IS SO PAINFUL!  FIX THIS WHEN GAT's EXIST!
				// This 'should' be safeish as it's just casting lifetimes to a more constrained lifetime
				unsafe { &mut *(&mut cast_storages as *mut VTs::StoragesLockedRef) },
				location.group,
				location.index,
			)
		} else {
			None
		}
	}

	pub fn get<GTs: GetValueTypes<'a>>(
		&'a mut self,
		entity: ValidEntity<EntityType>,
	) -> Option<GTs::GetRef> {
		if let Ok(location) =
			DenseEntityDynamicPagedMultiValueTable::<EntityType>::get_valid_location(
				&self.table.reverse,
				&self.table.entities,
				entity.raw(),
			) {
			let mut cast_storages = GTs::cast_locked_storages::<VTs>(&mut self.storage_locked);
			GTs::get::<EntityType>(
				// TODO:  LACK OF GAT's IS SO PAINFUL!  FIX THIS WHEN GAT's EXIST!
				// This 'should' be safeish as it's just casting lifetimes to a more constrained lifetime
				unsafe { &mut *(&mut cast_storages as *mut GTs::StoragesLockedRef) },
				location.group,
				location.index,
			)
		} else {
			None
		}
	}
}

impl<'g, 's, EntityType: Entity, VTs: InsertValueTypes> GroupInsertLock<'g, 's, EntityType, VTs> {
	pub fn insert(
		&mut self,
		entity: ValidEntity<EntityType>,
		data: VTs::MoveData,
	) -> Result<(), DenseEntityDynamicPagedMultiValueTableErrors<EntityType>> {
		let location =
			DenseEntityDynamicPagedMultiValueTable::<EntityType>::insert_valid_location_mut(
				&mut self.table.reverse,
				&mut self.table.entities,
				entity.raw(),
				self.group,
			)?;
		VTs::push(&mut self.storage_locked, location.group, data);
		Ok(())
	}

	pub fn extend_slices(
		&mut self,
		entity_slice: &[ValidEntity<EntityType>],
		data: VTs::MoveDataVec,
	) -> Result<(), DenseEntityDynamicPagedMultiValueTableErrors<EntityType>> {
		if !VTs::ensure_vec_length(&data, entity_slice.len()) {
			panic!(
				"All vecs passed to DenseEntityDynamicPagedMultiValueTable must be the same length"
			);
		}
		VTs::extend(&mut self.storage_locked, self.group, data);
		for entity in entity_slice {
			DenseEntityDynamicPagedMultiValueTable::<EntityType>::insert_valid_location_mut(
					&mut self.table.reverse,
					&mut self.table.entities,
					entity.raw(),
					self.group,
				).expect("Entity Already exists, when extending a DenseEntityDynamicPagedMultiValueTable then all entities must be new to it, else use `transform`");
		}

		Ok(())
	}
}

impl<EntityType: Entity, VTs: ValueTypes> DynGroup for GroupQuery<EntityType, VTs> {
	fn as_any(&self) -> &dyn Any {
		self
	}

	fn get_idx(&self) -> usize {
		self.group
	}
}

impl<EntityType: Entity, VTs: InsertValueTypes> DynGroup for GroupInsert<EntityType, VTs> {
	fn as_any(&self) -> &dyn Any {
		self
	}

	fn get_idx(&self) -> usize {
		self.group
	}
}

#[derive(PartialEq, Eq, Hash)]
struct QueryTypedPagedKey<'a> {
	include: &'a [TypeId],
	//exclude: &'a [TypeId],
}

#[derive(PartialEq, Eq, Hash)]
struct QueryTypedPagedKeyBoxed {
	include: Box<[TypeId]>,
	//exclude: Box<[TypeId]>,
	include_storage_idxs: Box<[usize]>,
}

impl<'a> QueryTypedPagedKey<'a> {
	fn to_box(
		self,
		storages: &IndexMap<TypeId, Rc<RefCell<dyn DynDensePagedData>>, UniqueHasherBuilder>,
	) -> QueryTypedPagedKeyBoxed {
		QueryTypedPagedKeyBoxed {
			include: self.include.into(),
			//exclude: self.exclude.into(),
			include_storage_idxs: self
				.include
				.iter()
				.map(|tid| storages.get_full(tid).unwrap().0)
				.collect(),
		}
	}

	fn to_box_from_locked(self, storages: &AllLockedStorages) -> QueryTypedPagedKeyBoxed {
		QueryTypedPagedKeyBoxed {
			include: self.include.into(),
			//exclude: self.exclude.into(),
			include_storage_idxs: self
				.include
				.iter()
				.map(|&tid| {
					storages
						.iter()
						.position(|s| s.get_type_id() == tid)
						.unwrap()
				})
				.collect(),
		}
	}
}

// impl<'a> Hash for QueryTypedPagedKey<'a> {
// 	fn hash<H: Hasher>(&self, state: &mut H) {
// 		self.include.hash(state);
// 		self.exclude.hash(state);
// 	}
// }
//
// impl Hash for QueryTypedPagedKeyBoxed {
// 	fn hash<H: Hasher>(&self, state: &mut H) {
// 		self.include.as_ref().hash(state);
// 		self.exclude.as_ref().hash(state);
// 	}
// }

impl<'a> indexmap::Equivalent<QueryTypedPagedKeyBoxed> for QueryTypedPagedKey<'a> {
	fn equivalent(&self, key: &QueryTypedPagedKeyBoxed) -> bool {
		&*key.include == self.include // && &*key.exclude == self.exclude
	}
}

pub struct DenseEntityDynamicPagedMultiValueTable<EntityType: Entity> {
	this: Weak<RefCell<Self>>,
	database_id: DatabaseId,
	table_name: SmolStr,
	table_id: TableId,
	//entity_table: EntityTable<EntityType>,
	reverse: SecondaryEntityIndex<EntityType, ComponentLocations>,
	entities: Vec<Vec<EntityType>>,
	storages: IndexMap<TypeId, Rc<RefCell<dyn DynDensePagedData>>, UniqueHasherBuilder>,
	group_inserts: IndexMap<QueryTypedPagedKeyBoxed, Option<Box<dyn DynGroup>>>,
	group_queries: IndexMap<TypeId, Box<dyn DynGroup>, UniqueHasherBuilder>,
}

impl<EntityType: Entity> DenseEntityDynamicPagedMultiValueTable<EntityType> {
	fn insert_valid_location_mut<'a>(
		reverse: &'a mut SecondaryEntityIndex<EntityType, ComponentLocations>,
		entities: &mut Vec<Vec<EntityType>>,
		entity: EntityType,
		group: usize,
	) -> Result<&'a mut ComponentLocations, DenseEntityDynamicPagedMultiValueTableErrors<EntityType>>
	{
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
		reverse: &'a SecondaryEntityIndex<EntityType, ComponentLocations>,
		entities: &Vec<Vec<EntityType>>,
		entity: EntityType,
	) -> Result<&'a ComponentLocations, DenseEntityDynamicPagedMultiValueTableErrors<EntityType>> {
		let location = reverse.get(entity)?;
		if entities[location.group][location.index] != entity {
			return Err(
				DenseEntityDynamicPagedMultiValueTableErrors::EntityGenerationMismatch(
					entity,
					entities[location.group][location.index],
				),
			);
		}
		Ok(location)
	}

	fn get_valid_location_mut<'a>(
		reverse: &'a mut SecondaryEntityIndex<EntityType, ComponentLocations>,
		entities: &Vec<Vec<EntityType>>,
		entity: EntityType,
	) -> Result<&'a mut ComponentLocations, DenseEntityDynamicPagedMultiValueTableErrors<EntityType>>
	{
		let location = reverse.get_mut(entity)?;
		if entities[location.group][location.index] != entity {
			return Err(
				DenseEntityDynamicPagedMultiValueTableErrors::EntityGenerationMismatch(
					entity,
					entities[location.group][location.index],
				),
			);
		}
		Ok(location)
	}

	fn remove_valid_location(
		reverse: &mut SecondaryEntityIndex<EntityType, ComponentLocations>,
		entities: &mut Vec<Vec<EntityType>>,
		entity: EntityType,
	) -> Result<ComponentLocations, DenseEntityDynamicPagedMultiValueTableErrors<EntityType>> {
		let location = reverse.get_mut(entity)?;
		let entities_group = &mut entities[location.group];
		if entities_group[location.index] != entity {
			return Err(
				DenseEntityDynamicPagedMultiValueTableErrors::EntityGenerationMismatch(
					entity,
					entities[location.group][location.index],
				),
			);
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

	fn ensure_group_count_on_storages(&mut self) {
		let groups = self.group_inserts.len();
		self.entities.resize(groups, Vec::new());
		for storage in self.storages.values() {
			storage.borrow_mut().ensure_group_count(groups);
		}
	}

	pub fn builder(
		entity_table: Rc<RefCell<EntityTable<EntityType>>>,
	) -> DenseEntityPagedMultiValueTableBuilder<EntityType> {
		DenseEntityPagedMultiValueTableBuilder {
			entity_table,
			capacity: 0,
		}
	}

	pub fn builder_with_capacity(
		entity_table: Rc<RefCell<EntityTable<EntityType>>>,
		capacity: usize,
	) -> DenseEntityPagedMultiValueTableBuilder<EntityType> {
		DenseEntityPagedMultiValueTableBuilder {
			entity_table,
			capacity,
		}
	}

	pub fn group_query<VTs: ValueTypes>(
		&mut self,
	) -> Result<GroupQuery<EntityType, VTs>, DenseEntityDynamicPagedMultiValueTableErrors<EntityType>>
	{
		let group = if let Some(group) = self.group_queries.get(&TypeId::of::<VTs::Raw>()) {
			group
				.as_any()
				.downcast_ref::<GroupQuery<EntityType, VTs>>()
				.expect("failed to cast type to itself")
				.clone()
		} else {
			let group = GroupQuery::<EntityType, VTs> {
				group: self.group_queries.len(),
				storage: VTs::get_or_create_storage(&mut self.storages),
				_phantom: PhantomData,
			};
			self.group_queries
				.insert(TypeId::of::<VTs::Raw>(), Box::new(group.clone()));
			self.ensure_group_count_on_storages();
			group
		};
		Ok(group)
	}

	pub fn group_insert<VTs: InsertValueTypes>(
		&mut self,
	) -> Result<
		GroupInsert<EntityType, VTs>,
		DenseEntityDynamicPagedMultiValueTableErrors<EntityType>,
	> {
		let include_tids = VTs::get_include_type_ids();
		let exclude_tids = VTs::get_exclude_type_ids();
		let key = QueryTypedPagedKey {
			include: include_tids.as_slice(),
			//exclude: exclude_tids.as_slice(),
		};
		let group = if let Some((idx, _key, group_page)) = self.group_inserts.get_full_mut(&key) {
			if let Some(group_page) = group_page {
				group_page
					.as_any()
					.downcast_ref::<GroupInsert<EntityType, VTs>>()
					.expect("failed to cast to self type")
					.clone()
			} else {
				// This can be hit when adding/removing components, it will create a new group but
				//// typeless at that point in time, we now have the types so we now create it.
				let group = GroupInsert::<EntityType, VTs> {
					group: idx,
					storage: VTs::get_or_create_storage(&mut self.storages),
					storage_idxs: VTs::get_storage_idxs(&self.storages, Vec::new())
						.into_boxed_slice(),
					_phantom: PhantomData,
				};
				*group_page = Some(Box::new(group.clone()));
				group
			}
		} else {
			let group = GroupInsert::<EntityType, VTs> {
				group: self.group_inserts.len(),
				storage: VTs::get_or_create_storage(&mut self.storages),
				storage_idxs: VTs::get_storage_idxs(&self.storages, Vec::new()).into_boxed_slice(),
				_phantom: PhantomData,
			};
			self.group_inserts
				.insert(key.to_box(&self.storages), Some(Box::new(group.clone())));
			self.ensure_group_count_on_storages();
			group
		};
		Ok(group)
	}

	pub fn delete(
		&mut self,
		entity: ValidEntity<EntityType>,
	) -> Result<(), DenseEntityDynamicPagedMultiValueTableErrors<EntityType>> {
		let location =
			Self::remove_valid_location(&mut self.reverse, &mut self.entities, entity.raw())?;
		let storage_idxs = &self
			.group_inserts
			.get_index(location.group)
			.unwrap()
			.0
			.include_storage_idxs;
		for idx in storage_idxs.iter().copied() {
			self.storages[idx]
				.borrow_mut()
				.swap_remove(location.group, location.index);
		}

		Ok(())
	}

	pub fn lock(
		&mut self,
	) -> Result<AllLock<EntityType>, DenseEntityDynamicPagedMultiValueTableErrors<EntityType>> {
		let mut storages = SmallVec::with_capacity(self.storages.len());
		for storage in self.storages.values_mut() {
			storages.push(OwningHandle::new_with_fn(storage.clone(), |storage| {
				// This `unsafe` is required because OwningHandle doesn't handle dyn traits on an inner type as it requires Sized needlessly
				unsafe { RefCell::borrow_mut(&*storage) }
			}));
		}
		Ok(AllLock {
			reverse: &mut self.reverse,
			entities: &mut self.entities,
			group_inserts: &mut self.group_inserts,
			storages,
		})
	}
}

// If this is worth increasing then please request with a reason
type AllLockedStorages<'a> = SmallVec<
	[OwningHandle<Rc<RefCell<dyn DynDensePagedData>>, RefMut<'a, dyn DynDensePagedData + 'static>>;
		32],
>;

pub struct AllLock<'a, EntityType: Entity> {
	reverse: &'a mut SecondaryEntityIndex<EntityType, ComponentLocations>,
	entities: &'a mut Vec<Vec<EntityType>>,
	group_inserts: &'a mut IndexMap<QueryTypedPagedKeyBoxed, Option<Box<dyn DynGroup>>>,
	storages: AllLockedStorages<'a>,
}

impl<'a, EntityType: Entity> AllLock<'a, EntityType> {
	pub fn delete(
		&mut self,
		entity: ValidEntity<EntityType>,
	) -> Result<(), DenseEntityDynamicPagedMultiValueTableErrors<EntityType>> {
		let location = DenseEntityDynamicPagedMultiValueTable::remove_valid_location(
			self.reverse,
			self.entities,
			entity.raw(),
		)?;
		let storage_idxs = &self
			.group_inserts
			.get_index(location.group)
			.unwrap()
			.0
			.include_storage_idxs;
		for idx in storage_idxs.iter().copied() {
			self.storages[idx].swap_remove(location.group, location.index);
		}

		Ok(())
	}

	fn ensure_group_count_on_storages(
		group_inserts: &mut IndexMap<QueryTypedPagedKeyBoxed, Option<Box<dyn DynGroup>>>,
		entities: &mut Vec<Vec<EntityType>>,
		storages: &mut AllLockedStorages,
	) {
		let groups = group_inserts.len();
		entities.resize(groups, Vec::new());
		for storage in storages.iter_mut() {
			storage.ensure_group_count(groups);
		}
	}

	pub fn transform<Remove: RemoveTypes, Add: InsertValueTypes>(
		&mut self,
		entity: ValidEntity<EntityType>,
		inserter: &GroupInsert<EntityType, Add>, // Not actually used, but its existence means the type storages exist
		add: Add::MoveData,
	) -> Result<(), DenseEntityDynamicPagedMultiValueTableErrors<EntityType>> {
		let location = DenseEntityDynamicPagedMultiValueTable::get_valid_location_mut(
			self.reverse,
			self.entities,
			entity.raw(),
		)?;
		let (group_key, _group_value) = self.group_inserts.get_index(location.group).unwrap();
		let mut moving = ArrayVec::<[(TypeId, usize); 32]>::new();
		moving.extend(
			group_key
				.include
				.iter()
				.copied()
				.zip(group_key.include_storage_idxs.iter().copied()),
		);
		Remove::swap_remove_type_ids(&mut moving);
		Add::swap_remove_type_ids(&mut moving);

		// First remove the ones being perma-removed...
		let mut removing = TypeIdCacheVec::new();
		Remove::push_type_ids(&mut removing);
		let storages = &mut self.storages;
		group_key
			.include
			.iter()
			.copied()
			.zip(group_key.include_storage_idxs.iter().copied())
			.filter(|(tid, _idx)| removing.iter().any(|t| t == tid))
			.for_each(|(_tid, idx)| storages[idx].swap_remove(location.group, location.index));

		// Then figure out where to move/add to...
		let mut new_include = TypeIdCacheVec::new();
		new_include.extend(moving.iter().map(|(tid, _idx)| *tid));
		Add::push_type_ids(&mut new_include);
		new_include.sort();
		let key = QueryTypedPagedKey {
			include: new_include.as_slice(),
		};
		let new_group_idx = if let Some((group_idx, _group_key, _group_value)) =
			self.group_inserts.get_full(&key)
		{
			group_idx
		} else {
			self.group_inserts
				.insert(key.to_box_from_locked(&self.storages), None);
			Self::ensure_group_count_on_storages(
				&mut self.group_inserts,
				&mut self.entities,
				&mut self.storages,
			);
			self.group_inserts.len() - 1
		};

		// Then add the new ones to the new location
		Add::push_prelocked(
			&mut self.storages,
			&inserter.storage_idxs,
			new_group_idx,
			add,
		);

		// And move over all other components
		for (_tid, idx) in moving {
			self.storages[idx].move_groups(location.group, location.index, new_group_idx);
		}

		// And move the entity itself in the index
		let old_location = *location;
		self.entities[old_location.group].swap_remove(old_location.index);
		self.entities[new_group_idx].push(entity.raw());
		location.group = new_group_idx;
		location.index = self.entities[new_group_idx].len() - 1;
		// While also fixing the moved entity that took its old place if it exists
		let old_entity_group = &mut self.entities[old_location.group];
		if old_location.index < old_entity_group.len() {
			let moved_entity = old_entity_group[old_location.index];
			let location = self
				.reverse
				.get_mut(moved_entity)
				.expect("This should always exist as it was just got from the entity array");
			location.index = old_location.index;
		}
		Ok(())
	}
}

pub trait RemoveTypes: 'static {
	fn push_type_ids(arr: &mut TypeIdCacheVec);
	fn swap_remove_type_ids(arr: &mut ArrayVec<[(TypeId, usize); 32]>);
}

impl RemoveTypes for () {
	#[inline]
	fn push_type_ids(_arr: &mut TypeIdCacheVec) {}
	#[inline]
	fn swap_remove_type_ids(_arr: &mut ArrayVec<[(TypeId, usize); 32]>) {}
}

impl<HEAD: 'static, TAIL: RemoveTypes> RemoveTypes for (HEAD, TAIL) {
	#[inline]
	fn push_type_ids(arr: &mut TypeIdCacheVec) {
		arr.push(TypeId::of::<HEAD>());
		TAIL::push_type_ids(arr);
	}
	#[inline]
	fn swap_remove_type_ids(arr: &mut ArrayVec<[(TypeId, usize); 32]>) {
		if let Some(found_idx) = arr
			.iter()
			.position(|(tid, _idx)| *tid == TypeId::of::<HEAD>())
		{
			arr.swap_remove(found_idx);
		}
		TAIL::swap_remove_type_ids(arr);
	}
}

pub trait ValueTypes: 'static {
	type Raw: 'static;
	type SelfRaw: 'static;
	type Storage: 'static + Clone;
	type StorageLocked: 'static;
	type SingleStorageLocked: 'static;
	fn push_type_ids(arr: &mut TypeIdCacheVec);
	fn swap_remove_type_ids(arr: &mut ArrayVec<[(TypeId, usize); 32]>);
	fn get_storage_idxs(
		storages: &IndexMap<TypeId, Rc<RefCell<dyn DynDensePagedData>>, UniqueHasherBuilder>,
		vec: Vec<usize>,
	) -> Vec<usize>;
	fn get_or_create_storage(
		storages: &mut IndexMap<TypeId, Rc<RefCell<dyn DynDensePagedData>>, UniqueHasherBuilder>,
	) -> Self::Storage;
	fn try_storage_locked(storage: &Self::Storage) -> Result<Self::StorageLocked, ()>;
	fn get_locked_storage_ref<'s, TT: ValueTypes>(
		storages: &Self::StorageLocked,
	) -> &'s TT::SingleStorageLocked;
	fn get_locked_storage_ref_mut<'s, TT: ValueTypes>(
		storages: &mut Self::StorageLocked,
	) -> &'s mut TT::SingleStorageLocked;
}

// Ask if this should be increased in size, but honestly, more tables should probably be used instead
type TypeIdCacheVec = ArrayVec<[TypeId; 32]>;

pub trait InsertValueTypes: ValueTypes {
	fn fill_include_type_ids(arr: &mut TypeIdCacheVec);
	fn fill_exclude_type_ids(arr: &mut TypeIdCacheVec);
	#[inline(always)]
	fn get_include_type_ids() -> TypeIdCacheVec {
		let mut vec = TypeIdCacheVec::new();
		Self::fill_include_type_ids(&mut vec);
		vec
	}
	#[inline(always)]
	fn get_exclude_type_ids() -> TypeIdCacheVec {
		let mut vec = TypeIdCacheVec::new();
		Self::fill_include_type_ids(&mut vec);
		vec
	}
	type MoveData: 'static;
	type MoveDataVec: 'static;
	fn push(storage_locked: &mut Self::StorageLocked, group: usize, data: Self::MoveData);
	fn push_prelocked(
		storage_locked: &mut AllLockedStorages,
		idxs: &[usize],
		group: usize,
		data: Self::MoveData,
	);
	fn ensure_vec_length(data: &Self::MoveDataVec, len: usize) -> bool;
	fn extend(storage_locked: &mut Self::StorageLocked, group: usize, data: Self::MoveDataVec);
}

impl ValueTypes for () {
	type Raw = ();
	type SelfRaw = ();
	type Storage = ();
	type StorageLocked = ();
	type SingleStorageLocked = ();

	#[inline]
	fn push_type_ids(_arr: &mut TypeIdCacheVec) {}
	#[inline]
	fn swap_remove_type_ids(_arr: &mut ArrayVec<[(TypeId, usize); 32]>) {}

	#[inline]
	fn get_storage_idxs(
		_storages: &IndexMap<TypeId, Rc<RefCell<dyn DynDensePagedData>>, UniqueHasherBuilder>,
		vec: Vec<usize>,
	) -> Vec<usize> {
		vec
	}

	#[inline]
	fn get_or_create_storage(
		_storages: &mut IndexMap<TypeId, Rc<RefCell<dyn DynDensePagedData>>, UniqueHasherBuilder>,
	) -> Self::Storage {
	}

	#[inline]
	fn try_storage_locked(_storage: &Self::Storage) -> Result<Self::StorageLocked, ()> {
		Ok(())
	}

	#[inline]
	fn get_locked_storage_ref<'s, TT: ValueTypes>(
		_storages: &Self::StorageLocked,
	) -> &'s TT::SingleStorageLocked {
		panic!(
			"requested a component type that does not exist in this storage: {}",
			std::any::type_name::<TT::SelfRaw>()
		)
	}

	fn get_locked_storage_ref_mut<'s, TT: ValueTypes>(
		_storages: &mut Self::StorageLocked,
	) -> &'s mut TT::SingleStorageLocked {
		panic!(
			"requested a component type that does not exist in this storage: {}",
			std::any::type_name::<TT::SelfRaw>()
		)
	}
}

impl InsertValueTypes for () {
	#[inline(always)]
	fn fill_include_type_ids(_arr: &mut TypeIdCacheVec) {}

	#[inline(always)]
	fn fill_exclude_type_ids(_arr: &mut TypeIdCacheVec) {}

	type MoveData = ();
	type MoveDataVec = ();

	#[inline]
	fn push(_storage_locked: &mut Self::StorageLocked, _group: usize, _data: Self::MoveData) {}

	#[inline]
	fn push_prelocked(
		_storage_locked: &mut AllLockedStorages,
		_idxs: &[usize],
		_group: usize,
		_data: Self::MoveData,
	) {
	}

	#[inline]
	fn ensure_vec_length(_data: &Self::MoveDataVec, _len: usize) -> bool {
		true
	}

	#[inline]
	fn extend(_storage_locked: &mut Self::StorageLocked, _group: usize, _data: Self::MoveDataVec) {}
}

pub enum CannotMoveGroupWithImmutableType {}

impl<HEAD: 'static, TAIL: ValueTypes> ValueTypes for (&'static HEAD, TAIL) {
	type Raw = (HEAD, TAIL::Raw);
	type SelfRaw = &'static HEAD;
	type Storage = (Rc<RefCell<DensePagedData<HEAD>>>, TAIL::Storage);
	type StorageLocked = (Self::SingleStorageLocked, TAIL::StorageLocked);
	type SingleStorageLocked =
		OwningHandle<Rc<RefCell<DensePagedData<HEAD>>>, Ref<'static, DensePagedData<HEAD>>>;

	#[inline]
	fn push_type_ids(arr: &mut TypeIdCacheVec) {
		arr.push(TypeId::of::<HEAD>());
		TAIL::push_type_ids(arr);
	}

	#[inline]
	fn swap_remove_type_ids(arr: &mut ArrayVec<[(TypeId, usize); 32]>) {
		if let Some(found_idx) = arr
			.iter()
			.position(|(tid, _idx)| *tid == TypeId::of::<HEAD>())
		{
			arr.swap_remove(found_idx);
		}
		TAIL::swap_remove_type_ids(arr);
	}

	#[inline]
	fn get_storage_idxs(
		storages: &IndexMap<TypeId, Rc<RefCell<dyn DynDensePagedData>>, UniqueHasherBuilder>,
		mut vec: Vec<usize>,
	) -> Vec<usize> {
		let idx = storages.get_full(&TypeId::of::<HEAD>()).unwrap().0;
		vec.push(idx);
		TAIL::get_storage_idxs(storages, vec)
	}

	#[inline]
	fn get_or_create_storage(
		storages: &mut IndexMap<TypeId, Rc<RefCell<dyn DynDensePagedData>>, UniqueHasherBuilder>,
	) -> Self::Storage {
		let storage = if let Some(storage) = storages.get(&TypeId::of::<HEAD>()) {
			storage
				.borrow()
				.as_any()
				.downcast_ref::<DensePagedData<HEAD>>()
				.expect("Failed to cast type to itself?")
				.get_strong_self()
		} else {
			let storage = DensePagedData::<HEAD>::new(storages.len());
			storages.insert(TypeId::of::<HEAD>(), storage.clone());
			storage
		};
		(storage, TAIL::get_or_create_storage(storages))
	}

	#[inline]
	fn try_storage_locked(storage: &Self::Storage) -> Result<Self::StorageLocked, ()> {
		Ok((
			OwningHandle::new(storage.0.clone()),
			TAIL::try_storage_locked(&storage.1)?,
		))
	}

	#[inline]
	fn get_locked_storage_ref<'s, TT: ValueTypes>(
		storages: &Self::StorageLocked,
	) -> &'s TT::SingleStorageLocked {
		if TypeId::of::<TT::SelfRaw>() == TypeId::of::<&'static HEAD>() {
			// TODO:  Lack of GATs sucks...  This unsafe can be removed once they exist.
			// This unsafe 'should' be safeish considering the type is the same and we are just
			// constraining, not widening, the lifetime.
			unsafe {
				&*(&storages.0 as *const Self::SingleStorageLocked
					as *const TT::SingleStorageLocked)
			}
		} else {
			TAIL::get_locked_storage_ref::<TT>(&storages.1)
		}
	}

	#[inline]
	fn get_locked_storage_ref_mut<'s, TT: ValueTypes>(
		_storages: &mut Self::StorageLocked,
	) -> &'s mut TT::SingleStorageLocked {
		panic!(
			"requested a component type that does not exist in this storage: {}",
			std::any::type_name::<TT::SelfRaw>()
		)
	}
}

impl<HEAD: 'static, TAIL: ValueTypes> ValueTypes for (&'static mut HEAD, TAIL) {
	type Raw = (HEAD, TAIL::Raw);
	type SelfRaw = &'static mut HEAD;
	type Storage = (Rc<RefCell<DensePagedData<HEAD>>>, TAIL::Storage);
	type StorageLocked = (Self::SingleStorageLocked, TAIL::StorageLocked);
	type SingleStorageLocked =
		OwningHandle<Rc<RefCell<DensePagedData<HEAD>>>, RefMut<'static, DensePagedData<HEAD>>>;

	#[inline]
	fn push_type_ids(arr: &mut TypeIdCacheVec) {
		arr.push(TypeId::of::<HEAD>());
		TAIL::push_type_ids(arr);
	}

	#[inline]
	fn swap_remove_type_ids(arr: &mut ArrayVec<[(TypeId, usize); 32]>) {
		if let Some(found_idx) = arr
			.iter()
			.position(|(tid, _idx)| *tid == TypeId::of::<HEAD>())
		{
			arr.swap_remove(found_idx);
		}
		TAIL::swap_remove_type_ids(arr);
	}

	#[inline]
	fn get_storage_idxs(
		storages: &IndexMap<TypeId, Rc<RefCell<dyn DynDensePagedData>>, UniqueHasherBuilder>,
		mut vec: Vec<usize>,
	) -> Vec<usize> {
		let idx = storages.get_full(&TypeId::of::<HEAD>()).unwrap().0;
		vec.push(idx);
		TAIL::get_storage_idxs(storages, vec)
	}

	#[inline]
	fn get_or_create_storage(
		storages: &mut IndexMap<TypeId, Rc<RefCell<dyn DynDensePagedData>>, UniqueHasherBuilder>,
	) -> Self::Storage {
		let storage = if let Some(storage) = storages.get(&TypeId::of::<HEAD>()) {
			storage
				.borrow()
				.as_any()
				.downcast_ref::<DensePagedData<HEAD>>()
				.expect("Failed to cast type to itself?")
				.get_strong_self()
		} else {
			let storage = DensePagedData::<HEAD>::new(storages.len());
			storages.insert(TypeId::of::<HEAD>(), storage.clone());
			storage
		};
		(storage, TAIL::get_or_create_storage(storages))
	}

	#[inline]
	fn try_storage_locked(storage: &Self::Storage) -> Result<Self::StorageLocked, ()> {
		Ok((
			OwningHandle::new_mut(storage.0.clone()),
			TAIL::try_storage_locked(&storage.1)?,
		))
	}

	#[inline]
	fn get_locked_storage_ref<'s, TT: ValueTypes>(
		storages: &Self::StorageLocked,
	) -> &'s TT::SingleStorageLocked {
		if TypeId::of::<TT::SelfRaw>() == TypeId::of::<&'static HEAD>()
			|| TypeId::of::<TT::SelfRaw>() == TypeId::of::<&'static mut HEAD>()
		{
			// TODO:  Lack of GATs sucks...  This unsafe can be removed once they exist.
			// This unsafe 'should' be safeish considering the type is the same and we are just
			// constraining, not widening, the lifetime.
			unsafe {
				&*(&storages.0 as *const Self::SingleStorageLocked
					as *const TT::SingleStorageLocked)
			}
		} else {
			TAIL::get_locked_storage_ref::<TT>(&storages.1)
		}
	}

	#[inline]
	fn get_locked_storage_ref_mut<'s, TT: ValueTypes>(
		storages: &mut Self::StorageLocked,
	) -> &'s mut TT::SingleStorageLocked {
		if TypeId::of::<TT::SelfRaw>() == TypeId::of::<&'static mut HEAD>() {
			// TODO:  Lack of GATs sucks...  This unsafe can be removed once they exist.
			// This unsafe 'should' be safeish considering the type is the same and we are just
			// constraining, not widening, the lifetime.
			unsafe {
				&mut *(&mut storages.0 as *mut Self::SingleStorageLocked
					as *mut TT::SingleStorageLocked)
			}
		} else {
			TAIL::get_locked_storage_ref_mut::<TT>(&mut storages.1)
		}
	}
}

impl<HEAD: 'static, TAIL: InsertValueTypes> InsertValueTypes for (&'static mut HEAD, TAIL) {
	#[inline(always)]
	fn fill_include_type_ids(arr: &mut TypeIdCacheVec) {
		arr.push(TypeId::of::<HEAD>());
		TAIL::fill_include_type_ids(arr);
	}
	#[inline(always)]
	fn fill_exclude_type_ids(arr: &mut TypeIdCacheVec) {
		TAIL::fill_exclude_type_ids(arr);
	}

	type MoveData = (HEAD, TAIL::MoveData);
	type MoveDataVec = (Vec<HEAD>, TAIL::MoveDataVec);

	#[inline]
	fn push(storage_locked: &mut Self::StorageLocked, group: usize, data: Self::MoveData) {
		storage_locked.0.push(group, data.0);
		TAIL::push(&mut storage_locked.1, group, data.1);
	}

	#[inline]
	fn push_prelocked(
		storage_locked: &mut AllLockedStorages,
		idxs: &[usize],
		group: usize,
		data: Self::MoveData,
	) {
		storage_locked[idxs[0]]
			.as_any_mut()
			.downcast_mut::<DensePagedData<HEAD>>()
			.expect("failed to cast type into self?")
			.push(group, data.0);
		TAIL::push_prelocked(storage_locked, &idxs[1..], group, data.1)
	}

	#[inline]
	fn ensure_vec_length(data: &Self::MoveDataVec, len: usize) -> bool {
		data.0.len() == len && TAIL::ensure_vec_length(&data.1, len)
	}

	#[inline]
	fn extend(storage_locked: &mut Self::StorageLocked, group: usize, data: Self::MoveDataVec) {
		storage_locked.0.extend(group, data.0);
		TAIL::extend(&mut storage_locked.1, group, data.1);
	}
}

pub trait GetValueTypes<'a>: ValueTypes {
	type StoragesLockedRef: Sized;
	// Uuuugh lack of GATs...
	// fn get_locked_storage_ptr<'s, VTs: ValueTypes>(
	// 	storages: &mut VTs::StorageLocked,
	// ) -> &'s mut VTs::StorageLocked;
	fn cast_locked_storages<VTs: ValueTypes>(
		storages: &mut VTs::StorageLocked,
	) -> Self::StoragesLockedRef;
	type GetRef: 'a;
	fn get<EntityType: Entity>(
		storage_locked: &'a mut Self::StoragesLockedRef,
		group: usize,
		index: usize,
	) -> Option<Self::GetRef>;
}

impl<'a> GetValueTypes<'a> for () {
	type StoragesLockedRef = ();

	fn cast_locked_storages<VTs: ValueTypes>(
		storages: &mut <VTs as ValueTypes>::StorageLocked,
	) -> Self::StoragesLockedRef {
	}

	type GetRef = ();

	fn get<EntityType: Entity>(
		_storage_locked: &'a mut Self::StorageLocked,
		_group: usize,
		_index: usize,
	) -> Option<Self::GetRef> {
		Some(())
	}
}

impl<'a, HEAD: 'static, TAIL: GetValueTypes<'a>> GetValueTypes<'a> for (&'static HEAD, TAIL) {
	type StoragesLockedRef = (
		&'a OwningHandle<Rc<RefCell<DensePagedData<HEAD>>>, Ref<'static, DensePagedData<HEAD>>>,
		TAIL::StoragesLockedRef,
	);

	fn cast_locked_storages<VTs: ValueTypes>(
		storages: &mut <VTs as ValueTypes>::StorageLocked,
	) -> Self::StoragesLockedRef {
		(
			VTs::get_locked_storage_ref::<Self>(storages),
			TAIL::cast_locked_storages::<VTs>(storages),
		)
	}

	type GetRef = (&'a HEAD, TAIL::GetRef);

	fn get<EntityType: Entity>(
		storage_locked: &'a mut Self::StoragesLockedRef,
		group: usize,
		index: usize,
	) -> Option<Self::GetRef> {
		// TODO:  Maybe make the `group` access unchecked?
		if let Some(found) = storage_locked.0.data[group].get(index) {
			if let Some(rest) = TAIL::get::<EntityType>(&mut storage_locked.1, group, index) {
				Some((found, rest))
			} else {
				None
			}
		} else {
			None
		}
	}
}

impl<'a, HEAD: 'static, TAIL: GetValueTypes<'a>> GetValueTypes<'a> for (&'static mut HEAD, TAIL) {
	type StoragesLockedRef = (
		&'a mut OwningHandle<
			Rc<RefCell<DensePagedData<HEAD>>>,
			RefMut<'static, DensePagedData<HEAD>>,
		>,
		TAIL::StoragesLockedRef,
	);

	fn cast_locked_storages<VTs: ValueTypes>(
		storages: &mut <VTs as ValueTypes>::StorageLocked,
	) -> Self::StoragesLockedRef {
		(
			VTs::get_locked_storage_ref_mut::<Self>(storages),
			TAIL::cast_locked_storages::<VTs>(storages),
		)
	}

	type GetRef = (&'a mut HEAD, TAIL::GetRef);

	fn get<EntityType: Entity>(
		storage_locked: &'a mut Self::StoragesLockedRef,
		group: usize,
		index: usize,
	) -> Option<Self::GetRef> {
		// TODO:  Maybe make the `group` access unchecked?
		if let Some(found) = storage_locked.0.data[group].get_mut(index) {
			if let Some(rest) = TAIL::get::<EntityType>(&mut storage_locked.1, group, index) {
				Some((found, rest))
			} else {
				None
			}
		} else {
			None
		}
	}
}

pub struct DenseEntityPagedMultiValueTableBuilder<EntityType: Entity> {
	entity_table: Rc<RefCell<EntityTable<EntityType>>>,
	capacity: usize,
}

impl<EntityType: Entity> TableBuilder for DenseEntityPagedMultiValueTableBuilder<EntityType> {
	type Table = DenseEntityDynamicPagedMultiValueTable<EntityType>;

	fn build(
		self,
		database_id: DatabaseId,
		table_name: &str,
		table_id: TableId,
	) -> Rc<RefCell<Self::Table>> {
		let mut entities = self.entity_table.borrow_mut();
		let this = Rc::new(RefCell::new(DenseEntityDynamicPagedMultiValueTable::<
			EntityType,
		> {
			this: Weak::new(),
			database_id,
			table_name: table_name.into(),
			table_id,
			reverse: SecondaryEntityIndex::new(ComponentLocations::INVALID),
			entities: Vec::with_capacity(self.capacity),
			storages: IndexMap::default(),
			group_inserts: IndexMap::default(),
			group_queries: IndexMap::default(),
		}));
		this.borrow_mut().this = Rc::downgrade(&this);
		let another_this = this.clone();
		let _id = entities.on_delete_entity(Box::new(move |_entity_table_id, entity| {
			if let Ok(mut deleter) = another_this.try_borrow_mut() {
				// Ignore the entity does not exist error
				let _ = deleter.delete(entity);// .expect("Unknown deletion error while deleting valid entity");
			} else {
				panic!("DenseEntityDynamicPagedMultiValueTable<{}> already locked while deleting an entity, all tables must be free when deleting an Entity", std::any::type_name::<EntityType>());
			};
		}));
		this
	}
}

impl<EntityType: Entity> Table for DenseEntityDynamicPagedMultiValueTable<EntityType> {
	fn as_any(&self) -> &dyn Any {
		self
	}

	fn get_strong(&self) -> Rc<RefCell<dyn Table>> {
		self.get_strong_self()
	}

	fn get_database_id(&self) -> DatabaseId {
		self.database_id
	}

	fn table_name(&self) -> &str {
		&self.table_name
	}

	fn table_id(&self) -> TableId {
		self.table_id
	}
}

impl<EntityType: Entity> TableCastable for DenseEntityDynamicPagedMultiValueTable<EntityType> {
	fn get_strong_self(&self) -> Rc<RefCell<Self>> {
		self.this.upgrade().unwrap() // It's obviously valid since it's obviously self
	}
}

#[cfg(test)]
mod tests {
	use crate::database::*;
	use crate::tables::dense_entity_dynamic_paged_multi_value_table::DenseEntityDynamicPagedMultiValueTable;
	use crate::tables::entity_table::EntityTable;
	use crate::{tl, TL};
	use std::cell::RefCell;
	use std::rc::Rc;

	fn basic_setup() -> (
		Database,
		Rc<RefCell<EntityTable<u64>>>,
		Rc<RefCell<DenseEntityDynamicPagedMultiValueTable<u64>>>,
	) {
		let mut database = Database::new();
		let entities_storage = database
			.tables
			.create("entities", EntityTable::<u64>::builder())
			.unwrap();
		let multi_storage = database
			.tables
			.create(
				"multi",
				DenseEntityDynamicPagedMultiValueTable::<u64>::builder(entities_storage.clone()),
			)
			.unwrap();
		(database, entities_storage, multi_storage)
	}

	#[test]
	fn transforms() {
		let (_database, entities_storage, multi_storage) = basic_setup();
		let mut entities = entities_storage.borrow_mut();
		let mut multi = multi_storage.borrow_mut();
		let mut first_inserter = multi
			.group_insert::<TL![&mut bool, &mut usize, &mut u8]>()
			.unwrap();
		let mut next_inserter = multi.group_insert::<TL![&mut isize]>().unwrap();
		let mut query_before = multi.group_query::<TL![&bool, &usize]>().unwrap();
		let mut query_after = multi.group_query::<TL![&bool, &isize]>().unwrap();
		let entity1 = entities.insert();
		first_inserter
			.lock(&mut multi)
			.insert(entity1, tl![true, 42, 16])
			.unwrap();
		assert_eq!(
			query_before.lock(&multi).get::<TL![&usize]>(entity1),
			Some(tl![&42])
		);
		assert_eq!(
			query_before.lock(&multi).get::<TL![&bool, &usize]>(entity1),
			Some(tl![&true, &42])
		);
		assert_eq!(query_after.lock(&multi).get::<TL![&isize]>(entity1), None);
		{
			let mut lock = multi.lock().unwrap();
			lock.transform::<TL![usize], _>(entity1, &next_inserter, tl![21isize])
				.unwrap();
		}
		assert_eq!(query_before.lock(&multi).get::<TL![&usize]>(entity1), None);
		assert_eq!(
			query_after.lock(&multi).get::<TL![&bool, &isize]>(entity1),
			Some(tl![&true, &21])
		);
		{
			let mut lock = multi.lock().unwrap();
			lock.transform::<TL![isize], _>(entity1, &first_inserter, tl![false, 42usize, 16])
				.unwrap();
		}
		assert_eq!(
			query_before.lock(&multi).get::<TL![&bool, &usize]>(entity1),
			Some(tl![&false, &42])
		);
		assert_eq!(query_after.lock(&multi).get::<TL![&isize]>(entity1), None);
		assert_eq!(
			query_before.lock(&multi).get_all(entity1),
			Some(tl![&false, &42])
		);
	}

	#[test]
	fn bench_test() {
		pub struct A(pub u64);
		pub struct B(pub u64);
		pub struct C(pub u64);
		pub struct D(pub u64);
		pub struct E(pub u64);
		pub struct F(pub u64);
		pub struct G(pub u64);
		pub struct H(pub u64);
		pub struct P(pub u64);

		pub type Type8 = TL![
			&'static mut A,
			&'static mut B,
			&'static mut C,
			&'static mut D,
			&'static mut E,
			&'static mut F,
			&'static mut G,
			&'static mut H
		];

		pub fn type8_new(v: u64) -> TL![A, B, C, D, E, F, G, H] {
			tl![A(v), B(v), C(v), D(v), E(v), F(v), G(v), H(v)]
		}

		let (_database, entities_storage, multi_storage) = basic_setup();
		let mut entities = entities_storage.borrow_mut();
		let entity_vec: Vec<_> = (0..100).map(|_| entities.insert().raw()).collect();
		let mut multi = multi_storage.borrow_mut();
		let mut inserter = multi.group_insert::<Type8>().unwrap();
		{
			let mut lock = inserter.lock(&mut multi);
			for &e in entity_vec.iter() {
				lock.insert(entities.valid(e).unwrap(), type8_new(e))
					.unwrap();
			}
		}
		let transform_to = multi.group_insert::<TL![&mut P]>().unwrap();
		let mut lock = multi.lock().unwrap();
		for e in entity_vec {
			let _ = lock
				.transform::<TL![D], _>(entities.valid(e).unwrap(), &transform_to, tl![P(e)])
				.unwrap();
		}
	}

	#[test]
	fn insertions_and_deletions() {
		let (_database, entities_storage, multi_storage) = basic_setup();
		let mut entities = entities_storage.borrow_mut();
		let mut multi = multi_storage.borrow_mut();
		let mut null_inserter = multi.group_insert::<TL![]>().unwrap();
		let mut single_inserter = multi.group_insert::<TL![&mut usize]>().unwrap();
		let mut nulls = multi.group_query::<TL![]>().unwrap();
		let mut singles = multi.group_query::<TL![&mut usize]>().unwrap();
		let entity1 = entities.insert();
		null_inserter
			.lock(&mut multi)
			.insert(entity1, tl![])
			.unwrap();
		let entity1 = entity1.raw();
		let entity2 = entities.insert();
		single_inserter
			.lock(&mut multi)
			.insert(entity2, tl![42])
			.unwrap();
		assert!(null_inserter
			.lock(&mut multi)
			.insert(entity2, tl![])
			.is_err());
		{
			let mut multi_locked = multi.lock().unwrap();
			multi_locked.delete(entity2).unwrap();
		}
		multi.delete(entities.valid(entity1).unwrap()).unwrap();
		let entity1 = entities.insert().raw();
		let entity2 = entities.insert().raw();
		let entity3 = entities.insert().raw();
		null_inserter
			.lock(&mut multi)
			.insert(entities.valid(entity1).unwrap(), tl![])
			.unwrap();
		null_inserter
			.lock(&mut multi)
			.insert(entities.valid(entity2).unwrap(), tl![])
			.unwrap();
		null_inserter
			.lock(&mut multi)
			.insert(entities.valid(entity3).unwrap(), tl![])
			.unwrap();
		multi.delete(entities.valid(entity1).unwrap()).unwrap();
		multi.delete(entities.valid(entity2).unwrap()).unwrap();
		multi.delete(entities.valid(entity3).unwrap()).unwrap();
		let entity_vec: Vec<_> = entities.extend_iter().take(10).collect();
		single_inserter
			.lock(&mut multi)
			.extend_slices(&entity_vec, tl![(0..(entity_vec.len())).collect()])
			.unwrap();
		for (mut i, e) in entity_vec.iter().enumerate() {
			assert_eq!(singles.lock(&mut multi).get_all(*e).unwrap(), tl![&mut i]);
		}
	}
}
