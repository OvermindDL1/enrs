#[cfg(feature = "flecs-nightly")]
pub mod flecs;
#[cfg(feature = "legion")]
pub mod legion;
#[cfg(feature = "shipyard")]
pub mod shipyard;

#[cfg(not(feature = "legion"))]
pub mod legion {
	pub fn benchmarks() {}
}

#[cfg(not(feature = "shipyard"))]
pub mod shipyard {
	pub fn benchmarks() {}
}

#[cfg(not(feature = "flecs-nightly"))]
pub mod flecs {
	pub fn benchmarks() {}
}
