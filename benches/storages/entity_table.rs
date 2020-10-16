use criterion::*;
use enrs::database::Database;
use enrs::tables::{
	DenseEntityDynamicPagedMultiValueTable, DenseEntityValueTable, EntityTable, VecEntityValueTable,
};
use std::time::Instant;

type EntityType = u64;

fn entity_table(c: &mut Criterion) {
	let mut group = c.benchmark_group(
		std::any::type_name::<EntityTable<EntityType>>()
			.split("::")
			.last()
			.unwrap(),
	);
	group.bench_function("insert", move |b| {
		b.iter_custom(|times| {
			let mut database = Database::new();
			let entities_storage = database
				.tables
				.create(
					"entities",
					EntityTable::<EntityType>::builder_with_capacity(times as usize),
				)
				.unwrap();
			let mut entities = entities_storage.borrow_mut();
			let start = Instant::now();
			for _i in 0..times {
				black_box(entities.insert());
			}
			start.elapsed()
		});
	});
	group.bench_function("insert/recycled", move |b| {
		b.iter_custom(|times| {
			let mut database = Database::new();
			let entities_storage = database
				.tables
				.create(
					"entities",
					EntityTable::<EntityType>::builder_with_capacity(times as usize),
				)
				.unwrap();
			let mut entities = entities_storage.borrow_mut();
			let entity_vec: Vec<_> = (0..times).map(|_| entities.insert().raw()).collect();
			for e in entity_vec {
				let _ = black_box(entities.delete(e));
			}
			let start = Instant::now();
			for _i in 0..times {
				black_box(entities.insert());
			}
			start.elapsed()
		});
	});
	group.bench_function("valid-check/exists", move |b| {
		b.iter_custom(|times| {
			let mut database = Database::new();
			let entities_storage = database
				.tables
				.create(
					"entities",
					EntityTable::<EntityType>::builder_with_capacity(times as usize),
				)
				.unwrap();
			let mut entities = entities_storage.borrow_mut();
			let entity_vec: Vec<_> = (0..times).map(|_| entities.insert().raw()).collect();
			let start = Instant::now();
			for e in entity_vec {
				let _ = black_box(entities.valid(e));
			}
			start.elapsed()
		});
	});
	group.bench_function("valid-check/deleted", move |b| {
		b.iter_custom(|times| {
			let mut database = Database::new();
			let entities_storage = database
				.tables
				.create(
					"entities",
					EntityTable::<EntityType>::builder_with_capacity(times as usize),
				)
				.unwrap();
			let mut entities = entities_storage.borrow_mut();
			let entity_vec: Vec<_> = (0..times).map(|_| entities.insert().raw()).collect();
			for &e in entity_vec.iter() {
				let _ = black_box(entities.delete(e));
			}
			let start = Instant::now();
			for e in entity_vec {
				let _ = black_box(entities.valid(e));
			}
			start.elapsed()
		});
	});
	group.bench_function("valid-check/never-existed", move |b| {
		b.iter_custom(|times| {
			let mut database = Database::new();
			let entities_storage = database
				.tables
				.create(
					"entities",
					EntityTable::<EntityType>::builder_with_capacity(0),
				)
				.unwrap();
			let entities = entities_storage.borrow_mut();
			let start = Instant::now();
			for e in 0..times {
				let _ = black_box(entities.valid(e));
			}
			start.elapsed()
		});
	});
	group.bench_function("delete", move |b| {
		b.iter_custom(|times| {
			let mut database = Database::new();
			let entities_storage = database
				.tables
				.create(
					"entities",
					EntityTable::<EntityType>::builder_with_capacity(times as usize),
				)
				.unwrap();
			let mut entities = entities_storage.borrow_mut();
			let entity_vec: Vec<_> = (0..times).map(|_| entities.insert().raw()).collect();
			let start = Instant::now();
			for e in entity_vec {
				let _ = black_box(entities.delete(e));
			}
			start.elapsed()
		});
	});
	group.bench_function("delete/multi", move |b| {
		b.iter_custom(|times| {
			let mut database = Database::new();
			let entities_storage = database
				.tables
				.create(
					"entities",
					EntityTable::<EntityType>::builder_with_capacity(times as usize),
				)
				.unwrap();
			let _multi_storage = database
				.tables
				.create(
					"multi",
					DenseEntityDynamicPagedMultiValueTable::<u64>::builder(
						entities_storage.clone(),
					),
				)
				.unwrap();
			let mut entities = entities_storage.borrow_mut();
			let entity_vec: Vec<_> = (0..times).map(|_| entities.insert().raw()).collect();
			let start = Instant::now();
			for e in entity_vec {
				let _ = black_box(entities.delete(e));
			}
			start.elapsed()
		});
	});
	group.bench_function("delete/vec-dense-multi", move |b| {
		b.iter_custom(|times| {
			let mut database = Database::new();
			let entities_storage = database
				.tables
				.create(
					"entities",
					EntityTable::<EntityType>::builder_with_capacity(times as usize),
				)
				.unwrap();
			let _multi_storage = database
				.tables
				.create(
					"multi",
					DenseEntityDynamicPagedMultiValueTable::<u64>::builder(
						entities_storage.clone(),
					),
				)
				.unwrap();
			let _ints_storage = database
				.tables
				.create(
					"ints",
					DenseEntityValueTable::<u64, isize>::builder(entities_storage.clone()),
				)
				.unwrap();
			let _shorts_storage = database
				.tables
				.create(
					"shorts",
					VecEntityValueTable::<u64, i16>::builder(entities_storage.clone()),
				)
				.unwrap();
			let mut entities = entities_storage.borrow_mut();
			let entity_vec: Vec<_> = (0..times).map(|_| entities.insert().raw()).collect();
			let start = Instant::now();
			for e in entity_vec {
				let _ = black_box(entities.delete(e));
			}
			start.elapsed()
		});
	});
	group.bench_function("delete/dense-multi", move |b| {
		b.iter_custom(|times| {
			let mut database = Database::new();
			let entities_storage = database
				.tables
				.create(
					"entities",
					EntityTable::<EntityType>::builder_with_capacity(times as usize),
				)
				.unwrap();
			let _multi_storage = database
				.tables
				.create(
					"multi",
					DenseEntityDynamicPagedMultiValueTable::<u64>::builder(
						entities_storage.clone(),
					),
				)
				.unwrap();
			let _ints_storage = database
				.tables
				.create(
					"ints",
					DenseEntityValueTable::<u64, isize>::builder(entities_storage.clone()),
				)
				.unwrap();
			let mut entities = entities_storage.borrow_mut();
			let entity_vec: Vec<_> = (0..times).map(|_| entities.insert().raw()).collect();
			let start = Instant::now();
			for e in entity_vec {
				let _ = black_box(entities.delete(e));
			}
			start.elapsed()
		});
	});
}

criterion_group!(benchmarks, entity_table,);
