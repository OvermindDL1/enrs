pub mod entity;
pub mod registry;
pub mod typed_index_map;

mod instances {
	pub use crate as enrs;
	use crate::unsigned_integral_entity;
	unsigned_integral_entity!(
		u16,
		u8,
		0x0_FFF,
		0xF_000,
		12,
		"`u16` Entity, Index: 12 bits, Generation: 4 bits, Invalid ID: 0, Max: 4095"
	);
	unsigned_integral_entity!(
		u32,
		u16,
		0x000_FFFFF,
		0xFFF_00000,
		20,
		"`u32` Entity, Index: 20 bits, Generation: 12 bits, Invalid ID: 0, Max: 1048575"
	);
	unsigned_integral_entity!(
		u64,
		u32,
		0x00000000_FFFFFFFF,
		0xFFFFFFFF_00000000,
		32,
		"`u64` Entity, Index: 32 bits, Generation: 32 bits, Invalid ID: 0, Max: 4294967295"
	);
}
