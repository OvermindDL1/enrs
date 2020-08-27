/// Entity Type Trait to allow for a variety of entity storages to be used.
///
/// Can make a trivial tuple wrapper with the `delegate_wrapped_entity!` macro:
///
/// ```
/// # use enrs::{delegate_wrapped_entity, entity::Entity};
/// #[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
/// struct Wrapper(u32);
/// delegate_wrapped_entity!(Wrapper, u32);
/// let mut e = Wrapper::new(42);
/// assert_eq!(e.idx(), 42);
/// assert_eq!(e.version(), 0);
/// e.bump_version_with_idx(16);
/// assert_eq!(e.idx(), 16);
/// assert_eq!(e.version(), 1);
/// assert_eq!(e.0, 1048592);
/// ```
pub trait Entity: PartialEq + Copy + Ord {
	/// The actual container type of this entity date, it should be Copy, and thus cheap to Copy.
	type StorageType;

	/// The type returned to hold the version, smaller than the StorageType in general.
	type VersionType;
	//	type DifferenceType;
	//	const ENTITY_MASK: Self::StorageType;
	//	const VERSION_MASK: Self::StorageType;
	//	const ENTITY_SHIFT: u32;
	///// Must be what `Self::new(0)` returns.
	//const NULL: Self;
	/// Constructs an Entity Handle using the given ID and a 0 version
	fn new(id: usize) -> Self;
	//	fn to_integral(self) -> Self::StorageType;
	/// Return true if this entity is index 0
	fn is_null(self) -> bool;
	//	fn id(self) -> Self::StorageType;
	/// Returns the index of this entity
	fn idx(self) -> usize;
	/// Sets the index of this entity
	fn set_idx(&mut self, idx: usize) -> &mut Self;
	/// Returns the version of this entity, generally a number but it could potentially be something else, like a UUID
	fn version(self) -> Self::VersionType;
	/// Sets the index as well as increments the version in a single call
	fn bump_version_with_idx(&mut self, idx: usize);
	//	fn new_next_version(self) -> Self;
	//	fn set_version(&mut self, version: Self::StorageType) {
	//		*self = self.new_version(version);
	//	}
	//	fn next_version(&mut self) {
	//		*self = self.new_next_version()
	//	}
}

#[macro_export]
macro_rules! unsigned_integral_entity {
	($INT:ident, $INT_VERSION:ident, $INDEX_MASK:literal, $VERSION_MASK:literal, $SHIFT_BITS:literal, $DOC:literal) => {
		#[doc=$DOC]
		impl enrs::entity::Entity for $INT {
			type StorageType = $INT;
			type VersionType = $INT_VERSION;

			fn new(idx: usize) -> Self {
				#[cfg(not(enrs_disable_asserts))]
				assert!(idx <= $INDEX_MASK);
				idx as Self::StorageType
			}

			#[allow(clippy::verbose_bit_mask)]
			fn is_null(self) -> bool {
				(self & $INDEX_MASK) == 0
			}

			fn idx(self) -> usize {
				(self & $INDEX_MASK) as usize
			}

			fn set_idx(&mut self, idx: usize) -> &mut Self {
				#[cfg(not(enrs_disable_asserts))]
				assert!(idx <= $INDEX_MASK);
				*self = (*self & $VERSION_MASK) | (idx as Self::StorageType);
				self
			}

			fn version(self) -> Self::VersionType {
				(self & $VERSION_MASK).wrapping_shr($SHIFT_BITS) as Self::VersionType
			}

			fn bump_version_with_idx(&mut self, idx: usize) {
				#[cfg(not(enrs_disable_asserts))]
				assert!(idx <= $INDEX_MASK);
				*self = ((*self & $VERSION_MASK).wrapping_shr($SHIFT_BITS) + 1)
					.wrapping_shl($SHIFT_BITS)
					+ (idx as Self::StorageType);
			}
		}
	};
}

/// Can make a trivial tuple1 wrapper with the `delegate_wrapped_entity!` macro around another valid
/// type:
///
/// ```
/// # use enrs::{delegate_wrapped_entity, entity::Entity};
/// #[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
/// struct Wrapper(u32);
/// delegate_wrapped_entity!(Wrapper, u32);
/// let mut e = Wrapper::new(42);
/// assert_eq!(e.idx(), 42);
/// assert_eq!(e.version(), 0);
/// e.bump_version_with_idx(16);
/// assert_eq!(e.idx(), 16);
/// assert_eq!(e.version(), 1);
/// assert_eq!(e.0, 1048592);
/// ```
#[macro_export]
macro_rules! delegate_wrapped_entity {
	($SELF:ident, $INTERNAL:ident) => {
		impl enrs::entity::Entity for $SELF {
			type StorageType = <$INTERNAL as enrs::entity::Entity>::StorageType;
			type VersionType = <$INTERNAL as enrs::entity::Entity>::VersionType;

			fn new(idx: usize) -> Self {
				$SELF(<$INTERNAL as enrs::entity::Entity>::new(idx))
			}

			#[allow(clippy::verbose_bit_mask)]
			fn is_null(self) -> bool {
				self.0.is_null()
			}

			fn idx(self) -> usize {
				self.0.idx()
			}

			fn set_idx(&mut self, idx: usize) -> &mut Self {
				self.0.set_idx(idx);
				self
			}

			fn version(self) -> Self::VersionType {
				self.0.version()
			}

			fn bump_version_with_idx(&mut self, idx: usize) {
				self.0.bump_version_with_idx(idx)
			}
		}
	};
}
