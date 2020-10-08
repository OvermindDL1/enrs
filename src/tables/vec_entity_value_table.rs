use crate::database::{DatabaseId, TableId};
use crate::entity::Entity;
use crate::table::{Table, TableBuilder, TableCastable};
use crate::tables::entity_table::{EntityTable, ValidEntity};
use smol_str::SmolStr;
use std::any::Any;
use std::cell::RefCell;
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::rc::{Rc, Weak};

pub struct VecEntityValueTable<EntityType: Entity, ValueType: 'static> {
	this: Weak<RefCell<Self>>,
	database_id: DatabaseId,
	table_name: SmolStr,
	table_id: TableId,
	entities: Vec<EntityType>,
	values: Vec<MaybeUninit<ValueType>>,
	count: usize,
}

impl<EntityType: Entity, ValueType: 'static> VecEntityValueTable<EntityType, ValueType> {
	pub fn builder(
		entity_table: Rc<RefCell<EntityTable<EntityType>>>,
	) -> VecEntityValueTableBuilder<EntityType, ValueType> {
		VecEntityValueTableBuilder {
			entity_table,
			capacity: 0,
			_phantom: PhantomData,
		}
	}

	pub fn builder_with_capacity(
		entity_table: Rc<RefCell<EntityTable<EntityType>>>,
		capacity: usize,
	) -> VecEntityValueTableBuilder<EntityType, ValueType> {
		VecEntityValueTableBuilder {
			entity_table,
			capacity,
			_phantom: PhantomData,
		}
	}

	pub fn contains(&self, entity: EntityType) -> bool {
		self.entities.len() > entity.idx() && self.entities[entity.idx()] == entity
	}

	pub fn len(&self) -> usize {
		self.count
	}

	pub fn is_empty(&self) -> bool {
		self.count == 0
	}

	pub fn insert(&mut self, entity: ValidEntity<EntityType>, value: ValueType) -> Result<(), ()> {
		let entity = entity.raw();
		if self.entities.len() <= entity.idx() {
			self.entities.resize(entity.idx() + 1, EntityType::new(0));
			self.values.reserve(entity.idx() - self.values.len() + 1);
			unsafe {
				self.values.set_len(entity.idx() + 1);
			}
		}
		if self.entities[entity.idx()] == entity {
			return Err(());
		}
		self.entities[entity.idx()] = entity;
		unsafe {
			*self.values.get_unchecked_mut(entity.idx()) = MaybeUninit::new(value);
		}
		self.count += 1;
		Ok(())
	}

	pub fn delete(&mut self, entity: EntityType) -> Result<(), ()> {
		if self.entities.len() <= entity.idx() || self.entities[entity.idx()] != entity {
			return Err(());
		}
		self.entities[entity.idx()] = EntityType::new(0);
		unsafe {
			// Can remove this and just forget about `self.values` if we can ensure it doesn't have a `Drop` implementation
			let mut forgetting = MaybeUninit::uninit();
			std::mem::swap(self.values.get_unchecked_mut(entity.idx()), &mut forgetting);
		}
		self.count -= 1;
		Ok(())
	}
}

pub struct VecEntityValueTableBuilder<EntityType: Entity, ValueType: 'static> {
	entity_table: Rc<RefCell<EntityTable<EntityType>>>,
	capacity: usize,
	_phantom: PhantomData<ValueType>,
}

impl<EntityType: Entity, ValueType: 'static> TableBuilder
	for VecEntityValueTableBuilder<EntityType, ValueType>
{
	type Table = VecEntityValueTable<EntityType, ValueType>;

	fn build(
		self,
		database_id: DatabaseId,
		table_name: &str,
		table_id: TableId,
	) -> Rc<RefCell<Self::Table>> {
		let mut entities = self.entity_table.borrow_mut();
		let this = Rc::new(RefCell::new(VecEntityValueTable::<EntityType, ValueType> {
			this: Weak::new(),
			database_id,
			table_name: table_name.into(),
			table_id,
			entities: Vec::with_capacity(self.capacity),
			values: Vec::with_capacity(self.capacity),
			count: 0,
		}));
		this.borrow_mut().this = Rc::downgrade(&this);
		let another_this = this.clone();
		let _id = entities.on_delete_entity(Box::new(move |_entity_table_id, entity| {
			if let Ok(mut deleter) = another_this.try_borrow_mut() {
				// Don't care if it didn't exist
				let _ = deleter.delete(entity.raw()); // .expect("Unknown deletion error while deleting valid entity")
			} else {
				panic!("DenseEntityTable<{}, {}> already locked while deleting an entity, all tables must be free when deleting an Entity", std::any::type_name::<EntityType>(), std::any::type_name::<ValueType>());
			};
		}));
		this
	}
}

impl<EntityType: Entity, ValueType: 'static> Table for VecEntityValueTable<EntityType, ValueType> {
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

impl<EntityType: Entity, ValueType: 'static> TableCastable
	for VecEntityValueTable<EntityType, ValueType>
{
	fn get_strong_self(&self) -> Rc<RefCell<Self>> {
		self.this.upgrade().unwrap() // It's obviously valid since it's obviously self
	}
}
