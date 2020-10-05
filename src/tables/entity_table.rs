use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::{Rc, Weak};

use smol_str::SmolStr;

use crate::database::{DatabaseId, TableId};
use crate::entity::Entity;
use crate::table::fields::{Field, IndexField};
use crate::table::{Table, TableBuilder, TableCastable};
// use bitvec::prelude::*;
use std::any::Any;
use std::ops::Deref;
// use tinyvec::TinyVec;

// #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
// pub struct EventListenerId<Type>(u16, PhantomData<Type>);
//
// pub struct EventIndexedHandler<Type, ListenerType> {
// 	listeners: Vec<ListenerType>,
// 	registrations: Vec<TinyVec<[u16; 16]>>,
// 	_phantom: PhantomData<Type>,
// }
//
// impl<Type, ListenerType> EventIndexedHandler<Type, ListenerType> {
// 	fn with_capacity(capacity: usize) -> Self {
// 		Self {
// 			listeners: vec![],
// 			registrations: Vec::with_capacity(capacity),
// 			_phantom: PhantomData,
// 		}
// 	}
//
// 	fn push(&mut self) {
// 		self.registrations.push(Default::default());
// 	}
//
// 	pub fn add(&mut self, f: ListenerType) -> EventListenerId<Type> {
// 		assert!(self.listeners.len() < u16::MAX as usize-1);
// 		self.listeners.push(f);
// 		EventListenerId((self.listeners.len() - 1) as u16, PhantomData)
// 	}
//
// 	pub fn register(&mut self, listener_id: EventListenerId<Type>, idx: usize) {
// 		self.registrations[idx].push(listener_id.0);
// 	}
//
// 	pub fn unregister(&mut self, listener_id: EventListenerId<Type>, idx: usize) {
// 		let mut listener_idx =
// 		self.registrations[idx].swap_remove()
// 	}
// }

pub struct EntityTable<EntityType: Entity> {
	this: Weak<RefCell<Self>>,
	database_id: DatabaseId,
	table_name: SmolStr,
	table_id: TableId,
	on_delete: Vec<Box<dyn FnMut(TableId, ValidEntity<EntityType>)>>,
	// pub on_destroy: EventIndexedHandler<Box<dyn Fn(TableId, &[EntityType])>>,
	//registrations_destroy: Vec<BitVec>,
	/// `entities` is interesting in that alive ones have their internal index
	/// match their actual index, if it's dead they don't.  If it's dead the
	/// internal index actually points to the actual index of the next 'dead'
	/// one, thus making a handle-based link-list.  If it points to
	/// `0` then there are no more dead entities after this one.
	/// The generation gets incremented on destruction.
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

	pub fn on_delete_entity(
		&mut self,
		f: Box<dyn FnMut(TableId, ValidEntity<EntityType>)>,
	) -> usize {
		self.on_delete.push(f);
		self.on_delete.len() - 1
	}

	pub fn contains(&self, entity: EntityType) -> bool {
		let idx = entity.idx();
		(idx < self.entities.len()) && self.entities[idx] == entity
	}

	pub fn valid(&self, entity: EntityType) -> Option<ValidEntity<EntityType>> {
		if self.contains(entity) {
			Some(ValidEntity(entity, PhantomData))
		} else {
			None
		}
	}

	pub fn insert(&mut self) -> ValidEntity<EntityType> {
		if self.destroyed.is_null() {
			// `destroyed` linked list is empty
			let entity = EntityType::new(self.entities.len());
			self.entities.push(entity);
			ValidEntity(entity, PhantomData)
		} else {
			let head = self.destroyed.idx();
			// TODO:  This should be safe to make unsafe and use `get_unchecked`
			let head_entity = &mut self.entities[head];
			self.destroyed = EntityType::new(head_entity.idx()); // New head of destroyed list
			ValidEntity(*head_entity.set_idx(head), PhantomData)
		}
	}

	pub fn delete(&mut self, entity: EntityType) -> Result<(), ()> {
		let idx = entity.idx();
		if idx >= self.entities.len() || self.entities[idx] != entity {
			return Err(());
		}

		(&mut self.entities[idx]).bump_version_with_idx(self.destroyed.idx());
		self.destroyed = EntityType::new(idx);

		//let listeners = &self.registrations_destroy[idx];
		//for listener_id in listeners.ite {}
		//self.registrations.destroy.iter();
		for cb in self.on_delete.iter_mut() {
			cb(self.table_id, ValidEntity(entity, PhantomData));
		}

		Ok(())
	}
}

pub struct ValidEntity<'a, EntityType: Entity>(EntityType, PhantomData<&'a ()>);

impl<'a, EntityType: Entity> Deref for ValidEntity<'a, EntityType> {
	type Target = EntityType;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl<'a, EntityType: Entity> ValidEntity<'a, EntityType> {
	pub fn raw(&self) -> EntityType {
		self.0
	}
}

impl<EntityType: Entity> TableBuilder for EntityTableBuilder<EntityType> {
	type Table = EntityTable<EntityType>;

	fn build(
		self,
		database_id: DatabaseId,
		table_name: &str,
		table_id: TableId,
	) -> Rc<RefCell<Self::Table>> {
		let this = Rc::new(RefCell::new(EntityTable {
			this: Weak::new(),
			database_id,
			table_name: table_name.into(),
			table_id,
			on_delete: Vec::with_capacity(self.capacity),
			//on_destroy: EventIndexedHandler::with_capacity(self.capacity),
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

impl<EntityType: Entity> TableCastable for EntityTable<EntityType> {
	fn get_strong_self(&self) -> Rc<RefCell<Self>> {
		self.this.upgrade().unwrap() // It's obviously valid since it's obviously self
	}
}
