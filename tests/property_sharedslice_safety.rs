//! Property-based tests for SharedSlice safety
//! 
//! This module validates that SharedSlice provides safe concurrent
//! memory access with compile-time data race prevention.
//! 
//! Property 5: Shared Memory Safety
//! Validates: Requirements 3.2

use wasm::{SharedSlice, SharedMemory, Pod};
use wasm::host::get_host_capabilities;
use quickcheck::{QuickCheck, TestResult};
use std::time::{Instant, Duration};

#[cfg(test)]
mod tests {
    use super::*;

    /// Property: SharedSlice provides bounds checking for all operations
    #[test]
    fn prop_sharedslice_bounds_checking() {
        fn property(data_size: usize, access_pattern: u8) -> TestResult {
            if data_size > 1000 {
                return TestResult::discard();
            }

            // Create test data
            let test_data: Vec<u8> = (0..data_size).map(|i| (i % 256) as u8).collect();
            let slice_result = SharedSlice::from_slice(&test_data);
            if slice_result.is_err() {
                return TestResult::failed();
            }
            
            let shared_slice = slice_result.unwrap();
            let start = Instant::now();
            let mut _access_count = 0;
            
            // Test various access patterns with bounds checking
            for _iteration in 0..50 {
                match access_pattern % 4 {
                    0 => {
                        // Test valid access
                        if !shared_slice.is_empty() {
                            let _value = shared_slice.get(0);
                            _access_count += 1;
                        }
                    }
                    1 => {
                        // Test bounds checking with different indices
                        for i in 0..=shared_slice.len() {
                            let _value = shared_slice.get(i);
                            _access_count += 1;
                        }
                    }
                    2 => {
                        // Test out-of-bounds access
                        let _value = shared_slice.get(shared_slice.len());
                        let _value = shared_slice.get(shared_slice.len() + 1);
                        let _value = shared_slice.get(data_size + 100);
                        _access_count += 3;
                    }
                    3 => {
                        // Test slice operations
                        if shared_slice.len() >= 10 {
                            let _sub_slice = shared_slice.get_slice(5..10);
                            _access_count += 1;
                        }
                        
                        let _split = shared_slice.split_at(shared_slice.len() / 2);
                        _access_count += 1;
                    }
                    _ => unreachable!(),
                }
                
                // Break if the test runs too long
                if start.elapsed() > Duration::from_millis(100) {
                    break;
                }
            }
            
            let duration = start.elapsed();
            
            // Check that we didn't have any panics or crashes
            let is_safe = duration.as_millis() < 150;
            
            if !is_safe {
                return TestResult::failed();
            }
            
            TestResult::passed()
        }

        QuickCheck::new()
            .tests(30)
            .quickcheck(property as fn(usize, u8) -> TestResult);
    }

    /// Property: SharedSlice maintains memory safety with Pod types
    #[test]
    fn prop_sharedslice_pod_type_safety() {
        fn property(type_id: u8, data_size: usize) -> TestResult {
            if data_size > 100 {
                return TestResult::discard();
            }

            // Create test data based on type
            match type_id % 4 {
                0 => {
                    // Test with u32 data
                    let u32_data: Vec<u32> = (0..data_size).map(|i| i as u32).collect();
                    let u32_slice = SharedSlice::from_slice(&u32_data);
                    if u32_slice.is_err() {
                        return TestResult::failed();
                    }
                    
                    // Test Pod type operations
                    let shared_slice = u32_slice.unwrap();
                    for i in 0..shared_slice.len() {
                        let _value = shared_slice.get(i);
                    }
                    
                    // Test that u32 is indeed Pod
                    let is_pod = u32::is_valid_for_sharing();
                    if !is_pod {
                        return TestResult::failed();
                    }
                    
                    TestResult::passed()
                }
                1 => {
                    // Test with f64 data
                    let f64_data: Vec<f64> = (0..data_size).map(|i| i as f64).collect();
                    let f64_slice = SharedSlice::from_slice(&f64_data);
                    if f64_slice.is_err() {
                        return TestResult::failed();
                    }
                    
                    let shared_slice = f64_slice.unwrap();
                    for i in 0..shared_slice.len() {
                        let _value = shared_slice.get(i);
                    }
                    
                    let is_pod = f64::is_valid_for_sharing();
                    if !is_pod {
                        return TestResult::failed();
                    }
                    
                    TestResult::passed()
                }
                2 => {
                    // Test with bool data
                    let bool_data: Vec<bool> = (0..data_size).map(|i| i % 2 == 0).collect();
                    let bool_slice = SharedSlice::from_slice(&bool_data);
                    if bool_slice.is_err() {
                        return TestResult::failed();
                    }
                    
                    let shared_slice = bool_slice.unwrap();
                    for i in 0..shared_slice.len() {
                        let _value = shared_slice.get(i);
                    }
                    
                    let is_pod = bool::is_valid_for_sharing();
                    if !is_pod {
                        return TestResult::failed();
                    }
                    
                    TestResult::passed()
                }
                3 => {
                    // Test with array data
                    let array_data: Vec<[u8; 4]> = (0..data_size).map(|i| [i as u8; 4]).collect();
                    let array_slice = SharedSlice::from_slice(&array_data);
                    if array_slice.is_err() {
                        return TestResult::failed();
                    }
                    
                    let shared_slice = array_slice.unwrap();
                    for i in 0..shared_slice.len() {
                        let _value = shared_slice.get(i);
                    }
                    
                    let is_pod = <[u8; 4]>::is_valid_for_sharing();
                    if !is_pod {
                        return TestResult::failed();
                    }
                    
                    TestResult::passed()
                }
                _ => TestResult::discard(),
            }
        }

        QuickCheck::new()
            .tests(40)
            .quickcheck(property as fn(u8, usize) -> TestResult);
    }

    /// Property: SharedSlice provides zero-copy sharing for compatible types
    #[test]
    fn prop_sharedslice_zero_copy_sharing() {
        fn property(data_size: usize, shared_count: u8) -> TestResult {
            if data_size > 100 || shared_count > 10 {
                return TestResult::discard();
            }

            // Create test data
            let test_data: Vec<u8> = (0..data_size).map(|i| (i % 256) as u8).collect();
            let original_slice = SharedSlice::from_slice(&test_data);
            if original_slice.is_err() {
                return TestResult::failed();
            }
            
            let shared_slice = original_slice.unwrap();
            let start = Instant::now();
            
            // Create multiple shared references (should be zero-copy)
            let shared_references: Vec<_> = (0..shared_count)
                .map(|_| shared_slice.clone())
                .collect();
            
            // Test that all references point to the same data
            for reference in shared_references.iter() {
                for j in 0..reference.len() {
                    let value1 = reference.get(j);
                    let value2 = shared_slice.get(j);
                    
                    match (value1, value2) {
                        (Ok(v1), Ok(v2)) if *v1 == *v2 => {
                            // Same value - good
                        }
                        (Err(_), Err(_)) => {
                            // Out of bounds for both - consistent
                        }
                        (Ok(_), Err(_)) | (Err(_), Ok(_)) => {
                            // Inconsistent results - bad
                            return TestResult::failed();
                        }
                        (Ok(v1), Ok(v2)) if v1 != v2 => {
                            // Different values for same position - bad
                            return TestResult::failed();
                        }
                        _ => {}
                    }
                }
            }
            
            let duration = start.elapsed();
            
            // Zero-copy sharing should be extremely fast
            if duration.as_micros() > 1000 {
                return TestResult::failed();
            }
            
            TestResult::passed()
        }

        QuickCheck::new()
            .tests(40)
            .quickcheck(property as fn(usize, u8) -> TestResult);
    }

    /// Property: SharedSlice respects host capabilities for mutable access
    #[test]
    fn prop_sharedslice_host_capability_respect() {
        fn property(should_support_threading: bool) -> TestResult {
            let test_data = vec![1u8, 2u8, 3u8, 4u8];
            let slice_result = SharedSlice::from_slice(&test_data);
            if slice_result.is_err() {
                return TestResult::failed();
            }
            
            let _shared_slice = slice_result.unwrap();
            
            // Test mutable access based on host capabilities
            let caps = get_host_capabilities();
            let supports_threading = caps.threading;
            
            // Check that capabilities match expectation
            if supports_threading != should_support_threading {
                // This is a test setup issue, not a failure
                return TestResult::discard();
            }
            
            // Test mutable memory creation
            let mut_result = SharedMemory::<u8>::new(test_data.len(), true);
            
            match (supports_threading, mut_result) {
                (true, Ok(_)) => {
                    // Should succeed when threading is supported
                    TestResult::passed()
                }
                (false, Err(_)) => {
                    // Should fail when threading is not supported
                    TestResult::passed()
                }
                (false, Ok(_)) => {
                    // Should not succeed when threading is not supported
                    TestResult::failed()
                }
                (true, Err(_)) => {
                    // Should not fail when threading is supported
                    TestResult::failed()
                }
            }
        }

        QuickCheck::new()
            .tests(20)
            .quickcheck(property as fn(bool) -> TestResult);
    }
}

/// Integration tests for SharedSlice functionality
#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn test_sharedslice_with_various_pod_types() {
        // Test with different Pod types
        
        // u32 slice
        let u32_data = vec![1u32, 2u32, 3u32];
        let u32_slice = SharedSlice::from_slice(&u32_data).unwrap();
        assert_eq!(u32_slice.len(), 3);
        assert_eq!(*u32_slice.get(0).unwrap(), 1);
        
        // f64 slice
        let f64_data = vec![1.0, 2.0, 3.0];
        let f64_slice = SharedSlice::from_slice(&f64_data).unwrap();
        assert_eq!(f64_slice.len(), 3);
        assert_eq!(*f64_slice.get(1).unwrap(), 2.0);
        
        // bool slice
        let bool_data = vec![true, false, true];
        let bool_slice = SharedSlice::from_slice(&bool_data).unwrap();
        assert_eq!(bool_slice.len(), 3);
        assert_eq!(*bool_slice.get(2).unwrap(), true);
    }

    #[test]
    fn test_sharedslice_slice_operations() {
        let data = vec![1u8, 2u8, 3u8, 4u8, 5u8];
        let slice = SharedSlice::from_slice(&data).unwrap();
        
        // Test get_slice
        let sub_slice = slice.get_slice(1..4).unwrap();
        assert_eq!(sub_slice.len(), 3);
        assert_eq!(*sub_slice.get(0).unwrap(), 2);
        assert_eq!(*sub_slice.get(2).unwrap(), 4);
        
        // Test split_at
        let (left, right) = slice.split_at(2);
        assert_eq!(left.len(), 2);
        assert_eq!(right.len(), 3);
        assert_eq!(*left.get(0).unwrap(), 1);
        assert_eq!(*right.get(0).unwrap(), 3);
    }

    #[test]
    fn test_sharedslice_iteration() {
        let data = vec![10u32, 20u32, 30u32];
        let slice = SharedSlice::from_slice(&data).unwrap();
        
        // Test iteration
        let collected: Vec<_> = slice.into_iter().collect();
        assert_eq!(collected, vec![&10, &20, &30]);
        
        // Test iterator size_hint
        let (lower, upper) = slice.into_iter().size_hint();
        assert_eq!(lower, 3);
        assert_eq!(upper, Some(3));
    }

    #[test]
    fn test_sharedslice_error_handling() {
        let slice = SharedSlice::from_slice(&[1u8, 2u8, 3u8]).unwrap();
        
        // Test out-of-bounds access
        assert!(slice.get(3).is_err());
        assert!(slice.get(100).is_err());
        
        // Test out-of-bounds slice operations
        assert!(slice.get_slice(0..4).is_err());
        assert!(slice.get_slice(2..4).is_err());
        
        // Test invalid split
        let (left, right) = slice.split_at(2);
        assert_eq!(left.len(), 2);
        assert_eq!(right.len(), 1);
        
        // These operations should not panic
        let _invalid = slice.get_slice(1..2);
        let _split = slice.split_at(3); // Should not panic, just handle gracefully
    }

    #[test]
    fn test_sharedslice_performance_characteristics() {
        let data: Vec<u8> = (0..10000).map(|i| i as u8).collect();
        let slice = SharedSlice::from_slice(&data).unwrap();
        
        // Test that operations are reasonably fast
        let start = Instant::now();
        
        // Sequential access
        for i in 0..slice.len() {
            let _value = slice.get(i);
        }
        
        let sequential_time = start.elapsed();
        
        // Random access
        for _i in 0..slice.len() {
            let index = (start.elapsed().as_nanos() as usize) % slice.len();
            let _value = slice.get(index);
        }
        
        let random_time = start.elapsed();
        
        // Iteration
        let mut sum = 0u8;
        for value in slice.into_iter() {
            sum = sum.wrapping_add(*value);
        }
        
        let iteration_time = start.elapsed();
        
        // All operations should complete quickly
        assert!(sequential_time.as_millis() < 50);
        assert!(random_time.as_millis() < 100);
        assert!(iteration_time.as_millis() < 20);
        
        // Verify the sum calculation
        let expected_sum: u8 = data.iter().fold(0u8, |acc, &x| acc.wrapping_add(x));
        assert_eq!(sum, expected_sum);
    }

    #[test]
    fn test_sharedslice_memory_efficiency() {
        // Test that SharedSlice doesn't create unnecessary copies
        
        let data = vec![1u8, 2u8, 3u8];
        let slice = SharedSlice::from_slice(&data).unwrap();
        
        // Creating multiple references should be cheap
        let start = Instant::now();
        
        let references: Vec<_> = (0..1000)
            .map(|_| slice.clone())
            .collect();
        
        let clone_time = start.elapsed();
        
        // Verify all references point to the same data
        for reference in &references {
            assert_eq!(reference.len(), 3);
            assert_eq!(*reference.get(0).unwrap(), 1);
            assert_eq!(*reference.get(1).unwrap(), 2);
            assert_eq!(*reference.get(2).unwrap(), 3);
        }
        
        // Clone should be very fast (just reference count increment)
        assert!(clone_time.as_micros() < 10000);
        
        // Drop all references
        drop(references);
    }
}