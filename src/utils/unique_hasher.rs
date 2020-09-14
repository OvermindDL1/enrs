//! Pass-through Hasher for unique values of i/u8, i/u16, i/u32, i/u64, or i/usize.
pub struct UniqueHasher {
	result: u64,
}

impl UniqueHasher {
	pub const fn new() -> Self {
		Self { result: 0 }
	}

	#[inline]
	pub fn add(&mut self, val: u64) {
		debug_assert_eq!(self.result, 0); // Should only ever be called once
		self.result = val;
	}
}

impl core::hash::Hasher for UniqueHasher {
	#[inline]
	fn finish(&self) -> u64 {
		self.result
	}

	#[inline]
	fn write(&mut self, _: &[u8]) {
		panic!("should not be used");
	}

	#[inline]
	fn write_u8(&mut self, i: u8) {
		self.add(i.into());
	}

	#[inline]
	fn write_u16(&mut self, i: u16) {
		self.add(i.into());
	}

	#[inline]
	fn write_u32(&mut self, i: u32) {
		self.add(i.into());
	}

	#[inline]
	fn write_u64(&mut self, i: u64) {
		self.add(i.into());
	}

	#[inline]
	fn write_usize(&mut self, i: usize) {
		self.add(i as u64)
	}

	#[inline]
	fn write_i8(&mut self, i: i8) {
		self.add(i as u64);
	}

	#[inline]
	fn write_i16(&mut self, i: i16) {
		self.add(i as u64);
	}

	#[inline]
	fn write_i32(&mut self, i: i32) {
		self.add(i as u64);
	}

	#[inline]
	fn write_i64(&mut self, i: i64) {
		self.add(i as u64);
	}

	#[inline]
	fn write_isize(&mut self, i: isize) {
		self.add(i as u64);
	}
}

pub struct UniqueHasherBuilder;

impl core::hash::BuildHasher for UniqueHasherBuilder {
	type Hasher = UniqueHasher;

	#[inline]
	fn build_hasher(&self) -> Self::Hasher {
		UniqueHasher::new()
	}
}
