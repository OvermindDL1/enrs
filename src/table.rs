pub mod fields;

use std::cell::RefCell;
use std::rc::Rc;

use crate::database::{DatabaseId, TableId};
use crate::table::fields::IndexField;
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
	/// Get's the index count for when calling `get_index_metadata(0..indexes_len())`.
	/// Should always be at least 1 in length to be dynamically accessible.
	fn indexes_len(&self) -> usize;
	/// Index 0 is the primary key and should always exist
	fn get_index_metadata(&self, idx: usize) -> Option<&dyn IndexField>;
}

pub trait TableCastable: 'static {
	fn get_strong_self(&self) -> Rc<RefCell<Self>>;
}

impl dyn Table {
	pub fn get_strong_cast<T: TableCastable>(&self) -> Option<Rc<RefCell<T>>> {
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
	use crate::tables::dense_entity_value_table::DenseEntityValueTable;
	use crate::tables::entity_table::EntityTable;
	use std::sync::atomic::AtomicPtr;

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

	#[test]
	#[should_panic]
	fn cannot_mutate_entities_with_valid_active() {
		let mut database = Database::new();
		assert_eq!(database.tables.len(), 0);
		let entities_table_id = database
			.tables
			.create("entities", EntityTable::<u64>::builder())
			.unwrap();
		let entities_table = database.tables.get_by_id(entities_table_id);
		let entities_storage = entities_table
			.borrow()
			.get_strong_cast::<EntityTable<u64>>()
			.unwrap();
		let entity = entities_storage.borrow_mut().insert();
		// Changing this to a borrow_mut and uncommenting below will not compile because `valid` is holding an immutable reference
		let entities = entities_storage.borrow();
		let valid_entity = entities.valid(entity).unwrap();
		//let another_entity = entities.insert(); // No way to craft this without a mut while a valid is active, see above comment
		let _another_entity = entities_storage.borrow_mut().insert(); // This will panic
		assert_eq!(valid_entity.0, entity);
	}

	#[test]
	fn registration_test() {
		let mut database = Database::new();
		assert_eq!(database.tables.len(), 0);
		let entities_table_id = database
			.tables
			.create("entities", EntityTable::<u64>::builder())
			.unwrap();
		let entities_table = database.tables.get_by_id(entities_table_id);
		let entities_storage = entities_table
			.borrow()
			.get_strong_cast::<EntityTable<u64>>()
			.unwrap();
		let ints_table_id = database
			.tables
			.create("ints", DenseEntityValueTable::<u64, isize>::builder())
			.unwrap();
		let ints_table = database.tables.get_by_id(ints_table_id);
		let ints_storage = ints_table
			.borrow()
			.get_strong_cast::<DenseEntityValueTable<u64, isize>>()
			.unwrap();
		let mut insert_query = ints_storage.borrow().insert_query();
		let mut locked = insert_query.try_lock().unwrap();
		let entity = entities_storage.borrow_mut().insert();
		assert_eq!(entity, 1);
		locked
			.insert(entities_storage.borrow().valid(entity).unwrap(), 42)
			.unwrap();
		// AtomicPtr::
		// use crate::table::Table;
		// use bitvec::prelude::*;
		// use crossbeam::atomic::*;
		// let cell = bitvec![42];
		// assert!(AtomicCell::<&dyn Table>::is_lock_free());
	}

	#[test]
	fn blah_test() {
		// use bitvec::prelude::*;
		// use rayon::prelude::*;
		// use std::cell::*;
		// let mut bitvecs = vec![bitvec![8]; 64];
		// let bitslices = bitvecs.as_mut_slice();
		// let bitcellslices = UnsafeCell::from_mut(bitslices);
		// let mut bitslicescells = bitcellslices.as_slice_of_cells();
		// (0..64)
		// 	.collect::<Vec<usize>>()
		// 	.par_iter()
		// 	.for_each(|&i| bitslicescells[i].get_mut().set(i, true));
		// dbg!(bitvecs);
	}
}
