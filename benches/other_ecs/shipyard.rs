use crate::components::*;
use criterion::*;
use shipyard::*;
use std::time::Instant;

fn entity_table(c: &mut Criterion) {
	let mut group = c.benchmark_group("other_ecs/shipyard/EntityTable<u64>");
	group.bench_function("insert", move |b| {
		b.iter_custom(|times| {
			let mut world = World::new();
			world.run(|mut ents: EntitiesViewMut| {
				let start = Instant::now();
				for _i in 0..times {
					black_box(ents.add_entity((), ()));
				}
				start.elapsed()
			})
		});
	});
	group.bench_function("insert/recycled", move |b| {
		b.iter_custom(|times| {
			let mut world = World::new();
			let entities: Vec<_> = world.run(|mut entities: EntitiesViewMut| {
				(0..times).map(|_| entities.add_entity((), ())).collect()
			});
			world.run(|mut storages: AllStoragesViewMut| {
				for e in entities {
					let _ = black_box(storages.delete(e));
				}
			});
			world.run(|mut ents: EntitiesViewMut| {
				let start = Instant::now();
				for _i in 0..times {
					black_box(ents.add_entity((), ()));
				}
				start.elapsed()
			})
		});
	});
	group.bench_function("valid-check/exists", move |b| {
		b.iter_custom(|times| {
			let mut world = World::new();
			let entities: Vec<_> = world.run(|mut entities: EntitiesViewMut| {
				(0..times).map(|_| entities.add_entity((), ())).collect()
			});
			world.run(|mut ents: EntitiesViewMut| {
				let start = Instant::now();
				for e in entities {
					black_box(ents.is_alive(e));
				}
				start.elapsed()
			})
		});
	});
	group.bench_function("valid-check/deleted", move |b| {
		b.iter_custom(|times| {
			let mut world = World::new();
			let entities: Vec<_> = world.run(|mut ents: EntitiesViewMut| {
				(0..times).map(|_| ents.add_entity((), ())).collect()
			});
			world.run(|mut storages: AllStoragesViewMut| {
				for &e in &entities {
					let _ = black_box(storages.delete(e));
				}
			});
			world.run(|mut ents: EntitiesViewMut| {
				let start = Instant::now();
				for e in entities {
					black_box(ents.is_alive(e));
				}
				start.elapsed()
			})
		});
	});
	group.bench_function("delete", move |b| {
		b.iter_custom(|times| {
			let mut world = World::new();
			let entities: Vec<_> = world.run(|mut ents: EntitiesViewMut| {
				(0..times).map(|_| ents.add_entity((), ())).collect()
			});
			world.run(|mut storages: AllStoragesViewMut| {
				let start = Instant::now();
				for &e in &entities {
					let _ = black_box(storages.delete(e));
				}
				start.elapsed()
			})
		});
	});
}

fn storage_table(c: &mut Criterion) {
	let mut group = c.benchmark_group("other_ecs/shipyard/StorageTable<u64>");
	group.bench_function("insert/1/no-create-entity", move |b| {
		b.iter_custom(|times| {
			let mut world = World::new();
			let entities: Vec<_> = world.run(|mut ents: EntitiesViewMut| {
				(0..times).map(|_| ents.add_entity((), ())).collect()
			});
			world.run(|ents: EntitiesView, mut a: ViewMut<A>| {
				let start = Instant::now();
				for e in entities {
					ents.add_component((&mut a,), (A(e.index()),), e);
				}
				start.elapsed()
			})
		});
	});
	group.bench_function("insert/1/with-create-entity", move |b| {
		b.iter_custom(|times| {
			let mut world = World::new();
			world.run(|mut ents: EntitiesViewMut, mut a: ViewMut<A>| {
				let start = Instant::now();
				for i in 0u64..times {
					black_box(ents.add_entity((&mut a,), (A(i),)));
				}
				start.elapsed()
			})
		});
	});
	group.bench_function("insert/4/no-create-entity", move |b| {
		b.iter_custom(|times| {
			let mut world = World::new();
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
			let mut world = World::new();
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
			let mut world = World::new();
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
			let mut world = World::new();
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
			let mut world = World::new();
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
			let mut world = World::new();
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
}

criterion_group!(benchmarks, entity_table, storage_table,);
