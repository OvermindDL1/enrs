mod insertion;
mod other_ecs;
mod storages;

mod components {
	use enrs::{tl, TL};

	pub struct A(pub u64);
	pub struct B(pub u64);
	pub struct C(pub u64);
	pub struct D(pub u64);
	pub struct E(pub u64);
	pub struct F(pub u64);
	pub struct G(pub u64);
	pub struct H(pub u64);
	pub struct I(pub u64);
	pub struct J(pub u64);
	pub struct K(pub u64);
	pub struct L(pub u64);
	pub struct M(pub u64);
	pub struct N(pub u64);
	pub struct O(pub u64);
	pub struct P(pub u64);

	pub type Type4 = TL![
		&'static mut A,
		&'static mut B,
		&'static mut C,
		&'static mut D
	];
	pub type Type8 = TL![
		&'static mut A,
		&'static mut B,
		&'static mut C,
		&'static mut D,
		&'static mut E,
		&'static mut F,
		&'static mut G,
		&'static mut H
	];
	pub type Type16 = TL![
		&'static mut A,
		&'static mut B,
		&'static mut C,
		&'static mut D,
		&'static mut E,
		&'static mut F,
		&'static mut G,
		&'static mut H,
		&'static mut I,
		&'static mut J,
		&'static mut K,
		&'static mut L,
		&'static mut M,
		&'static mut N,
		&'static mut O,
		&'static mut P
	];

	pub fn type4_new(v: u64) -> TL![A, B, C, D] {
		tl![A(v), B(v), C(v), D(v)]
	}

	pub fn type4_new_flat(v: u64) -> (A, B, C, D) {
		(A(v), B(v), C(v), D(v))
	}

	pub fn type8_new(v: u64) -> TL![A, B, C, D, E, F, G, H] {
		tl![A(v), B(v), C(v), D(v), E(v), F(v), G(v), H(v)]
	}

	pub fn type8_new_flat(v: u64) -> (A, B, C, D, E, F, G, H) {
		(A(v), B(v), C(v), D(v), E(v), F(v), G(v), H(v))
	}

	pub fn type16_new(v: u64) -> TL![A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P] {
		tl![
			A(v),
			B(v),
			C(v),
			D(v),
			E(v),
			F(v),
			G(v),
			H(v),
			I(v),
			J(v),
			K(v),
			L(v),
			M(v),
			N(v),
			O(v),
			P(v)
		]
	}

	pub fn type16_new_flat(v: u64) -> (A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P) {
		(
			A(v),
			B(v),
			C(v),
			D(v),
			E(v),
			F(v),
			G(v),
			H(v),
			I(v),
			J(v),
			K(v),
			L(v),
			M(v),
			N(v),
			O(v),
			P(v),
		)
	}
}

criterion::criterion_main! {
	storages::entity_table::benchmarks,
	storages::dense_entity_dynamic_paged_multi_value_table::benchmarks,
	storages::simple_storages::benchmarks,
	other_ecs::shipyard::benchmarks,
	other_ecs::legion::benchmarks,
	//insertion::insertion,
}
