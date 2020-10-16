use crate::components::*;
use criterion::*;
use enrs::database::Database;
use enrs::tables::{DenseEntityValueTable, EntityTable, VecEntityValueTable};
use enrs::{tl, TL};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

type EntityType = u64;

fn setup(times: u64) -> (Database, Rc<RefCell<EntityTable<EntityType>>>) {
	let mut database = Database::new();
	let entities_storage = database
		.tables
		.create(
			"entities",
			EntityTable::<EntityType>::builder_with_capacity(times as usize),
		)
		.unwrap();
	(database, entities_storage)
}

/*
macro_rules! delete_benchmark {
	($GROUP:ident, $COUNT:expr, $TYPE:ty, $NEW:ident) => {
		$GROUP.bench_function(format!("delete/{}/components-only", $COUNT), move |b| {
			b.iter_custom(|times| {
				let (mut database, entities_storage, multi_storage) = setup(times);
				let mut entities = entities_storage.borrow_mut();
				let mut multi = multi_storage.borrow_mut();
				let entity_vec: Vec<_> = (0..times).map(|_| entities.insert().raw()).collect();
				let mut inserter = multi.group_insert::<$TYPE>().unwrap();
					{
					let mut lock = inserter.lock(&mut multi);
					for &e in entity_vec.iter() {
						lock.insert(entities.valid(e).unwrap(), $NEW(e)).unwrap();
					}
					}
				let mut lock = multi.lock().unwrap();
				let start = Instant::now();
				for e in entity_vec {
					let _ = lock.delete(entities.valid(e).unwrap());
					}
				start.elapsed()
			});
			});
		$GROUP.bench_function(
			format!("delete/{}/entity-and-components", $COUNT),
			move |b| {
				b.iter_custom(|times| {
					let (mut database, entities_storage, multi_storage) = setup(times);
					let mut entities = entities_storage.borrow_mut();
					let entity_vec = {
						let mut multi = multi_storage.borrow_mut();
						let entity_vec: Vec<_> =
							(0..times).map(|_| entities.insert().raw()).collect();
						let mut inserter = multi.group_insert::<$TYPE>().unwrap();
						let mut lock = inserter.lock(&mut multi);
						for &e in entity_vec.iter() {
							lock.insert(entities.valid(e).unwrap(), $NEW(e)).unwrap();
						}
						entity_vec
					};
					let start = Instant::now();
					for e in entity_vec {
						let _ = entities.delete(e);
					}
					start.elapsed()
				});
				},
			);
	};
}
*/

/*
fn benchmark(c: &mut Criterion) {
	let mut group = c.benchmark_group(
		std::any::type_name::<DenseEntityDynamicPagedMultiValueTable<EntityType>>()
			.split("::")
			.last()
			.unwrap(),
	);
	/*
	group.bench_function("insert/1/no-create-entity", move |b| {
		b.iter_custom(|times| {
			let (mut database, entities_storage, multi_storage) = setup(times);
			let mut entities = entities_storage.borrow_mut();
			let mut multi = multi_storage.borrow_mut();
			let entity_vec: Vec<_> = (0..times).map(|_| entities.insert().raw()).collect();
			let mut inserter = multi.group_insert::<TL![&mut A]>().unwrap();
			let mut lock = inserter.lock(&mut multi);
			let start = Instant::now();
			for e in entity_vec {
				black_box(lock.insert(entities.valid(e).unwrap(), tl![A(e)]));
			}
			start.elapsed()
		});
	});
	group.bench_function("insert/1/with-create-entity", move |b| {
		b.iter_custom(|times| {
			let (mut database, entities_storage, multi_storage) = setup(times);
			let mut entities = entities_storage.borrow_mut();
			let mut multi = multi_storage.borrow_mut();
			let mut inserter = multi.group_insert::<TL![&mut A]>().unwrap();
			let mut lock = inserter.lock(&mut multi);
			let start = Instant::now();
			for _i in 0..times {
				let e = entities.insert();
				black_box(lock.insert(e, tl![A(e.raw())]));
			}
			start.elapsed()
		});
	});
	group.bench_function("insert/4/no-create-entity", move |b| {
		b.iter_custom(|times| {
			let (mut database, entities_storage, multi_storage) = setup(times);
			let mut entities = entities_storage.borrow_mut();
			let mut multi = multi_storage.borrow_mut();
			let entity_vec: Vec<_> = (0..times).map(|_| entities.insert().raw()).collect();
			let mut inserter = multi.group_insert::<Type4>().unwrap();
			let mut lock = inserter.lock(&mut multi);
			let start = Instant::now();
			for e in entity_vec {
				black_box(lock.insert(entities.valid(e).unwrap(), type4_new(e)));
			}
			start.elapsed()
		});
	});
	group.bench_function("insert/4/with-create-entity", move |b| {
		b.iter_custom(|times| {
			let (mut database, entities_storage, multi_storage) = setup(times);
			let mut entities = entities_storage.borrow_mut();
			let mut multi = multi_storage.borrow_mut();
			let mut inserter = multi.group_insert::<Type4>().unwrap();
			let mut lock = inserter.lock(&mut multi);
			let start = Instant::now();
			for _i in 0..times {
				let e = entities.insert();
				black_box(lock.insert(e, type4_new(e.raw())));
			}
			start.elapsed()
		});
	});
	group.bench_function("insert/8/no-create-entity", move |b| {
		b.iter_custom(|times| {
			let (mut database, entities_storage, multi_storage) = setup(times);
			let mut entities = entities_storage.borrow_mut();
			let mut multi = multi_storage.borrow_mut();
			let entity_vec: Vec<_> = (0..times).map(|_| entities.insert().raw()).collect();
			let mut inserter = multi.group_insert::<Type8>().unwrap();
			let mut lock = inserter.lock(&mut multi);
			let start = Instant::now();
			for e in entity_vec {
				black_box(lock.insert(entities.valid(e).unwrap(), type8_new(e)));
			}
			start.elapsed()
		});
	});
	group.bench_function("insert/8/with-create-entity", move |b| {
		b.iter_custom(|times| {
			let (mut database, entities_storage, multi_storage) = setup(times);
			let mut entities = entities_storage.borrow_mut();
			let mut multi = multi_storage.borrow_mut();
			let mut inserter = multi.group_insert::<Type8>().unwrap();
			let mut lock = inserter.lock(&mut multi);
			let start = Instant::now();
			for _i in 0..times {
				let e = entities.insert();
				black_box(lock.insert(e, type8_new(e.raw())));
			}
			start.elapsed()
		});
	});
	group.bench_function("insert/16/no-create-entity", move |b| {
		b.iter_custom(|times| {
			let (mut database, entities_storage, multi_storage) = setup(times);
			let mut entities = entities_storage.borrow_mut();
			let mut multi = multi_storage.borrow_mut();
			let entity_vec: Vec<_> = (0..times).map(|_| entities.insert().raw()).collect();
			let mut inserter = multi.group_insert::<Type16>().unwrap();
			let mut lock = inserter.lock(&mut multi);
			let start = Instant::now();
			for e in entity_vec {
				black_box(lock.insert(entities.valid(e).unwrap(), type16_new(e)));
			}
			start.elapsed()
		});
	});
	group.bench_function("insert/16/with-create-entity", move |b| {
		b.iter_custom(|times| {
			let (mut database, entities_storage, multi_storage) = setup(times);
			let mut entities = entities_storage.borrow_mut();
			let mut multi = multi_storage.borrow_mut();
			let mut inserter = multi.group_insert::<Type16>().unwrap();
			let mut lock = inserter.lock(&mut multi);
			let start = Instant::now();
			for _i in 0..times {
				let e = entities.insert();
				black_box(lock.insert(e, type16_new(e.raw())));
			}
			start.elapsed()
		});
	});
	group.bench_function("delete/1/components-only", move |b| {
		b.iter_custom(|times| {
			let (mut database, entities_storage, multi_storage) = setup(times);
			let mut entities = entities_storage.borrow_mut();
			let mut multi = multi_storage.borrow_mut();
			let entity_vec: Vec<_> = (0..times).map(|_| entities.insert().raw()).collect();
			let mut inserter = multi.group_insert::<TL![&mut A]>().unwrap();
			{
				let mut lock = inserter.lock(&mut multi);
				for &e in entity_vec.iter() {
					lock.insert(entities.valid(e).unwrap(), tl![A(e)]).unwrap();
				}
			}
			let mut lock = multi.lock().unwrap();
			let start = Instant::now();
			for e in entity_vec {
				let _ = lock.delete(entities.valid(e).unwrap());
			}
			start.elapsed()
		});
	});
	group.bench_function("delete/1/entity-and-components", move |b| {
		b.iter_custom(|times| {
			let (mut database, entities_storage, multi_storage) = setup(times);
			let mut entities = entities_storage.borrow_mut();
			let entity_vec = {
				let mut multi = multi_storage.borrow_mut();
				let entity_vec: Vec<_> = (0..times).map(|_| entities.insert().raw()).collect();
				let mut inserter = multi.group_insert::<TL![&mut A]>().unwrap();
				let mut lock = inserter.lock(&mut multi);
				for &e in entity_vec.iter() {
					lock.insert(entities.valid(e).unwrap(), tl![A(e)]).unwrap();
				}
				entity_vec
			};
			let start = Instant::now();
			for e in entity_vec {
				let _ = entities.delete(e);
			}
			start.elapsed()
		});
	});
	delete_benchmark!(group, 4, Type4, type4_new);
	delete_benchmark!(group, 8, Type8, type8_new);
	delete_benchmark!(group, 16, Type16, type16_new);
	group.bench_function("transform/1/add-1/remove-1", move |b| {
		b.iter_custom(|times| {
			let (mut database, entities_storage, multi_storage) = setup(times);
			let mut entities = entities_storage.borrow_mut();
			let entity_vec: Vec<_> = (0..times).map(|_| entities.insert().raw()).collect();
			let mut multi = multi_storage.borrow_mut();
			let mut inserter = multi.group_insert::<TL![&mut A]>().unwrap();
			{
				let mut lock = inserter.lock(&mut multi);
				for &e in entity_vec.iter() {
					lock.insert(entities.valid(e).unwrap(), tl![A(e)]).unwrap();
				}
			}
			let transform_to = multi.group_insert::<TL![&mut B]>().unwrap();
			let mut lock = multi.lock().unwrap();
			let start = Instant::now();
			for e in entity_vec {
				let _ = lock.transform::<TL![A], _>(
					entities.valid(e).unwrap(),
					&transform_to,
					tl![B(e)],
				);
			}
			start.elapsed()
		});
	});
	group.bench_function("transform/8/add-1/remove-1", move |b| {
		b.iter_custom(|times| {
			let (mut database, entities_storage, multi_storage) = setup(times);
			let mut entities = entities_storage.borrow_mut();
			let entity_vec: Vec<_> = (0..times).map(|_| entities.insert().raw()).collect();
			let mut multi = multi_storage.borrow_mut();
			let mut inserter = multi.group_insert::<Type8>().unwrap();
			{
				let mut lock = inserter.lock(&mut multi);
				for &e in entity_vec.iter() {
					lock.insert(entities.valid(e).unwrap(), type8_new(e))
						.unwrap();
				}
			}
			let transform_to = multi.group_insert::<TL![&mut P]>().unwrap();
			let mut lock = multi.lock().unwrap();
			let start = Instant::now();
			for e in entity_vec {
				let _ = lock.transform::<TL![D], _>(
					entities.valid(e).unwrap(),
					&transform_to,
					tl![P(e)],
				);
			}
			start.elapsed()
		});
	});
	*/
}
*/

macro_rules! simple_storage_benchmark {
	(SETUP, $TYPE:ty, $TIMES:ident) => {{
		let (mut database, entities_storage) = setup($TIMES);
		let simple_storage = database
			.tables
			.create("storage", <$TYPE>::builder(entities_storage.clone()))
			.unwrap();
		(database, entities_storage, simple_storage)
		}};
	($BENCH_NAME:ident, $TYPE:ty) => {
		fn $BENCH_NAME(c: &mut Criterion) {
			use regex::*;
			let re = Regex::new(r"[a-zA-Z_]+::").unwrap();
			let name = re.replace_all(std::any::type_name::<$TYPE>(), "");
			let mut group = c.benchmark_group(name);
			group.bench_function("insert/no-create-entity", move |b| {
				b.iter_custom(|times| {
					let (_database, entities_storage, simple_storage) =
						simple_storage_benchmark!(SETUP, $TYPE, times);
					let mut entities = entities_storage.borrow_mut();
					let mut simple = simple_storage.borrow_mut();
					let entity_vec: Vec<_> = (0..times).map(|_| entities.insert().raw()).collect();
					let start = Instant::now();
					for e in entity_vec {
						black_box(simple.insert(entities.valid(e).unwrap(), A(e)));
					}
					start.elapsed()
				});
			});
			group.bench_function("insert/with-create-entity", move |b| {
				b.iter_custom(|times| {
					let (_database, entities_storage, simple_storage) =
						simple_storage_benchmark!(SETUP, $TYPE, times);
					let mut entities = entities_storage.borrow_mut();
					let mut simple = simple_storage.borrow_mut();
					let start = Instant::now();
					for _i in 0..times {
						let e = entities.insert();
						black_box(simple.insert(e, A(e.raw())));
					}
					start.elapsed()
				});
			});
		}
	};
}

simple_storage_benchmark!(vec_bench, VecEntityValueTable<EntityType, A>);
simple_storage_benchmark!(dense_vec_bench, DenseEntityValueTable<EntityType, A>);

criterion_group!(benchmarks, vec_bench, dense_vec_bench,);
