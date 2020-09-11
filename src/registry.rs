pub use crate::entity::Entity;

pub struct Registry<EntityType> {
	e: EntityType,
}

impl<EntityType: Entity> Registry<EntityType> {
	#[warn(clippy::new_without_default)]
	pub fn new() -> Registry<EntityType> {
		Registry {
			e: EntityType::new(0),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn create_static_components() {
		let mut registry = Registry::<u32>::new();
		// let _ = registry.create::<()>();
		// let _ = registry.create::<(usize, ())>();
	}
}
