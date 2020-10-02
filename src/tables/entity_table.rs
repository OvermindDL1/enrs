use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::{Rc, Weak};

use smol_str::SmolStr;

use crate::database::{DatabaseId, TableId};
use crate::entity::Entity;
use crate::table::{Table, TableBuilder, TableCastable};
use std::any::Any;

pub struct EntityTable<EntityType: Entity> {
	this: Weak<RefCell<Self>>,
	database_id: DatabaseId,
	table_name: SmolStr,
	table_id: TableId,
	/// `entities` is interesting in that alive ones have their internal index
	/// match their actual index, if it's dead they don't.  If it's dead the
	/// internal index actually points to the actual index of the next 'dead'
	/// one, thus making a handle-based link-list.  If it points to
	/// `0` then there are no more dead entities and a new one needs to be
	/// created.  The generation gets incremented on destruction.
	entities: Vec<EntityType>,
	/// This is the 'head' of the singly-linked list of destroyed entities.
	destroyed: EntityType,
}

pub struct EntityTableBuilder<EntityType: Entity> {
	capacity: usize,
	_phantom: PhantomData<EntityType>,
}

impl<EntityType: Entity> EntityTable<EntityType> {
	pub fn builder() -> EntityTableBuilder<EntityType> {
		EntityTableBuilder {
			capacity: 0,
			_phantom: PhantomData,
		}
	}

	pub fn builder_with_capacity(capacity: usize) -> EntityTableBuilder<EntityType> {
		EntityTableBuilder {
			capacity,
			_phantom: PhantomData,
		}
	}

	pub fn contains(&self, entity: EntityType) -> bool {
		let idx = entity.idx();
		(idx < self.entities.len()) && self.entities[idx] == entity
	}

	pub fn insert(&mut self) -> EntityType {
		if self.destroyed.is_null() {
			// `destroyed` linked list is empty
			let entity = EntityType::new(self.entities.len());
			self.entities.push(entity);
			entity
		} else {
			let head = self.destroyed.idx();
			// TODO:  This should be safe to make unsafe and use `get_unchecked`
			let head_entity = &mut self.entities[head];
			self.destroyed = EntityType::new(head_entity.idx()); // New head of destroyed list
			*head_entity.set_idx(head)
		}
	}
}

impl<EntityType: Entity> TableBuilder for EntityTableBuilder<EntityType> {
	fn build(
		self,
		database_id: DatabaseId,
		table_name: &str,
		table_id: TableId,
	) -> Rc<RefCell<dyn Table>> {
		let this = Rc::new(RefCell::new(EntityTable {
			this: Weak::new(),
			database_id,
			table_name: table_name.into(),
			table_id,
			entities: Vec::with_capacity(self.capacity),
			destroyed: EntityType::new(0),
		}));
		this.borrow_mut().entities.push(EntityType::new(0));
		this.borrow_mut().this = Rc::downgrade(&this);
		this
	}
}

impl<EntityType: Entity> Table for EntityTable<EntityType> {
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
}

impl<EntityType: Entity> TableCastable for EntityTable<EntityType> {
	fn get_strong_self(&self) -> Rc<RefCell<Self>> {
		self.this.upgrade().unwrap() // It's obviously valid since it's obviously us
	}
}
