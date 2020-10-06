use criterion::*;
use enrs::database::*;
use enrs::tables::*;
use enrs::{tl, TL};
use std::marker::PhantomData;

type EntityType = u64;

const TIMES: &[usize] = &[10_000];

fn entity_table(c: &mut Criterion) {
	let mut group = c.benchmark_group(format!(
		"insertion/{}",
		std::any::type_name::<EntityTable<EntityType>>()
	));
	for times in TIMES {
		group.bench_with_input(format!("{}", times), times, |b: &mut Bencher<_>, &times| {
			let mut database = Database::new();
			let entities_storage = database
				.tables
				.create(
					"entities",
					EntityTable::<EntityType>::builder_with_capacity(times),
				)
				.unwrap();
			b.iter_batched(
				|| {
					let mut entities = entities_storage.borrow_mut();
					entities.clear().unwrap();
					entities
				},
				|mut entities| {
					for _ in 0..times {
						entities.insert();
					}
				},
				BatchSize::PerIteration,
			);
		});
	}
}

macro_rules! entity_storage_insert_TYPE {
	($BENCH_NAME:ident, $STORAGE:ty, $VALUE_CB:expr) => {
		fn $BENCH_NAME(c: &mut Criterion) {
			let mut group =
				c.benchmark_group(format!("insertion/{}", std::any::type_name::<$STORAGE>()));
			for times in TIMES {
				for count in [1, 4, 8, 16].iter() {
					group.bench_with_input(
						format!("{}/{}", times, count),
						&(times, count),
						|b: &mut Bencher<_>, (&times, &count)| {
							let mut database = Database::new();
							let entities_storage = database
								.tables
								.create(
									"entities",
									EntityTable::<EntityType>::builder_with_capacity(times),
								)
								.unwrap();
							let storages: Vec<_> = (0..count)
								.map(|idx| {
									database
										.tables
										.create(
											format!("storage-{}", idx),
											<$STORAGE>::builder_with_capacity(
												entities_storage.clone(),
												times,
											),
										)
										.unwrap()
								})
								.collect();
							b.iter_batched(
								|| {
									let mut entities = entities_storage.borrow_mut();
									entities.clear().unwrap();
									let storages: Vec<_> =
										storages.iter().map(|s| s.borrow_mut()).collect();
									(entities, storages)
								},
								|(mut entities, mut storages)| {
									for _ in 0..times {
										let entity = entities.insert();
										for storage in storages.iter_mut() {
											let _ = storage.insert(entity, $VALUE_CB(entity));
										}
									}
								},
								BatchSize::PerIteration,
							);
						},
					);
				}
			}
		}
	};
}

entity_storage_insert_TYPE!(dense_entity_value_bench, DenseEntityValueTable<EntityType, u64>, |e: ValidEntity<EntityType>| e.raw() as u64);
entity_storage_insert_TYPE!(vec_entity_value_bench, VecEntityValueTable<EntityType, u64>, |e: ValidEntity<EntityType>| e.raw() as u64);

macro_rules! dense_entity_multi_storage_insert_TYPE {
	($BENCH_NAME:ident, $STORAGE:ty, $VALUE_CB:expr) => {
		fn $BENCH_NAME(c: &mut Criterion) {
			let mut group = c.benchmark_group(format!(
				"insertion/{}",
				std::any::type_name::<DenseEntityDynamicPagedMultiValueTable<EntityType>>()
			));
			for times in TIMES {
				for count in [1, 4, 8, 16].iter() {
					group.bench_with_input(
						format!("{}/{}", times, count),
						&(times, count),
						|b: &mut Bencher<_>, (&times, &count)| {
							let mut database = Database::new();
							let entities_storage = database
								.tables
								.create(
									"entities",
									EntityTable::<EntityType>::builder_with_capacity(times),
								)
								.unwrap();
							let storage = database
								.tables
								.create(
									"multi",
									DenseEntityDynamicPagedMultiValueTable::builder_with_capacity(
										entities_storage.clone(),
										times,
									),
								)
								.unwrap();
							//let mut multi = storage.borrow_mut();
							let mut single_inserter =
								storage.borrow_mut().group_insert::<$STORAGE>().unwrap();
							//let mut entities = entities_storage.borrow_mut();
							//let mut inserter = single_inserter.lock(&mut *multi);
							let new = $VALUE_CB;
							b.iter_batched(
								|| {
									entities_storage.borrow_mut().clear().unwrap();
									()
								},
								|()| {
									let mut entities = entities_storage.borrow_mut();
									let mut multi = storage.borrow_mut();
									let mut inserter = single_inserter.lock(&mut *multi);
									for _ in 0..times {
										let entity = entities.insert();
										inserter.insert(entity, new(entity.raw()));
									}
								},
								BatchSize::PerIteration,
							);
						},
					);
				}
			}
		}
	};
}

struct S<T>(u64, PhantomData<T>);

type S0 = S<[i8; 0]>;
type S1 = S<[i8; 1]>;
type S2 = S<[i8; 2]>;
type S3 = S<[i8; 3]>;
type S4 = S<[i8; 4]>;
type S5 = S<[i8; 5]>;
type S6 = S<[i8; 6]>;
type S7 = S<[i8; 7]>;
type S8 = S<[i8; 8]>;
type S9 = S<[i8; 9]>;
type S10 = S<[i8; 10]>;
type S11 = S<[i8; 11]>;
type S12 = S<[i8; 12]>;
type S13 = S<[i8; 13]>;
type S14 = S<[i8; 14]>;
type S15 = S<[i8; 15]>;

fn s<T>(data: u64) -> S<T> {
	S(data, PhantomData)
}

dense_entity_multi_storage_insert_TYPE!(
	dense_entity_dynamic_paged_multi_value_bench_16,
	TL![
		&mut S0, &mut S1, &mut S2, &mut S3, &mut S4, &mut S5, &mut S6, &mut S7, &mut S8, &mut S9,
		&mut S10, &mut S11, &mut S12, &mut S13, &mut S14, &mut S15
	],
	|e| tl![
		s::<[i8; 0]>(e),
		s::<[i8; 1]>(e),
		s::<[i8; 2]>(e),
		s::<[i8; 3]>(e),
		s::<[i8; 4]>(e),
		s::<[i8; 5]>(e),
		s::<[i8; 6]>(e),
		s::<[i8; 7]>(e),
		s::<[i8; 8]>(e),
		s::<[i8; 9]>(e),
		s::<[i8; 10]>(e),
		s::<[i8; 11]>(e),
		s::<[i8; 12]>(e),
		s::<[i8; 13]>(e),
		s::<[i8; 14]>(e),
		s::<[i8; 15]>(e)
	]
);

criterion_group! {
	//name = insertion;
	//config = Criterion::default().sample_size(20).measurement_time(std::time::Duration::from_secs(10));
	//targets =
	insertion,
		entity_table, dense_entity_value_bench, vec_entity_value_bench,
		dense_entity_dynamic_paged_multi_value_bench_16
	//	storage_new_u64_nil, storage_new_u64_i64,
	//	storage_insert_u64_nil, storage_insert_u64_128,
	//	storage_lookup_u64_nil, storage_lookup_u64_128,
}
