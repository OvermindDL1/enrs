use crate::database::{DatabaseId, TableId};
use crate::entity::Entity;
use crate::table::{Table, TableBuilder, TableCastable};
use crate::tables::entity_table::{EntityTable, ValidEntity};
use crate::utils::secondary_entity_index::{SecondaryEntityIndex, SecondaryEntityIndexErrors};
use smol_str::SmolStr;
use std::any::Any;
use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::{Rc, Weak};

pub struct DenseEntityValueTable<EntityType: Entity, ValueType: 'static> {
	this: Weak<RefCell<Self>>,
	database_id: DatabaseId,
	table_name: SmolStr,
	table_id: TableId,
	//entity_table: EntityTable<EntityType>,
	reverse: SecondaryEntityIndex<EntityType, usize>,
	entities: Vec<EntityType>,
	values: Vec<ValueType>,
}

impl<EntityType: Entity, ValueType: 'static> DenseEntityValueTable<EntityType, ValueType> {
	pub fn builder(
		entity_table: Rc<RefCell<EntityTable<EntityType>>>,
	) -> DenseEntityValueTableBuilder<EntityType, ValueType> {
		DenseEntityValueTableBuilder {
			entity_table,
			capacity: 0,
			_phantom: PhantomData,
		}
	}

	pub fn builder_with_capacity(
		entity_table: Rc<RefCell<EntityTable<EntityType>>>,
		capacity: usize,
	) -> DenseEntityValueTableBuilder<EntityType, ValueType> {
		DenseEntityValueTableBuilder {
			entity_table,
			capacity,
			_phantom: PhantomData,
		}
	}

	pub fn contains(&self, entity: EntityType) -> bool {
		self.reverse.get(entity).is_ok()
	}

	pub fn len(&self) -> usize {
		self.entities.len()
	}

	pub fn is_empty(&self) -> bool {
		self.entities.is_empty()
	}

	pub fn insert(
		&mut self,
		entity: ValidEntity<EntityType>,
		value: ValueType,
	) -> Result<(), SecondaryEntityIndexErrors<EntityType>> {
		let location = self.reverse.insert_mut(entity.raw())?;
		*location = self.entities.len();
		self.entities.push(entity.raw());
		self.values.push(value);
		Ok(())
	}

	pub fn delete(
		&mut self,
		entity: EntityType,
	) -> Result<(), SecondaryEntityIndexErrors<EntityType>> {
		let location_mut = self.reverse.get_mut(entity)?;
		if self.entities[*location_mut] != entity {
			return Err(SecondaryEntityIndexErrors::IndexDoesNotExist(entity));
		}
		let location = *location_mut;
		*location_mut = usize::MAX;
		self.entities.swap_remove(location);
		self.values.swap_remove(location);
		if self.entities.len() > location {
			let moved = self
				.reverse
				.get_mut(self.entities[location])
				.expect("reverse mapping is in invalid state with DenseEntityValueTable");
			*moved = location
		}
		Ok(())
	}
}

pub struct DenseEntityValueTableBuilder<EntityType: Entity, ValueType: 'static> {
	entity_table: Rc<RefCell<EntityTable<EntityType>>>,
	capacity: usize,
	_phantom: PhantomData<ValueType>,
}

impl<EntityType: Entity, ValueType: 'static> TableBuilder
	for DenseEntityValueTableBuilder<EntityType, ValueType>
{
	type Table = DenseEntityValueTable<EntityType, ValueType>;

	fn build(
		self,
		database_id: DatabaseId,
		table_name: &str,
		table_id: TableId,
	) -> Rc<RefCell<Self::Table>> {
		let mut entities = self.entity_table.borrow_mut();
		let this = Rc::new(RefCell::new(
			DenseEntityValueTable::<EntityType, ValueType> {
				this: Weak::new(),
				database_id,
				table_name: table_name.into(),
				table_id,
				reverse: SecondaryEntityIndex::new(usize::MAX),
				entities: Vec::with_capacity(self.capacity),
				values: Vec::with_capacity(self.capacity),
			},
		));
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

impl<EntityType: Entity, ValueType: 'static> Table
	for DenseEntityValueTable<EntityType, ValueType>
{
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
	for DenseEntityValueTable<EntityType, ValueType>
{
	fn get_strong_self(&self) -> Rc<RefCell<Self>> {
		self.this.upgrade().unwrap() // It's obviously valid since it's obviously self
	}
}
