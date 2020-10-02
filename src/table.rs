use std::cell::RefCell;
use std::rc::Rc;

use crate::database::{DatabaseId, TableId};
// use smol_str::SmolStr;

// pub struct TableMetadata {
// 	database_id: DatabaseId,
// 	table_name: SmolStr,
// 	table_id: TableId,
// }
//
// impl TableMetadata {
// 	fn database_id(&self) -> DatabaseId {
// 		self.database_id
// 	}
//
// 	fn table_name(&self) -> &str {
// 		&self.table_name
// 	}
//
// 	fn table_id(&self) -> TableId {
// 		self.table_id
// 	}
// }

pub trait TableBuilder {
	fn build(
		self,
		database_id: DatabaseId,
		table_name: &str,
		table_id: TableId,
	) -> Rc<RefCell<dyn Table>>;
}

pub trait Table: 'static {
	fn as_any(&self) -> &dyn std::any::Any;
	fn get_strong(&self) -> Rc<RefCell<dyn Table>>;
	fn get_database_id(&self) -> DatabaseId;
	fn table_name(&self) -> &str;
	fn table_id(&self) -> TableId;
	fn indexes_len(&self) -> usize;
}

pub trait TableCastable: 'static {
	fn get_strong_self(&self) -> Rc<RefCell<Self>>;
}

impl dyn Table {
	fn get_strong_cast<T: TableCastable>(&self) -> Option<Rc<RefCell<T>>> {
		if let Some(blah) = self.as_any().downcast_ref::<T>() {
			Some(blah.get_strong_self())
		} else {
			None
		}
	}
}

#[cfg(test)]
mod tests {
	use crate::database::*;
	use crate::tables::entity_table::EntityTable;

	#[test]
	fn get_strong() {
		let mut database = Database::new();
		assert_eq!(database.tables.len(), 0);
		let entities_table_id = database
			.tables
			.create("entities", EntityTable::<u64>::builder())
			.unwrap();
		let entities_table = database.tables.get_by_id(entities_table_id);
		assert_eq!(entities_table.borrow().indexes_len(), 1);
		let entities_storage = entities_table
			.borrow()
			.get_strong_cast::<EntityTable<u64>>()
			.unwrap();
		let entity = entities_storage.borrow_mut().insert();
		assert!(entities_storage.borrow().contains(entity));
		assert_eq!(entity, 1);
	}
}
