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
	/*
	group.bench_function("insert/4/no-create-entity", move |b| {
		b.iter_custom(|times| {
			let mut world = World::default();
			let entities: Vec<_> = world.run(|mut ents: EntitiesViewMut| {
				(0..times).map(|_| ents.add_entity((), ())).collect()
			});
			world.run(
				|ents: EntitiesView,
				 mut a: ViewMut<A>,
				 mut b: ViewMut<B>,
				 mut c: ViewMut<C>,
				 mut d: ViewMut<D>| {
					let start = Instant::now();
					for e in entities {
						ents.add_component(
							(&mut a, &mut b, &mut c, &mut d),
							(A(e.index()), B(e.index()), C(e.index()), D(e.index())),
							e,
						);
					}
					start.elapsed()
				},
			)
		});
	});
	group.bench_function("insert/4/with-create-entity", move |b| {
		b.iter_custom(|times| {
			let mut world = World::default();
			world.run(
				|mut ents: EntitiesViewMut,
				 mut a: ViewMut<A>,
				 mut b: ViewMut<B>,
				 mut c: ViewMut<C>,
				 mut d: ViewMut<D>| {
					let start = Instant::now();
					for i in 0u64..times {
						black_box(ents.add_entity(
							(&mut a, &mut b, &mut c, &mut d),
							(A(i), B(i), C(i), D(i)),
						));
					}
					start.elapsed()
				},
			)
		});
	});
	group.bench_function("insert/8/no-create-entity", move |b| {
		b.iter_custom(|times| {
			let mut world = World::default();
			let entities: Vec<_> = world.run(|mut ents: EntitiesViewMut| {
				(0..times).map(|_| ents.add_entity((), ())).collect()
			});
			world.run(
				|ents: EntitiesView,
				 mut a: ViewMut<A>,
				 mut b: ViewMut<B>,
				 mut c: ViewMut<C>,
				 mut d: ViewMut<D>,
				 mut e: ViewMut<E>,
				 mut f: ViewMut<F>,
				 mut g: ViewMut<G>,
				 mut h: ViewMut<H>| {
					let start = Instant::now();
					for ent in entities {
						ents.add_component(
							(
								&mut a, &mut b, &mut c, &mut d, &mut e, &mut f, &mut g, &mut h,
							),
							(
								A(ent.index()),
								B(ent.index()),
								C(ent.index()),
								D(ent.index()),
								E(ent.index()),
								F(ent.index()),
								G(ent.index()),
								H(ent.index()),
							),
							ent,
						);
					}
					start.elapsed()
				},
			)
		});
	});
	group.bench_function("insert/8/with-create-entity", move |b| {
		b.iter_custom(|times| {
			let mut world = World::default();
			world.run(
				|mut ents: EntitiesViewMut,
				 mut a: ViewMut<A>,
				 mut b: ViewMut<B>,
				 mut c: ViewMut<C>,
				 mut d: ViewMut<D>,
				 mut e: ViewMut<E>,
				 mut f: ViewMut<F>,
				 mut g: ViewMut<G>,
				 mut h: ViewMut<H>| {
					let start = Instant::now();
					for i in 0u64..times {
						black_box(ents.add_entity(
							(
								&mut a, &mut b, &mut c, &mut d, &mut e, &mut f, &mut g, &mut h,
							),
							(A(i), B(i), C(i), D(i), E(i), F(i), G(i), H(i)),
						));
					}
					start.elapsed()
				},
			)
		});
	});
	group.bench_function("insert/16/no-create-entity", move |b| {
		b.iter_custom(|times| {
			let mut world = World::default();
			let entities: Vec<_> = world.run(|mut ents: EntitiesViewMut| {
				(0..times).map(|_| ents.add_entity((), ())).collect()
			});
			world.run(|storages: AllStoragesViewMut| {
				let ents = storages.borrow::<EntitiesView>();
				let mut a = storages.borrow::<ViewMut<A>>();
				let mut b = storages.borrow::<ViewMut<B>>();
				let mut c = storages.borrow::<ViewMut<C>>();
				let mut d = storages.borrow::<ViewMut<D>>();
				let mut e = storages.borrow::<ViewMut<E>>();
				let mut f = storages.borrow::<ViewMut<F>>();
				let mut g = storages.borrow::<ViewMut<G>>();
				let mut h = storages.borrow::<ViewMut<H>>();
				let mut i = storages.borrow::<ViewMut<I>>();
				let mut j = storages.borrow::<ViewMut<J>>();
				let mut k = storages.borrow::<ViewMut<K>>();
				let mut l = storages.borrow::<ViewMut<L>>();
				let mut m = storages.borrow::<ViewMut<M>>();
				let mut n = storages.borrow::<ViewMut<N>>();
				let mut o = storages.borrow::<ViewMut<O>>();
				let mut p = storages.borrow::<ViewMut<P>>();
				let start = Instant::now();
				for ent in entities {
					ents.add_component(
						(
							&mut a, &mut b, &mut c, &mut d, &mut e, &mut f, &mut g, &mut h,
						),
						(
							A(ent.index()),
							B(ent.index()),
							C(ent.index()),
							D(ent.index()),
							E(ent.index()),
							F(ent.index()),
							G(ent.index()),
							H(ent.index()),
						),
						ent,
					);
					ents.add_component(
						(
							&mut i, &mut j, &mut k, &mut l, &mut m, &mut n, &mut o, &mut p,
						),
						(
							I(ent.index()),
							J(ent.index()),
							K(ent.index()),
							L(ent.index()),
							M(ent.index()),
							N(ent.index()),
							O(ent.index()),
							P(ent.index()),
						),
						ent,
					);
				}
				start.elapsed()
			})
		});
	});
	group.bench_function("insert/16/with-create-entity", move |b| {
		b.iter_custom(|times| {
			let mut world = World::default();
			world.run(|storages: AllStoragesViewMut| {
				let mut ents = storages.borrow::<EntitiesViewMut>();
				let mut a = storages.borrow::<ViewMut<A>>();
				let mut b = storages.borrow::<ViewMut<B>>();
				let mut c = storages.borrow::<ViewMut<C>>();
				let mut d = storages.borrow::<ViewMut<D>>();
				let mut e = storages.borrow::<ViewMut<E>>();
				let mut f = storages.borrow::<ViewMut<F>>();
				let mut g = storages.borrow::<ViewMut<G>>();
				let mut h = storages.borrow::<ViewMut<H>>();
				let mut is = storages.borrow::<ViewMut<I>>();
				let mut j = storages.borrow::<ViewMut<J>>();
				let mut k = storages.borrow::<ViewMut<K>>();
				let mut l = storages.borrow::<ViewMut<L>>();
				let mut m = storages.borrow::<ViewMut<M>>();
				let mut n = storages.borrow::<ViewMut<N>>();
				let mut o = storages.borrow::<ViewMut<O>>();
				let mut p = storages.borrow::<ViewMut<P>>();
				let start = Instant::now();
				for i in 0u64..times {
					// shipyard only supports 10 at a time, so split into two calls of 8 each, the author said it should be equivalent...
					let ent = black_box(ents.add_entity(
						(
							&mut a, &mut b, &mut c, &mut d, &mut e, &mut f, &mut g, &mut h,
						),
						(A(i), B(i), C(i), D(i), E(i), F(i), G(i), H(i)),
					));
					ents.add_component(
						(
							&mut is, &mut j, &mut k, &mut l, &mut m, &mut n, &mut o, &mut p,
						),
						(I(i), J(i), K(i), L(i), M(i), N(i), O(i), P(i)),
						ent,
					);
				}
				start.elapsed()
			})
		});
	});
	group.bench_function("transform/8/add-1/remove-1", move |b| {
		b.iter_custom(|times| {
			let mut world = World::default();
			let entity_vec: Vec<_> = world.run(
				|mut ents: EntitiesViewMut,
				 mut a: ViewMut<A>,
				 mut b: ViewMut<B>,
				 mut c: ViewMut<C>,
				 mut d: ViewMut<D>,
				 mut e: ViewMut<E>,
				 mut f: ViewMut<F>,
				 mut g: ViewMut<G>,
				 mut h: ViewMut<H>| {
					(0..times)
						.map(|i| {
							ents.add_entity(
								(
									&mut a, &mut b, &mut c, &mut d, &mut e, &mut f, &mut g, &mut h,
								),
								(A(i), B(i), C(i), D(i), E(i), F(i), G(i), H(i)),
							)
						})
						.collect()
				},
			);
			world.run(
				|entities: EntitiesView, mut d: ViewMut<D>, mut p: ViewMut<P>| {
					let start = Instant::now();
					for entity in entity_vec {
						d.remove(entity);
						entities.add_component(&mut p, P(entity.index()), entity);
					}
					start.elapsed()
				},
			)
		});
	});
	*/
}

criterion_group!(benchmarks, entity_table, storage_table,);
