use crate::components::*;
use criterion::*;
use legion::*;
use std::time::Instant;

fn entity_table(c: &mut Criterion) {
	let mut group = c.benchmark_group("other_ecs/legion/EntityTable<u64>");
	group.bench_function("insert", move |b| {
		b.iter_custom(|times| {
			let mut world = World::default();
			let start = Instant::now();
			for _i in 0..times {
				// Not using extend because that's not a real world usage at all in the vast vast vast majority of cases
				black_box(world.push(()));
			}
			start.elapsed()
		});
	});
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
}

fn storage_table(c: &mut Criterion) {
	let mut group = c.benchmark_group("other_ecs/legion/ValueTable<A>");
	group.bench_function("insert/1/no-create-entity", move |b| {
		b.iter_custom(|times| {
			let mut world = World::default();
			let entities: Vec<_> = (0..times).map(|_| world.push(())).collect();
			let start = Instant::now();
			for (e, i) in entities.iter().zip(0..times) {
				world.entry(*e).unwrap().add_component(A(i as u64));
			}
			start.elapsed()
		});
	});
	group.bench_function("insert/1/with-create-entity", move |b| {
		b.iter_custom(|times| {
			let mut world = World::default();
			let start = Instant::now();
			for i in 0..times {
				black_box(world.push((A(i),)));
			}
			start.elapsed()
		});
	});
	group.bench_function("insert/4/no-create-entity", move |b| {
		b.iter_custom(|times| {
			let mut world = World::default();
			let entities: Vec<_> = (0..times).map(|_| world.push(())).collect();
			let start = Instant::now();
			for (e, i) in entities.iter().zip(0..times) {
				let mut ent = world.entry(*e).unwrap();
				ent.add_component(A(i as u64));
				ent.add_component(B(i as u64));
				ent.add_component(C(i as u64));
				ent.add_component(D(i as u64));
			}
			start.elapsed()
		});
	});
	group.bench_function("insert/4/with-create-entity", move |b| {
		b.iter_custom(|times| {
			let mut world = World::default();
			let start = Instant::now();
			for i in 0..times {
				black_box(world.push(type4_new_flat(i)));
			}
			start.elapsed()
		});
	});
	group.bench_function("insert/8/no-create-entity", move |b| {
		b.iter_custom(|times| {
			let mut world = World::default();
			let entities: Vec<_> = (0..times).map(|_| world.push(())).collect();
			let start = Instant::now();
			for (e, i) in entities.iter().zip(0..times) {
				let mut ent = world.entry(*e).unwrap();
				ent.add_component(A(i as u64));
				ent.add_component(B(i as u64));
				ent.add_component(C(i as u64));
				ent.add_component(D(i as u64));
				ent.add_component(E(i as u64));
				ent.add_component(F(i as u64));
				ent.add_component(G(i as u64));
				ent.add_component(H(i as u64));
			}
			start.elapsed()
		});
	});
	group.bench_function("insert/8/with-create-entity", move |b| {
		b.iter_custom(|times| {
			let mut world = World::default();
			let start = Instant::now();
			for i in 0..times {
				black_box(world.push(type8_new_flat(i)));
			}
			start.elapsed()
		});
	});
	group.bench_function("insert/16/no-create-entity", move |b| {
		b.iter_custom(|times| {
			let mut world = World::default();
			let entities: Vec<_> = (0..times).map(|_| world.push(())).collect();
			let start = Instant::now();
			for (e, i) in entities.iter().zip(0..times) {
				let mut ent = world.entry(*e).unwrap();
				ent.add_component(A(i as u64));
				ent.add_component(B(i as u64));
				ent.add_component(C(i as u64));
				ent.add_component(D(i as u64));
				ent.add_component(E(i as u64));
				ent.add_component(F(i as u64));
				ent.add_component(G(i as u64));
				ent.add_component(H(i as u64));
				ent.add_component(I(i as u64));
				ent.add_component(J(i as u64));
				ent.add_component(K(i as u64));
				ent.add_component(L(i as u64));
				ent.add_component(M(i as u64));
				ent.add_component(N(i as u64));
				ent.add_component(O(i as u64));
				ent.add_component(P(i as u64));
			}
			start.elapsed()
		});
	});
	group.bench_function("insert/16/with-create-entity", move |b| {
		b.iter_custom(|times| {
			let mut world = World::default();
			let start = Instant::now();
			for i in 0..times {
				// Legion only allows 8 types to be inserted at once without the "extended-tuple-impls" feature
				black_box(world.push(type16_new_flat(i)));
			}
			start.elapsed()
		});
	});
	group.bench_function("insert/16/with-create-entity/bulk", move |b| {
		b.iter_custom(|times| {
			let mut world = World::default();
			let start = Instant::now();
			world.extend(
				(
					(0..times).map(|i| A(i)).collect(),
					(0..times).map(|i| B(i)).collect(),
					(0..times).map(|i| C(i)).collect(),
					(0..times).map(|i| D(i)).collect(),
					(0..times).map(|i| E(i)).collect(),
					(0..times).map(|i| F(i)).collect(),
					(0..times).map(|i| G(i)).collect(),
					(0..times).map(|i| H(i)).collect(),
					(0..times).map(|i| I(i)).collect(),
					(0..times).map(|i| J(i)).collect(),
					(0..times).map(|i| K(i)).collect(),
					(0..times).map(|i| L(i)).collect(),
					(0..times).map(|i| M(i)).collect(),
					(0..times).map(|i| N(i)).collect(),
					(0..times).map(|i| O(i)).collect(),
					(0..times).map(|i| P(i)).collect(),
				)
					.into_soa(),
			);
			start.elapsed()
		});
	});
	group.bench_function("transform/1/add-1/remove-1", move |b| {
		b.iter_custom(|times| {
			let mut world = World::default();
			let entity_vec: Vec<_> = (0..times).map(|i| world.push((A(i),))).collect();
			let start = Instant::now();
			for (entity, i) in entity_vec.iter().zip(0..times) {
				let mut e = world.entry(*entity).unwrap();
				e.add_component(B(i));
				e.remove_component::<A>();
			}
			start.elapsed()
		});
	});
	group.bench_function("transform/8/add-1/remove-1", move |b| {
		b.iter_custom(|times| {
			let mut world = World::default();
			let entity_vec: Vec<_> = (0..times).map(|i| world.push(type8_new_flat(i))).collect();
			let start = Instant::now();
			for (entity, i) in entity_vec.iter().zip(0..times) {
				let mut e = world.entry(*entity).unwrap();
				e.add_component(P(i));
				e.remove_component::<D>();
			}
			start.elapsed()
		});
	});
}

criterion_group!(benchmarks, entity_table, storage_table,);
