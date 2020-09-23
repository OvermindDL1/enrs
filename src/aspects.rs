use std::marker::PhantomData;

use crate::frunk::HNil;

use crate::storages::TypeList;

pub struct Aspects<RO: TypeList, RW: TypeList, E: TypeList> {
	read_only: PhantomData<RO>,
	read_write: PhantomData<RW>,
	except: PhantomData<E>,
}

impl Default for Aspects<HNil, HNil, HNil> {
	fn default() -> Self {
		Self {
			read_only: Default::default(),
			read_write: Default::default(),
			except: Default::default(),
		}
	}
}

impl Aspects<HNil, HNil, HNil> {
	pub fn new() -> Self {
		Self::default()
	}
}

impl<RO: TypeList, RW: TypeList, E: TypeList> Aspects<RO, RW, E> {
	pub fn len_read_only(&self) -> usize {
		RO::LEN
	}
	pub fn len_read_write(&self) -> usize {
		RW::LEN
	}
	pub fn len_except(&self) -> usize {
		E::LEN
	}
}

#[cfg(test)]
mod tests {
	//use crate::frunk::hlist;

	use super::*;

	#[test]
	fn creation() {
		let aspect_nil = Aspects::default();
		assert_eq!(aspect_nil.len_read_only(), 0);
		assert_eq!(aspect_nil.len_read_write(), 0);
		assert_eq!(aspect_nil.len_except(), 0);
	}
}
