use crate::database::{DatabaseId, TableId};
use crate::entity::Entity;
use crate::table::fields::{Field, IndexField};
use crate::table::{Table, TableBuilder, TableCastable};
use crate::tables::entity_table::{EntityTable, ValidEntity};
use crate::utils::secondary_entity_index::{SecondaryEntityIndex, SecondaryIndexErrors};
use smol_str::SmolStr;
use std::any::Any;
use std::cell::{BorrowMutError, RefCell, RefMut};
use std::marker::PhantomData;
use std::rc::{Rc, Weak};

pub struct DenseEntityValueTable<EntityType: Entity, ValueType: 'static> {
	this: Weak<RefCell<Self>>,
	database_id: DatabaseId,
	table_name: SmolStr,
	table_id: TableId,
	//entity_table: Rc<RefCell<EntityTable<EntityType>>>,
	reverse: Rc<RefCell<SecondaryEntityIndex<EntityType, usize>>>,
	entities: Rc<RefCell<Vec<EntityType>>>,
	values: Rc<RefCell<Vec<ValueType>>>,
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

	pub fn insert_query(&self) -> InsertQuery<EntityType, ValueType> {
		InsertQuery {
			//entity_table: self.entity_table.clone(),
			reverse: self.reverse.clone(),
			entities: self.entities.clone(),
			values: self.values.clone(),
		}
	}

	pub fn delete_query(&self) -> DeleteQuery<EntityType, ValueType> {
		DeleteQuery {
			//entity_table: self.entity_table.clone(),
			reverse: self.reverse.clone(),
			entities: self.entities.clone(),
			values: self.values.clone(),
		}
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
				reverse: Rc::new(RefCell::new(SecondaryEntityIndex::new(usize::MAX))),
				entities: Rc::new(RefCell::new(Vec::with_capacity(self.capacity))),
				values: Rc::new(RefCell::new(Vec::with_capacity(self.capacity))),
			},
		));
		this.borrow_mut().this = Rc::downgrade(&this);
		let mut delete_query = this.borrow().delete_query();
		let _id = entities.on_delete_entity(Box::new(move |_entity_table_id, entity| {
			if let Ok(mut deleter) = delete_query.try_lock() {
				deleter.delete(&entity).expect("Unknown deletion error while deleting valid entity")
			} else {
				panic!("DenseEntityTable<{}, {}> already locked while deleting an entity, all tables must be free when deleting an Entity", std::any::type_name::<EntityType>(), std::any::type_name::<ValueType>());
			};
		}));
		this
	}
}

pub struct InsertQuery<EntityType: Entity, ValueType: 'static> {
	//entity_table: Rc<RefCell<EntityTable<EntityType>>>,
	reverse: Rc<RefCell<SecondaryEntityIndex<EntityType, usize>>>,
	entities: Rc<RefCell<Vec<EntityType>>>,
	values: Rc<RefCell<Vec<ValueType>>>,
}

impl<EntityType: Entity, ValueType: 'static> InsertQuery<EntityType, ValueType> {
	pub fn try_lock(&mut self) -> Result<InsertQueryLocked<EntityType, ValueType>, BorrowMutError> {
		Ok(InsertQueryLocked {
			//entity_table: self.entity_table.try_borrow_mut()?,
			reverse: self.reverse.try_borrow_mut()?,
			entities: self.entities.try_borrow_mut()?,
			values: self.values.try_borrow_mut()?,
		})
	}
}

pub struct InsertQueryLocked<'a, EntityType: Entity, ValueType: 'static> {
	//entity_table: RefMut<'a, EntityTable<EntityType>>,
	reverse: RefMut<'a, SecondaryEntityIndex<EntityType, usize>>,
	entities: RefMut<'a, Vec<EntityType>>,
	values: RefMut<'a, Vec<ValueType>>,
}

impl<'a, EntityType: Entity, ValueType: 'static> InsertQueryLocked<'a, EntityType, ValueType> {
	pub fn insert(
		&mut self,
		entity: &ValidEntity<EntityType>,
		value: ValueType,
	) -> Result<(), SecondaryIndexErrors<EntityType>> {
		let location = self.reverse.insert_mut(entity.raw())?;
		*location = self.entities.len();
		self.entities.push(entity.raw());
		self.values.push(value);
		Ok(())
	}
}

pub struct DeleteQuery<EntityType: Entity, ValueType: 'static> {
	//entity_table: Rc<RefCell<EntityTable<EntityType>>>,
	reverse: Rc<RefCell<SecondaryEntityIndex<EntityType, usize>>>,
	entities: Rc<RefCell<Vec<EntityType>>>,
	values: Rc<RefCell<Vec<ValueType>>>,
}

impl<EntityType: Entity, ValueType: 'static> DeleteQuery<EntityType, ValueType> {
	pub fn try_lock(&mut self) -> Result<DeleteQueryLocked<EntityType, ValueType>, BorrowMutError> {
		Ok(DeleteQueryLocked {
			//entity_table: self.entity_table.try_borrow_mut()?,
			reverse: self.reverse.try_borrow_mut()?,
			entities: self.entities.try_borrow_mut()?,
			values: self.values.try_borrow_mut()?,
		})
	}
}

pub struct DeleteQueryLocked<'a, EntityType: Entity, ValueType: 'static> {
	//entity_table: RefMut<'a, EntityTable<EntityType>>,
	reverse: RefMut<'a, SecondaryEntityIndex<EntityType, usize>>,
	entities: RefMut<'a, Vec<EntityType>>,
	values: RefMut<'a, Vec<ValueType>>,
}

impl<'a, EntityType: Entity, ValueType: 'static> DeleteQueryLocked<'a, EntityType, ValueType> {
	pub fn delete(
		&mut self,
		entity: &ValidEntity<EntityType>,
	) -> Result<(), SecondaryIndexErrors<EntityType>> {
		let location = self.reverse.remove(entity.raw())?;
		self.entities.swap_remove(location);
		self.values.swap_remove(location);
		if self.entities.len() < location {
			let moved = self
				.reverse
				.get_mut(self.entities[location])
				.expect("reverse mapping is in invalid state with DenseEntityValueTable");
			*moved = location
		}
		Ok(())
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

	fn indexes_len(&self) -> usize {
		1
	}

	fn get_index_metadata(&self, idx: usize) -> Option<&dyn IndexField> {
		if idx != 0 {
			return None;
		}

		struct PrimaryKey;
		impl Field for PrimaryKey {}
		impl IndexField for PrimaryKey {}
		static PRIMARY_KEY: PrimaryKey = PrimaryKey;
		Some(&PRIMARY_KEY)
	}
}

impl<EntityType: Entity, ValueType: 'static> TableCastable
	for DenseEntityValueTable<EntityType, ValueType>
{
	fn get_strong_self(&self) -> Rc<RefCell<Self>> {
		self.this.upgrade().unwrap() // It's obviously valid since it's obviously self
	}
}
