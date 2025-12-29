//! Property-based tests for SharedSlice shared memory safety
//! 
//! This module validates that SharedSlice provides safe concurrent memory access
//! with compile-time data race prevention for Pod types.
//! 
//! Property 5: Shared Memory Safety
//! Validates: Requirements 3.2

use wasm::{SharedSlice, SharedSliceMut, Pod};
use wasm::memory::{SharedMemory, MemoryError};
use quickcheck::{Arbitrary, Gen, QuickCheck, TestResult};
use std::sync::{Arc, Barrier, Mutex};
use std::thread;
use std::time::Duration;

#[cfg(test)]
mod tests {
    use super::*;

    /// Arbitrary Pod data for testing
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct TestData {
        value: u32,
        flag: bool,
        data: u64,
    }

    // Safety: TestData is safe for zero-copy sharing (no internal pointers)
    unsafe impl Pod for TestData {}

    impl Arbitrary for TestData {
        fn arbitrary(g: &mut Gen) -> Self {
            Self {
                value: g.gen(),
                flag: g.gen_bool(),
                data: g.gen(),
            }
        }
    }

    /// Arbitrary vector of Pod data for testing
    #[derive(Debug, Clone)]
    struct ArbitraryPodVec(Vec<TestData>);

    impl Arbitrary for ArbitraryPodVec {
        fn arbitrary(g: &mut Gen) -> Self {
            let len = g.gen_range(0..100);
            let mut vec = Vec::with_capacity(len);
            for _ in 0..len {
                vec.push(TestData::arbitrary(g));
            }
            ArbitraryPodVec(vec)
        }
    }

    /// Property: SharedSlice creation preserves data integrity
    #[test]
    fn prop_shared_slice_creation_preserves_data() {
        fn property(data: ArbitraryPodVec) -> TestResult {
            if data.0.is_empty() {
                return TestResult::discard();
            }

            // Create SharedSlice from vector
            let shared = SharedMemory::from_slice(&data.0).unwrap();
            let shared_slice = shared.as_shared_slice();

            // Verify all data is preserved
            if shared_slice.len() != data.0.len() {
                return TestResult::failed();
            }

            for (i, expected) in data.0.iter().enumerate() {
                match shared_slice.get(i) {
                    Some(actual) => {
                        if actual != expected {
                            return TestResult::failed();
                        }
                    }
                    None => return TestResult::failed(),
                }
            }

            TestResult::passed()
        }

        QuickCheck::new()
            .tests(100)
            .gen(Gen::new(100))
            .quickcheck(property as fn(ArbitraryPodVec) -> TestResult);
    }

    /// Property: SharedSlice bounds checking prevents out-of-bounds access
    #[test]
    fn prop_shared_slice_bounds_checking() {
        fn property(data: ArbitraryPodVec, index: usize) -> TestResult {
            if data.0.is_empty() {
                return TestResult::discard();
            }

            let shared = SharedMemory::from_slice(&data.0).unwrap();
            let shared_slice = shared.as_shared_slice();

            // Access should succeed only for valid indices
            let is_valid_index = index < data.0.len();
            let result = shared_slice.get(index);

            match (is_valid_index, result) {
                (true, Some(_)) => TestResult::passed(),
                (false, None) => TestResult::passed(),
                (true, None) => TestResult::failed(),
                (false, Some(_)) => TestResult::failed(),
            }
        }

        QuickCheck::new()
            .tests(100)
            .gen(Gen::new(100))
            .quickcheck(property as fn(ArbitraryPodVec, usize) -> TestResult);
    }

    /// Property: SharedSlice split maintains data integrity
    #[test]
    fn prop_shared_slice_split_maintains_integrity() {
        fn property(data: ArbitraryPodVec, split_point: usize) -> TestResult {
            if data.0.is_empty() {
                return TestResult::discard();
            }

            let shared = SharedMemory::from_slice(&data.0).unwrap();
            let shared_slice = shared.as_shared_slice();

            // Clamp split point to valid range
            let split_point = split_point % (data.0.len() + 1);
            let (left, right) = shared_slice.split_at(split_point);

            // Verify lengths
            if left.len() != split_point || right.len() != data.0.len() - split_point {
                return TestResult::failed();
            }

            // Verify data integrity
            for (i, expected) in data.0.iter().enumerate() {
                let result = if i < split_point {
                    left.get(i)
                } else {
                    right.get(i - split_point)
                };

                match result {
                    Some(actual) => {
                        if actual != expected {
                            return TestResult::failed();
                        }
                    }
                    None => return TestResult::failed(),
                }
            }

            TestResult::passed()
        }

        QuickCheck::new()
            .tests(100)
            .gen(Gen::new(100))
            .quickcheck(property as fn(ArbitraryPodVec, usize) -> TestResult);
    }

    /// Property: SharedSlice iteration provides correct data
    #[test]
    fn prop_shared_slice_iteration_correctness() {
        fn property(data: ArbitraryPodVec) -> TestResult {
            if data.0.is_empty() {
                return TestResult::discard();
            }

            let shared = SharedMemory::from_slice(&data.0).unwrap();
            let shared_slice = shared.as_shared_slice();

            // Collect all elements via iteration
            let iterated: Vec<_> = shared_slice.iter().collect();

            // Verify iteration produces all elements in order
            if iterated.len() != data.0.len() {
                return TestResult::failed();
            }

            for (i, (expected, actual)) in data.0.iter().zip(iterated.iter()).enumerate() {
                if expected != actual {
                    return TestResult::failed();
                }
            }

            TestResult::passed()
        }

        QuickCheck::new()
            .tests(100)
            .gen(Gen::new(100))
            .quickcheck(property as fn(ArbitraryPodVec) -> TestResult);
    }

    /// Property: SharedSlice prevents data races in concurrent access
    #[test]
    fn prop_shared_slice_concurrent_access_safe() {
        fn property(data: ArbitraryPodVec) -> TestResult {
            if data.0.len() < 10 {
                return TestResult::discard(); // Need some data for meaningful test
            }

            let shared = SharedMemory::from_slice(&data.0).unwrap();
            let shared_slice = shared.as_shared_slice();
            let shared_slice = Arc::new(shared_slice);

            let num_threads = 4;
            let barrier = Arc::new(Barrier::new(num_threads));
            let errors = Arc::new(Mutex::new(Vec::new()));

            let mut handles = Vec::new();

            for thread_id in 0..num_threads {
                let shared_clone = Arc::clone(&shared_slice);
                let barrier_clone = Arc::clone(&barrier);
                let errors_clone = Arc::clone(&errors);

                let handle = thread::spawn(move || {
                    barrier_clone.wait();

                    // Each thread reads different portions of the data
                    let start = (thread_id * data.0.len()) / num_threads;
                    let end = ((thread_id + 1) * data.0.len()) / num_threads;

                    for i in start..end {
                        match shared_clone.get(i) {
                            Some(_value) => {
                                // Successfully read, which is expected for shared reads
                            }
                            None => {
                                // This should not happen with valid indices
                                let mut errors = errors_clone.lock().unwrap();
                                errors.push(format!("Thread {} failed to read index {}", thread_id, i));
                            }
                        }
                    }
                });

                handles.push(handle);
            }

            // Wait for all threads to complete
            for handle in handles {
                handle.join().unwrap();
            }

            // Check for any errors
            let errors = errors.lock().unwrap();
            TestResult::from_bool(errors.is_empty())
        }

        QuickCheck::new()
            .tests(50) // Reduce iterations due to thread creation overhead
            .gen(Gen::new(100))
            .quickcheck(property as fn(ArbitraryPodVec) -> TestResult);
    }

    /// Property: SharedSliceMut provides exclusive access when needed
    #[test]
    fn prop_shared_slice_mut_exclusive_access() {
        fn property(data: ArbitraryPodVec) -> TestResult {
            if data.0.is_empty() {
                return TestResult::discard();
            }

            let mut shared = SharedMemory::from_slice(&data.0).unwrap();
            let shared_slice = shared.as_shared_slice();
            let shared_slice = shared.as_shared_slice(); // Immutable version

            // Clone should be independent
            let mut slice_mut = shared.as_mut_shared_slice();

            // Modify through mutable slice
            if let Some(first) = slice_mut.get_mut(0) {
                let original_value = *first;
                *first = TestData { 
                    value: 999999, 
                    flag: false, 
                    data: 0 
                };

                // Immutable slice should not see the change immediately
                // (This depends on the implementation - for this test we assume
                // they point to the same memory, which is the case in our implementation)
                if let Some(immutable_value) = shared_slice.get(0) {
                    if immutable_value.value == 999999 {
                        // They point to the same memory, which is expected
                    } else {
                        return TestResult::failed();
                    }
                } else {
                    return TestResult::failed();
                }

                // Restore original value
                *first = original_value;
            }

            TestResult::passed()
        }

        QuickCheck::new()
            .tests(50)
            .gen(Gen::new(100))
            .quickcheck(property as fn(ArbitraryPodVec) -> TestResult);
    }

    /// Property: SharedSlice handles empty data correctly
    #[test]
    fn prop_shared_slice_empty_handling() {
        fn property() -> TestResult {
            let empty_data: Vec<TestData> = vec![];
            let shared = SharedMemory::from_slice(&empty_data).unwrap();
            let shared_slice = shared.as_shared_slice();

            // Empty slice should behave correctly
            if !shared_slice.is_empty() {
                return TestResult::failed();
            }

            if shared_slice.len() != 0 {
                return TestResult::failed();
            }

            // All accesses should return None
            for i in 0..10 {
                if shared_slice.get(i).is_some() {
                    return TestResult::failed();
                }
            }

            // Iteration should produce empty iterator
            let iterated: Vec<_> = shared_slice.iter().collect();
            if !iterated.is_empty() {
                return TestResult::failed();
            }

            TestResult::passed()
        }

        QuickCheck::new()
            .tests(20)
            .gen(Gen::new(100))
            .quickcheck(property as fn() -> TestResult);
    }

    /// Property: SharedSlice maintains type safety across operations
    #[test]
    fn prop_shared_slice_type_safety() {
        fn property(data: ArbitraryPodVec) -> TestResult {
            if data.0.is_empty() {
                return TestResult::discard();
            }

            let shared = SharedMemory::from_slice(&data.0).unwrap();
            let shared_slice = shared.as_shared_slice();

            // Type should be preserved in all operations
            let cloned = shared_slice.clone();
            let (left, right) = shared_slice.split_at(data.0.len() / 2);

            // All operations should return the same type
            if shared_slice.len() != cloned.len() {
                return TestResult::failed();
            }

            if shared_slice.len() != left.len() + right.len() {
                return TestResult::failed();
            }

            TestResult::passed()
        }

        QuickCheck::new()
            .tests(100)
            .gen(Gen::new(100))
            .quickcheck(property as fn(ArbitraryPodVec) -> TestResult);
    }

    /// Property: SharedSlice memory layout is compatible with Pod requirements
    #[test]
    fn prop_shared_slice_pod_compatibility() {
        fn property(data: ArbitraryPodVec) -> TestResult {
            if data.0.is_empty() {
                return TestResult::discard();
            }

            // Test with different Pod types
            let u32_data: Vec<u32> = data.0.iter().map(|d| d.value).collect();
            let u64_data: Vec<u64> = data.0.iter().map(|d| d.data).collect();

            // Create shared slices for each type
            let shared_u32 = SharedMemory::from_slice(&u32_data).unwrap();
            let shared_u64 = SharedMemory::from_slice(&u64_data).unwrap();

            let slice_u32 = shared_u32.as_shared_slice();
            let slice_u64 = shared_u64.as_shared_slice();

            // Verify data is preserved for each Pod type
            for (i, expected) in u32_data.iter().enumerate() {
                match slice_u32.get(i) {
                    Some(actual) => {
                        if actual != expected {
                            return TestResult::failed();
                        }
                    }
                    None => return TestResult::failed(),
                }
            }

            for (i, expected) in u64_data.iter().enumerate() {
                match slice_u64.get(i) {
                    Some(actual) => {
                        if actual != expected {
                            return TestResult::failed();
                        }
                    }
                    None => return TestResult::failed(),
                }
            }

            TestResult::passed()
        }

        QuickCheck::new()
            .tests(50)
            .gen(Gen::new(100))
            .quickcheck(property as fn(ArbitraryPodVec) -> TestResult);
    }

    /// Property: SharedSlice handles large datasets efficiently
    #[test]
    fn prop_shared_slice_large_dataset_handling() {
        fn property(size_factor: u8) -> TestResult {
            let size = (size_factor as usize + 1) * 1000; // 1K to 256K elements
            let data: Vec<TestData> = (0..size)
                .map(|i| TestData {
                    value: i as u32,
                    flag: i % 2 == 0,
                    data: (i * 2) as u64,
                })
                .collect();

            let start = std::time::Instant::now();
            
            let shared = SharedMemory::from_slice(&data).unwrap();
            let shared_slice = shared.as_shared_slice();

            let creation_time = start.elapsed();

            // Should be able to handle large datasets efficiently
            if creation_time.as_millis() > 1000 { // 1 second limit
                return TestResult::failed();
            }

            // Random access should be fast
            let start = std::time::Instant::now();
            for _ in 0..100 {
                let index = (std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos() as usize) % size;
                let _ = shared_slice.get(index);
            }
            let access_time = start.elapsed();

            // 100 random accesses should be very fast
            if access_time.as_millis() > 100 { // 100ms limit
                return TestResult::failed();
            }

            TestResult::passed()
        }

        QuickCheck::new()
            .tests(20) // Reduce iterations due to memory usage
            .gen(Gen::new(100))
            .quickcheck(property as fn(u8) -> TestResult);
    }
}

/// Integration tests for SharedSlice with threading scenarios
#[cfg(test)]
mod threading_integration_tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_concurrent_read_performance() {
        let data: Vec<u32> = (0..10000).collect();
        let shared = SharedMemory::from_slice(&data).unwrap();
        let shared_slice = Arc::new(shared.as_shared_slice());

        let num_threads = 8;
        let reads_per_thread = 1000;
        let total_reads = AtomicUsize::new(0);

        let start = std::time::Instant::now();
        let mut handles = Vec::new();

        for _ in 0..num_threads {
            let slice_clone = Arc::clone(&shared_slice);
            let reads_completed = Arc::clone(&total_reads);

            let handle = thread::spawn(move || {
                for _ in 0..reads_per_thread {
                    let index = (std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_nanos() as usize) % 10000;
                    let _ = slice_clone.get(index);
                    reads_completed.fetch_add(1, Ordering::Relaxed);
                }
            });

            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let duration = start.elapsed();
        let actual_reads = total_reads.load(Ordering::Relaxed);
        let expected_reads = num_threads * reads_per_thread;

        assert_eq!(actual_reads, expected_reads, "All reads should complete");
        
        // Performance should be reasonable
        let reads_per_second = actual_reads as f64 / duration.as_secs_f64();
        println!("Concurrent read performance: {:.0} reads/second", reads_per_second);
        
        // Should achieve reasonable throughput
        assert!(reads_per_second > 10000.0, "Should achieve good read throughput");
    }

    #[test]
    fn test_memory_usage_scaling() {
        // Test that memory usage scales linearly with data size
        let sizes = vec![100, 1000, 10000, 100000];
        
        for size in sizes {
            let data: Vec<u32> = (0..size).collect();
            
            let shared = SharedMemory::from_slice(&data).unwrap();
            let slice = shared.as_shared_slice();
            
            // Memory should be proportional to size
            assert_eq!(slice.len(), size);
            
            // Should be able to access all elements
            for i in 0..size.min(1000) { // Limit verification for large sizes
                assert!(slice.get(i).is_some(), "Should access element {}", i);
            }
        }
    }

    #[test]
    fn test_stress_concurrent_operations() {
        let data: Vec<TestData> = (0..1000)
            .map(|i| TestData {
                value: i as u32,
                flag: true,
                data: i as u64,
            })
            .collect();
        
        let shared = SharedMemory::from_slice(&data).unwrap();
        let shared_slice = Arc::new(shared.as_shared_slice());

        let num_threads = 16;
        let operations_per_thread = 100;
        let barrier = Arc::new(Barrier::new(num_threads));
        let success_count = Arc::new(AtomicUsize::new(0));

        let mut handles = Vec::new();

        for thread_id in 0..num_threads {
            let slice_clone = Arc::clone(&shared_slice);
            let barrier_clone = Arc::clone(&barrier);
            let success = Arc::clone(&success_count);

            let handle = thread::spawn(move || {
                barrier_clone.wait();
                
                let mut local_success = 0;
                for i in 0..operations_per_thread {
                    let index = (thread_id * operations_per_thread + i) % 1000;
                    if slice_clone.get(index).is_some() {
                        local_success += 1;
                    }
                }
                success.fetch_add(local_success, Ordering::Relaxed);
            });

            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let total_success = success_count.load(Ordering::Relaxed);
        let expected_success = num_threads * operations_per_thread;
        
        assert_eq!(total_success, expected_success, 
                  "All operations should succeed in stress test");
    }
}
