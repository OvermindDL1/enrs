use super::flecs::*;
use rental::__rental_prelude::PhantomData;

pub struct World(*mut ecs_world_t);

impl World {
	pub fn new() -> World {
		unsafe { World(ecs_init()) }
	}

	pub fn entity(&mut self) -> Entity {
		//let e = unsafe { ecs_new_w_type(self.0, 0 as _) };
		let e = unsafe { ecs_new_id(self.0) };
		Entity(self.0, e, PhantomData)
	}
}

impl Drop for World {
	fn drop(&mut self) {
		unsafe {
			ecs_fini(self.0);
		}
	}
}

pub struct Entity<'s>(*mut ecs_world_t, ecs_entity_t, PhantomData<&'s ()>);

impl<'s> Entity<'s> {}
