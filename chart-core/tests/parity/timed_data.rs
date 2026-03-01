/// Parity tests from LWC: tests/unittests/timed-data.spec.ts
///
/// Tests for visible timed values within a range.

/// LWC: visibleTimedValues — filter bars by time range
/// File: timed-data.spec.ts, line 27
/// visibleTimedValuesCase(rangeFrom, rangeTo, extendedRange, expectedFrom, expectedTo, times)
#[test]
fn lwc_visible_timed_values() {
    fn visible_count(range_from: i64, range_to: i64, times: &[i64]) -> usize {
        times
            .iter()
            .filter(|&&t| t >= range_from && t <= range_to)
            .count()
    }

    // LWC: (1, 3, false, ..., []) → 0 visible
    assert_eq!(visible_count(1, 3, &[]), 0);
    // LWC: (1, 3, false, ..., [1]) → 1 visible
    assert_eq!(visible_count(1, 3, &[1]), 1);
    // LWC: (1, 3, false, ..., [1, 2, 5]) → 2 visible (1 and 2)
    assert_eq!(visible_count(1, 3, &[1, 2, 5]), 2);
    // LWC: (1, 3, false, ..., [-1, 2, 5]) → 1 visible (2)
    assert_eq!(visible_count(1, 3, &[-1, 2, 5]), 1);
    // LWC: (1, 3, false, ..., [-1, 5]) → 0 visible
    assert_eq!(visible_count(1, 3, &[-1, 5]), 0);
    // LWC: (1, 3, false, ..., [4, 5]) → 0 visible
    assert_eq!(visible_count(1, 3, &[4, 5]), 0);
}

/// LWC: visibleTimedValues with extended range
/// File: timed-data.spec.ts, line 37
/// Extended range includes 1 bar before and after the visible range.
#[test]
fn lwc_visible_timed_values_extended() {
    fn visible_extended(range_from: i64, range_to: i64, times: &[i64]) -> Vec<i64> {
        // Find first bar >= range_from, but include one before it
        // Find last bar <= range_to, but include one after it
        let inner: Vec<usize> = times
            .iter()
            .enumerate()
            .filter(|(_, &t)| t >= range_from && t <= range_to)
            .map(|(i, _)| i)
            .collect();

        if inner.is_empty() {
            return vec![];
        }

        let first = if inner[0] > 0 { inner[0] - 1 } else { inner[0] };
        let last = if *inner.last().unwrap() < times.len() - 1 {
            inner.last().unwrap() + 1
        } else {
            *inner.last().unwrap()
        };

        times[first..=last].to_vec()
    }

    // (1, 3, true, ..., []) → empty
    assert_eq!(visible_extended(1, 3, &[]), Vec::<i64>::new());
    // (1, 3, true, ..., [1]) → [1]
    assert_eq!(visible_extended(1, 3, &[1]), vec![1]);
    // (1, 3, true, ..., [1, 2, 5]) → [1, 2, 5] (2 visible + 1 after)
    assert_eq!(visible_extended(1, 3, &[1, 2, 5]), vec![1, 2, 5]);
    // (1, 3, true, ..., [-2, -1, 2, 5, 6]) → [-1, 2, 5] (1 before, 1 visible, 1 after)
    assert_eq!(visible_extended(1, 3, &[-2, -1, 2, 5, 6]), vec![-1, 2, 5]);
}
