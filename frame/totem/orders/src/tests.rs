#![cfg(test)]

#[test]
fn basic_setup_works() {
	new_test_ext().execute_with(|| {
		run_to_block(1);

		for q in 0..3 {
			assert!(Queues::<Test>::get(q).is_empty());
		}
		assert_eq!(ActiveTotal::<Test>::get(), ActiveGiltsTotal {
			frozen: 0,
			proportion: Perquintill::zero(),
			index: 0,
			target: Perquintill::zero(),
		});
		assert_eq!(QueueTotals::<Test>::get(), vec![(0, 0); 3]);
	});
}
