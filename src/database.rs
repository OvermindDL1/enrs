use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic;
use std::sync::atomic::AtomicUsize;

use indexmap::map::IndexMap;
use smol_str::SmolStr;

use crate::table::{Table, TableBuilder};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TableId(usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DatabaseId(usize);

#[derive(Debug, PartialEq, Eq)]
pub enum DatabaseErrors {
	TableNameAlreadyExists(SmolStr),
	TableDoesNotExistWithName(SmolStr),
}

impl std::fmt::Display for DatabaseErrors {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
		use DatabaseErrors::*;
		match self {
			TableNameAlreadyExists(name) => write!(f, "Table name `{}` already exists", name),
			TableDoesNotExistWithName(name) => write!(f, "Table name `{}` does not exist", name),
		}
	}
}

impl std::error::Error for DatabaseErrors {
	fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
		use DatabaseErrors::*;
		match self {
			TableNameAlreadyExists(_name) => None,
			TableDoesNotExistWithName(_name) => None,
		}
	}
}

// mod private {
// 	pub(super) trait Sealed {}
// }

struct TableWrapper {}

pub struct Tables {
	database_id: DatabaseId,
	mapping: IndexMap<SmolStr, Rc<RefCell<dyn Table>>>,
}

impl Tables {
	fn new(database_id: DatabaseId) -> Self {
		Self {
			database_id,
			mapping: IndexMap::default(),
		}
	}

	pub fn len(&self) -> usize {
		self.mapping.len()
	}

	pub fn is_empty(&self) -> bool {
		self.mapping.is_empty()
	}

	pub fn create(
		&mut self,
		name: impl Into<SmolStr>,
		table_builder: impl TableBuilder,
	) -> Result<TableId, DatabaseErrors> {
		let name: SmolStr = name.into();
		if self.mapping.contains_key(&name) {
			return Err(DatabaseErrors::TableNameAlreadyExists(name));
		}
		let table = table_builder.build(self.database_id, &name, TableId(self.mapping.len()));
		assert_eq!(table.borrow().get_database_id(), self.database_id);
		let (idx, old_value) = self.mapping.insert_full(name, table);
		assert!(old_value.is_none());
		Ok(TableId(idx))
	}

	pub fn get_by_id(&self, id: TableId) -> Rc<RefCell<dyn Table>> {
		if let Some((_name, table)) = self.mapping.get_index(id.0) {
			table.clone()
		} else {
			panic!("passed in an invalid TableId to a Database, this signifies an fatal programming error as a TableId from one Database should not be used on another Database")
		}
	}

	pub fn get_by_name(&self, name: &str) -> Result<Rc<RefCell<dyn Table>>, DatabaseErrors> {
		if let Some(table) = self.mapping.get(name) {
			Ok(table.clone())
		} else {
			Err(DatabaseErrors::TableDoesNotExistWithName(name.into()))
		}
	}
}

static DATABASE_IDS: AtomicUsize = AtomicUsize::new(0);

pub struct Database {
	uid: DatabaseId,
	pub tables: Tables,
}

impl Default for Database {
	fn default() -> Self {
		let uid = DatabaseId(DATABASE_IDS.fetch_add(1, atomic::Ordering::Relaxed));
		Database {
			uid,
			tables: Tables::new(uid),
		}
	}
}

impl Database {
	pub fn new() -> Database {
		Database::default()
	}
}

#[cfg(test)]
mod tests {
	use crate::database::*;
	use crate::tables::entity_table::EntityTable;

	#[test]
	fn initialize() {
		let database = Database::new();
		assert_eq!(database.tables.len(), 0);
	}

	#[test]
	fn table_create() {
		let mut database = Database::new();
		assert_eq!(database.tables.len(), 0);
		let entities_table_id = database
			.tables
			.create("entities", EntityTable::<u64>::builder())
			.unwrap();
		assert_eq!(database.tables.len(), 1);
		let entities_by_id = database.tables.get_by_id(entities_table_id);
		let entities_by_name = database.tables.get_by_name("entities").unwrap();
		assert_eq!(
			entities_by_id.borrow().table_name(),
			entities_by_name.borrow().table_name()
		);
		assert_eq!(entities_by_id.borrow().table_name(), "entities");
		assert_eq!(entities_by_id.borrow().table_name(), "entities");
		assert_eq!(
			entities_by_id.borrow().table_id(),
			entities_by_name.borrow().table_id()
		);
		assert_eq!(entities_by_id.borrow().table_id(), entities_table_id);
		assert_eq!(entities_by_name.borrow().table_id(), entities_table_id);
	}
}
