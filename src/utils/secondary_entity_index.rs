use crate::entity::Entity;
use std::marker::PhantomData;

const PER_PAGE: usize = u8::MAX as usize + 1; // 256; // Should only have a single bit set

#[derive(Debug, PartialEq, Eq)]
pub enum SecondaryEntityIndexErrors<EntityType: Entity> {
	IndexAlreadyExists(EntityType),
	IndexDoesNotExist(EntityType),
}

impl<EntityType: Entity> std::error::Error for SecondaryEntityIndexErrors<EntityType> {
	fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
		use SecondaryEntityIndexErrors::*;
		match self {
			IndexAlreadyExists(_entity) => None,
			IndexDoesNotExist(_entity) => None,
		}
	}
}

impl<EntityType: Entity> std::fmt::Display for SecondaryEntityIndexErrors<EntityType> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
		use SecondaryEntityIndexErrors::*;
		match self {
			IndexAlreadyExists(entity) => write!(f, "Index already exists: {:?}", entity),
			IndexDoesNotExist(entity) => write!(f, "Index does not exist: {:?}", entity),
		}
	}
}

#[derive(Clone)]
pub struct SecondaryEntityIndex<EntityType: Entity, IndexType: Copy + PartialEq> {
	invalid_index: IndexType,
	pages: Vec<Option<Box<[IndexType; PER_PAGE]>>>,
	_phantom: PhantomData<EntityType>,
}

impl<EntityType: Entity, IndexType: Copy + PartialEq> SecondaryEntityIndex<EntityType, IndexType> {
	pub fn new(invalid_index: IndexType) -> Self {
		Self {
			invalid_index,
			pages: vec![],
			_phantom: Default::default(),
		}
	}

	#[inline]
	fn page(entity: EntityType) -> usize {
		entity.idx() / PER_PAGE
	}

	#[inline]
	fn offset(entity: EntityType) -> u8 {
		(entity.idx() & (PER_PAGE - 1)) as u8
	}

	#[inline]
	fn page_offset(entity: EntityType) -> (usize, u8) {
		(Self::page(entity), Self::offset(entity))
	}

	pub fn insert_mut(
		&mut self,
		entity: EntityType,
	) -> Result<&mut IndexType, SecondaryEntityIndexErrors<EntityType>> {
		let (page_idx, offset) = Self::page_offset(entity);

		if page_idx >= self.pages.len() {
			self.pages.resize(page_idx + 1, None);
		}
		let invalid_index = &self.invalid_index;
		let page = self.pages[page_idx].get_or_insert_with(|| Box::new([*invalid_index; PER_PAGE]));

		let location = &mut page[offset as usize];
		if *location != self.invalid_index {
			return Err(SecondaryEntityIndexErrors::IndexAlreadyExists(entity));
		}

		Ok(location)
	}

	pub fn get(
		&self,
		entity: EntityType,
	) -> Result<&IndexType, SecondaryEntityIndexErrors<EntityType>> {
		let (page_idx, offset) = Self::page_offset(entity);

		if page_idx >= self.pages.len() {
			return Err(SecondaryEntityIndexErrors::IndexDoesNotExist(entity));
		}
		let page = if let Some(page) = &self.pages[page_idx] {
			page
		} else {
			return Err(SecondaryEntityIndexErrors::IndexDoesNotExist(entity));
		};

		let location = &page[offset as usize];
		if *location == self.invalid_index {
			return Err(SecondaryEntityIndexErrors::IndexDoesNotExist(entity));
		}

		Ok(location)
	}

	pub fn get_mut(
		&mut self,
		entity: EntityType,
	) -> Result<&mut IndexType, SecondaryEntityIndexErrors<EntityType>> {
		let (page_idx, offset) = Self::page_offset(entity);

		if page_idx >= self.pages.len() {
			return Err(SecondaryEntityIndexErrors::IndexDoesNotExist(entity));
		}
		let page = if let Some(page) = &mut self.pages[page_idx] {
			page
		} else {
			return Err(SecondaryEntityIndexErrors::IndexDoesNotExist(entity));
		};

		let location = &mut page[offset as usize];
		if *location == self.invalid_index {
			return Err(SecondaryEntityIndexErrors::IndexDoesNotExist(entity));
		}

		Ok(location)
	}

	// pub fn remove(
	// 	&mut self,
	// 	entity: EntityType,
	// ) -> Result<IndexType, SecondaryIndexErrors<EntityType>> {
	// 	let (page_idx, offset) = Self::page_offset(entity);
	//
	// 	if page_idx >= self.pages.len() {
	// 		return Err(SecondaryIndexErrors::IndexDoesNotExist(entity));
	// 	}
	// 	let page = if let Some(page) = &mut self.pages[page_idx] {
	// 		page
	// 	} else {
	// 		return Err(SecondaryIndexErrors::IndexDoesNotExist(entity));
	// 	};
	//
	// 	let location = &mut page[offset as usize];
	// 	if *location == self.invalid_index {
	// 		return Err(SecondaryIndexErrors::IndexDoesNotExist(entity));
	// 	}
	//
	// 	let ret = Ok(*location);
	// 	*location = self.invalid_index;
	// 	ret
	// }

	// pub fn remove_iter(&mut self, entities: impl IntoIterator<Item = EntityType>) {
	// 	for entity in entities {
	// 		self.remove(entity)
	// 			.expect("Attempted to remove an entity when it was not already existing");
	// 	}
	// }
}
