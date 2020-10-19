use std::time::Instant;

use criterion::*;

use crate::components::*;
use crate::utils::flecs_rust::*;

// ECS_COMPONENT_DECLARE!(AC, AT);

fn world_init() -> World {
	let w = World::new();

	// ECS_COMPONENT_DEFINE!(w, A, AC, AT);
	// ECS_COMPONENT_DEFINE!(w, B, BC, BT);
	// ECS_COMPONENT_DEFINE!(w, C, CC, CT);
	// ECS_COMPONENT_DEFINE!(w, D, DC, DT);
	// ECS_COMPONENT_DEFINE!(w, E, EC, ET);
	// ECS_COMPONENT_DEFINE!(w, F, FC, FT);
	// ECS_COMPONENT_DEFINE!(w, G, GC, GT);
	// ECS_COMPONENT_DEFINE!(w, H, HC, HT);
	// ECS_COMPONENT_DEFINE!(w, I, IC, IT);
	// ECS_COMPONENT_DEFINE!(w, J, JC, JT);
	// ECS_COMPONENT_DEFINE!(w, K, KC, KT);
	// ECS_COMPONENT_DEFINE!(w, L, LC, LT);
	// ECS_COMPONENT_DEFINE!(w, M, MC, MT);
	// ECS_COMPONENT_DEFINE!(w, N, NC, NT);
	// ECS_COMPONENT_DEFINE!(w, O, OC, OT);
	// ECS_COMPONENT_DEFINE!(w, P, PC, PT);

	// ECS_TYPE_DEFINE(w, T4, T4C, T4T, Comp_A, Comp_B, Comp_C, Comp_D);

	w
}

fn entity_table(c: &mut Criterion) {
	let mut group = c.benchmark_group("other_ecs/flecs/EntityTable<u64>");
	group.bench_function("insert", move |b| {
		b.iter_custom(|times| {
			let mut w = world_init();
			let start = Instant::now();
			for _i in 0..times {
				black_box(w.entity());
			}
			start.elapsed()
		});
	});
	/*
	group.bench_function("insert/recycled", move |b| {
		b.iter_custom(|times| {
			let mut world = World::default();
			let entities: Vec<_> = (0..times).map(|_| world.push(())).collect();
			for e in entities {
				world.remove(e);
			}
			let start = Instant::now();
			for _i in 0..times {
				// Not using extend because that's not a real world usage at all in the vast vast vast majority of cases
				black_box(world.push(()));
			}
			start.elapsed()
		});
	});
	group.bench_function("valid-check/exists", move |b| {
		b.iter_custom(|times| {
			let mut world = World::default();
			let entities: Vec<_> = (0..times).map(|_| world.push(())).collect();
			let start = Instant::now();
			for e in entities {
				black_box(world.contains(e));
			}
			start.elapsed()
		});
	});
	group.bench_function("valid-check/deleted", move |b| {
		b.iter_custom(|times| {
			let mut world = World::default();
			let entities: Vec<_> = (0..times).map(|_| world.push(())).collect();
			for e in entities.iter().copied() {
				world.remove(e);
			}
			let start = Instant::now();
			for e in entities {
				black_box(world.contains(e));
			}
			start.elapsed()
		});
	});
	group.bench_function("delete", move |b| {
		b.iter_custom(|times| {
			let mut world = World::default();
			let entities: Vec<_> = (0..times).map(|_| world.push(())).collect();
			let start = Instant::now();
			for e in entities.iter() {
				world.remove(*e);
			}
			start.elapsed()
		});
	});
	*/
}

criterion_group!(
	benchmarks,
	entity_table,
	//storage_table,
);
