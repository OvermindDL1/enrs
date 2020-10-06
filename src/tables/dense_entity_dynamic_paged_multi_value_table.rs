use crate::database::{DatabaseId, TableId};
use crate::entity::Entity;
use crate::table::{Table, TableBuilder, TableCastable};
use crate::tables::{EntityTable, ValidEntity};
use crate::utils::secondary_entity_index::{SecondaryEntityIndex, SecondaryEntityIndexErrors};
use crate::utils::unique_hasher::UniqueHasherBuilder;
use arrayvec::ArrayVec;
use indexmap::map::IndexMap;
use owning_ref::OwningHandle;
use smol_str::SmolStr;
use std::any::{Any, TypeId};
use std::cell::{Ref, RefCell, RefMut};
use std::marker::PhantomData;
use std::rc::{Rc, Weak};

#[derive(Debug, PartialEq, Eq)]
pub enum DenseEntityDynamicPagedMultiValueTableErrors<EntityType: Entity> {
	SecondaryIndexError(SecondaryEntityIndexErrors<EntityType>),
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
	fn as_any(&self) -> &dyn std::any::Any;
	fn get_strong(&self) -> Rc<RefCell<dyn DynDensePagedData>>;
	fn get_idx(&self) -> usize;
	fn ensure_group_count(&mut self, group_count: usize);
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
}

impl<ValueType: 'static> DynDensePagedData for DensePagedData<ValueType> {
	fn as_any(&self) -> &dyn Any {
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
	_phantom: PhantomData<EntityType>,
}

impl<EntityType: Entity, VTs: InsertValueTypes> Clone for GroupInsert<EntityType, VTs> {
	fn clone(&self) -> Self {
		GroupInsert {
			group: self.group,
			storage: self.storage.clone(),
			_phantom: PhantomData,
		}
	}
}

impl<EntityType: Entity, VTs: ValueTypes> GroupQuery<EntityType, VTs> {
	pub fn try_lock(&mut self) -> Option<GroupQueryLock<EntityType, VTs>> {
		if let Ok(storage_locked) = VTs::try_storage_locked(&self.storage) {
			Some(GroupQueryLock {
				group: self.group,
				storage_locked,
				_phantom: PhantomData,
			})
		} else {
			None
		}
	}

	pub fn lock<'a>(&'a mut self) -> GroupQueryLock<'a, EntityType, VTs> {
		self.try_lock().expect("unable to lock GroupQuery")
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

pub struct GroupQueryLock<'a, EntityType: Entity, VTs: ValueTypes> {
	group: usize,
	storage_locked: VTs::StorageLocked, // When GAT's exist then pass `'a` into StorageLocked
	_phantom: PhantomData<&'a EntityType>,
}

pub struct GroupInsertLock<'a, 's, EntityType: Entity, VTs: InsertValueTypes> {
	group: usize,
	storage_locked: VTs::StorageLocked, // When GAT's exist then pass `'a` into StorageLocked
	table: &'s mut DenseEntityDynamicPagedMultiValueTable<EntityType>,
	_phantom: PhantomData<&'a ()>,
}

impl<'g, EntityType: Entity, VTs: ValueTypes> GroupQueryLock<'g, EntityType, VTs> {}

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
	exclude: &'a [TypeId],
}

#[derive(PartialEq, Eq, Hash)]
struct QueryTypedPagedKeyBoxed {
	include: Box<[TypeId]>,
	exclude: Box<[TypeId]>,
}

impl<'a> QueryTypedPagedKey<'a> {
	fn to_box(self) -> QueryTypedPagedKeyBoxed {
		QueryTypedPagedKeyBoxed {
			// read_only: self.read_only.to_vec().into_boxed_slice(),
			// read_write: self.read_write.to_vec().into_boxed_slice(),
			include: self.include.into(),
			exclude: self.exclude.into(),
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
		&*key.include == self.include && &*key.exclude == self.exclude
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
		if entities_group.len() > 0 {
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

	pub fn group_query<VTs: ValueTypes>(&mut self) -> Result<GroupQuery<EntityType, VTs>, ()> {
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
	) -> Result<GroupInsert<EntityType, VTs>, ()> {
		let include_tids = VTs::get_include_type_ids();
		let exclude_tids = VTs::get_exclude_type_ids();
		let key = QueryTypedPagedKey {
			include: include_tids.as_slice(),
			exclude: exclude_tids.as_slice(),
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
					_phantom: PhantomData,
				};
				*group_page = Some(Box::new(group.clone()));
				group
			}
		} else {
			let group = GroupInsert::<EntityType, VTs> {
				group: self.group_inserts.len(),
				storage: VTs::get_or_create_storage(&mut self.storages),
				_phantom: PhantomData,
			};
			self.group_inserts
				.insert(key.to_box(), Some(Box::new(group.clone())));
			self.ensure_group_count_on_storages();
			group
		};
		Ok(group)
	}

	// pub fn insert_into_group<VTs: ValueTypes>(
	// 	&mut self,
	// 	locked: &mut GroupLock<EntityType, VTs>,
	// 	entity: ValidEntity<EntityType>,
	// 	data: VTs::MoveData,
	// ) -> Result<(), DenseEntityDynamicPagedMultiValueTableErrors<EntityType>> {
	// 	if locked.group_page == usize::MAX {
	// 		// self.pages.
	// 	}
	// 	let location = Self::insert_valid_location_mut(
	// 		&mut self.reverse,
	// 		&mut self.entities,
	// 		entity.raw(),
	// 		locked.group,
	// 	)?;
	// 	VTs::push(&mut locked.storage_locked, locked.group, data);
	// 	Ok(())
	// }
}

pub trait ValueTypes: 'static {
	type Raw: 'static;
	type Storage: 'static + Clone;
	type StorageLocked: 'static;
	fn get_or_create_storage(
		storages: &mut IndexMap<TypeId, Rc<RefCell<dyn DynDensePagedData>>, UniqueHasherBuilder>,
	) -> Self::Storage;
	fn try_storage_locked(storage: &Self::Storage) -> Result<Self::StorageLocked, ()>;
}

// Ask if this should be increased in size, but honestly, more tables should probably be used instead
type TypeIdCacheVec = ArrayVec<[TypeId; 31]>;

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
	fn push(storage_locked: &mut Self::StorageLocked, group: usize, data: Self::MoveData);
}

impl ValueTypes for () {
	type Raw = ();
	type Storage = ();
	type StorageLocked = ();

	#[inline]
	fn get_or_create_storage(
		_storages: &mut IndexMap<TypeId, Rc<RefCell<dyn DynDensePagedData>>, UniqueHasherBuilder>,
	) -> Self::Storage {
	}

	#[inline]
	fn try_storage_locked(_storage: &Self::Storage) -> Result<Self::StorageLocked, ()> {
		Ok(())
	}
}

impl InsertValueTypes for () {
	#[inline(always)]
	fn fill_include_type_ids(_arr: &mut TypeIdCacheVec) {}
	#[inline(always)]
	fn fill_exclude_type_ids(_arr: &mut TypeIdCacheVec) {}

	type MoveData = ();

	#[inline]
	fn push(_storage_locked: &mut Self::StorageLocked, _group: usize, _data: Self::MoveData) {}
}

pub enum CannotMoveGroupWithImmutableType {}

impl<HEAD: 'static, TAIL: ValueTypes> ValueTypes for (&'static HEAD, TAIL) {
	type Raw = (HEAD, TAIL::Raw);
	type Storage = (Rc<RefCell<DensePagedData<HEAD>>>, TAIL::Storage);
	type StorageLocked = (
		OwningHandle<Rc<RefCell<DensePagedData<HEAD>>>, Ref<'static, DensePagedData<HEAD>>>,
		TAIL::StorageLocked,
	);

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
}

impl<HEAD: 'static, TAIL: ValueTypes> ValueTypes for (&'static mut HEAD, TAIL) {
	type Raw = (HEAD, TAIL::Raw);
	type Storage = (Rc<RefCell<DensePagedData<HEAD>>>, TAIL::Storage);
	type StorageLocked = (
		OwningHandle<Rc<RefCell<DensePagedData<HEAD>>>, RefMut<'static, DensePagedData<HEAD>>>,
		TAIL::StorageLocked,
	);

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

	#[inline]
	fn push(storage_locked: &mut Self::StorageLocked, group: usize, data: Self::MoveData) {
		storage_locked.0.push(group, data.0);
		TAIL::push(&mut storage_locked.1, group, data.1);
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
				//deleter.delete(entity.raw()).expect("Unknown deletion error while deleting valid entity")
				todo!("Add support to delete components");
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
	fn insertions() {
		let (_database, entities_storage, multi_storage) = basic_setup();
		let mut entities = entities_storage.borrow_mut();
		let mut multi = multi_storage.borrow_mut();
		let mut null_inserter = multi.group_insert::<TL![]>().unwrap();
		let mut single_inserter = multi.group_insert::<TL![&mut usize]>().unwrap();
		let nulls = multi.group_query::<TL![]>().unwrap();
		let singles = multi.group_query::<TL![&mut usize]>().unwrap();
		let entity1 = entities.insert();
		null_inserter
			.lock(&mut multi)
			.insert(entity1, tl![])
			.unwrap();
		let entity2 = entities.insert();
		single_inserter
			.lock(&mut multi)
			.insert(entity2, tl![42])
			.unwrap();
		assert!(null_inserter
			.lock(&mut multi)
			.insert(entity2, tl![])
			.is_err());
	}
}
