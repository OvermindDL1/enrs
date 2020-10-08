use crate::components::*;
use criterion::*;
use enrs::database::Database;
use enrs::tables::{
	DenseEntityDynamicPagedMultiValueTable, DenseEntityValueTable, EntityTable, VecEntityValueTable,
};
use enrs::{tl, TL};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

type EntityType = u64;

fn setup(
	times: u64,
) -> (
	Database,
	Rc<RefCell<EntityTable<EntityType>>>,
	Rc<RefCell<DenseEntityDynamicPagedMultiValueTable<EntityType>>>,
) {
	let mut database = Database::new();
	let entities_storage = database
		.tables
		.create(
			"entities",
			EntityTable::<EntityType>::builder_with_capacity(times as usize),
		)
		.unwrap();
	let multi_storage = database
		.tables
		.create(
			"multi",
			DenseEntityDynamicPagedMultiValueTable::<u64>::builder(entities_storage.clone()),
		)
		.unwrap();
	(database, entities_storage, multi_storage)
}

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

fn benchmark(c: &mut Criterion) {
	let mut group = c.benchmark_group(
		std::any::type_name::<DenseEntityDynamicPagedMultiValueTable<EntityType>>()
			.split("::")
			.last()
			.unwrap(),
	);
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
	// group.bench_function("delete/4/components-only", move |b| {
	// 	b.iter_custom(|times| {
	// 		let (mut database, entities_storage, multi_storage) = setup(times);
	// 		let mut entities = entities_storage.borrow_mut();
	// 		let mut multi = multi_storage.borrow_mut();
	// 		let entity_vec: Vec<_> = (0..times).map(|_| entities.insert().raw()).collect();
	// 		let mut inserter = multi.group_insert::<Type4>().unwrap();
	// 		{
	// 			let mut lock = inserter.lock(&mut multi);
	// 			for &e in entity_vec.iter() {
	// 				lock.insert(entities.valid(e).unwrap(), type4_new(e))
	// 					.unwrap();
	// 			}
	// 		}
	// 		let mut lock = multi.lock().unwrap();
	// 		let start = Instant::now();
	// 		for e in entity_vec {
	// 			let _ = lock.delete(entities.valid(e).unwrap());
	// 		}
	// 		start.elapsed()
	// 	});
	// });
	// group.bench_function("delete/4/entity-and-components", move |b| {
	// 	b.iter_custom(|times| {
	// 		let (mut database, entities_storage, multi_storage) = setup(times);
	// 		let mut entities = entities_storage.borrow_mut();
	// 		let entity_vec = {
	// 			let mut multi = multi_storage.borrow_mut();
	// 			let entity_vec: Vec<_> = (0..times).map(|_| entities.insert().raw()).collect();
	// 			let mut inserter = multi.group_insert::<Type4>().unwrap();
	// 			let mut lock = inserter.lock(&mut multi);
	// 			for &e in entity_vec.iter() {
	// 				lock.insert(entities.valid(e).unwrap(), type4_new(e))
	// 					.unwrap();
	// 			}
	// 			entity_vec
	// 		};
	// 		let start = Instant::now();
	// 		for e in entity_vec {
	// 			let _ = entities.delete(e);
	// 		}
	// 		start.elapsed()
	// 	});
	// });
	// group.bench_function("insert/recycled", move |b| {
	// 	b.iter_custom(|times| {
	// 		let mut database = Database::new();
	// 		let entities_storage = database
	// 			.tables
	// 			.create(
	// 				"entities",
	// 				EntityTable::<EntityType>::builder_with_capacity(times as usize),
	// 			)
	// 			.unwrap();
	// 		let mut entities = entities_storage.borrow_mut();
	// 		let entity_vec: Vec<_> = (0..times).map(|_| entities.insert().raw()).collect();
	// 		for e in entity_vec {
	// 			let _ = black_box(entities.delete(e));
	// 		}
	// 		let start = Instant::now();
	// 		for _i in 0..times {
	// 			black_box(entities.insert());
	// 		}
	// 		start.elapsed()
	// 	});
	// });
	// group.bench_function("delete", move |b| {
	// 	b.iter_custom(|times| {
	// 		let mut database = Database::new();
	// 		let entities_storage = database
	// 			.tables
	// 			.create(
	// 				"entities",
	// 				EntityTable::<EntityType>::builder_with_capacity(times as usize),
	// 			)
	// 			.unwrap();
	// 		let mut entities = entities_storage.borrow_mut();
	// 		let entity_vec: Vec<_> = (0..times).map(|_| entities.insert().raw()).collect();
	// 		let start = Instant::now();
	// 		for e in entity_vec {
	// 			let _ = black_box(entities.delete(e));
	// 		}
	// 		start.elapsed()
	// 	});
	// });
	// group.bench_function("delete/multi", move |b| {
	// 	b.iter_custom(|times| {
	// 		let mut database = Database::new();
	// 		let entities_storage = database
	// 			.tables
	// 			.create(
	// 				"entities",
	// 				EntityTable::<EntityType>::builder_with_capacity(times as usize),
	// 			)
	// 			.unwrap();
	// 		let _multi_storage = database
	// 			.tables
	// 			.create(
	// 				"multi",
	// 				DenseEntityDynamicPagedMultiValueTable::<u64>::builder(
	// 					entities_storage.clone(),
	// 				),
	// 			)
	// 			.unwrap();
	// 		let mut entities = entities_storage.borrow_mut();
	// 		let entity_vec: Vec<_> = (0..times).map(|_| entities.insert().raw()).collect();
	// 		let start = Instant::now();
	// 		for e in entity_vec {
	// 			let _ = black_box(entities.delete(e));
	// 		}
	// 		start.elapsed()
	// 	});
	// });
	// group.bench_function("delete/vec-dense-multi", move |b| {
	// 	b.iter_custom(|times| {
	// 		let mut database = Database::new();
	// 		let entities_storage = database
	// 			.tables
	// 			.create(
	// 				"entities",
	// 				EntityTable::<EntityType>::builder_with_capacity(times as usize),
	// 			)
	// 			.unwrap();
	// 		let _multi_storage = database
	// 			.tables
	// 			.create(
	// 				"multi",
	// 				DenseEntityDynamicPagedMultiValueTable::<u64>::builder(
	// 					entities_storage.clone(),
	// 				),
	// 			)
	// 			.unwrap();
	// 		let _ints_storage = database
	// 			.tables
	// 			.create(
	// 				"ints",
	// 				DenseEntityValueTable::<u64, isize>::builder(entities_storage.clone()),
	// 			)
	// 			.unwrap();
	// 		let _shorts_storage = database
	// 			.tables
	// 			.create(
	// 				"shorts",
	// 				VecEntityValueTable::<u64, i16>::builder(entities_storage.clone()),
	// 			)
	// 			.unwrap();
	// 		let mut entities = entities_storage.borrow_mut();
	// 		let entity_vec: Vec<_> = (0..times).map(|_| entities.insert().raw()).collect();
	// 		let start = Instant::now();
	// 		for e in entity_vec {
	// 			let _ = black_box(entities.delete(e));
	// 		}
	// 		start.elapsed()
	// 	});
	// });
	// group.bench_function("delete/dense-multi", move |b| {
	// 	b.iter_custom(|times| {
	// 		let mut database = Database::new();
	// 		let entities_storage = database
	// 			.tables
	// 			.create(
	// 				"entities",
	// 				EntityTable::<EntityType>::builder_with_capacity(times as usize),
	// 			)
	// 			.unwrap();
	// 		let _multi_storage = database
	// 			.tables
	// 			.create(
	// 				"multi",
	// 				DenseEntityDynamicPagedMultiValueTable::<u64>::builder(
	// 					entities_storage.clone(),
	// 				),
	// 			)
	// 			.unwrap();
	// 		let _ints_storage = database
	// 			.tables
	// 			.create(
	// 				"ints",
	// 				DenseEntityValueTable::<u64, isize>::builder(entities_storage.clone()),
	// 			)
	// 			.unwrap();
	// 		let mut entities = entities_storage.borrow_mut();
	// 		let entity_vec: Vec<_> = (0..times).map(|_| entities.insert().raw()).collect();
	// 		let start = Instant::now();
	// 		for e in entity_vec {
	// 			let _ = black_box(entities.delete(e));
	// 		}
	// 		start.elapsed()
	// 	});
	// });
}

criterion_group!(benchmarks, benchmark,);
