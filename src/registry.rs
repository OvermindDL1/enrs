pub use crate::entity::Entity;

pub struct Registry<EntityType> {
	_e: EntityType,
}

impl<EntityType: Entity> Registry<EntityType> {
	#[allow(clippy::new_without_default)]
	pub fn new() -> Registry<EntityType> {
		Registry {
			_e: EntityType::new(0),
		}
	}
}

#[cfg(test)]
mod tests {
	//use super::*;

	#[test]
	fn create_static_components() {
		// let mut registry = Registry::<u32>::new();
		// let _ = registry.create::<()>();
		// let _ = registry.create::<(usize, ())>();
	}
}
