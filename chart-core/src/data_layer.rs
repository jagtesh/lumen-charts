use crate::chart_model::OhlcBar;

/// Data management layer — set, update, pop, and query bar data.
///
/// All operations maintain time-sorted order.
pub struct DataLayer {
    bars: Vec<OhlcBar>,
}

impl DataLayer {
    pub fn new() -> Self {
        DataLayer { bars: Vec::new() }
    }

    pub fn from_bars(bars: Vec<OhlcBar>) -> Self {
        let mut dl = DataLayer { bars };
        dl.sort();
        dl
    }

    /// Replace all data. Input is sorted by time.
    pub fn set_data(&mut self, bars: Vec<OhlcBar>) {
        self.bars = bars;
        self.sort();
    }

    /// Update or append a bar. If a bar with the same timestamp exists, it is
    /// replaced. Otherwise the new bar is inserted in sorted position.
    pub fn update(&mut self, bar: OhlcBar) {
        match self.find_by_time(bar.time) {
            Ok(idx) => {
                // Replace existing bar at this timestamp
                self.bars[idx] = bar;
            }
            Err(idx) => {
                // Insert at sorted position
                self.bars.insert(idx, bar);
            }
        }
    }

    /// Remove and return the last (most recent) bar, if any.
    pub fn pop(&mut self) -> Option<OhlcBar> {
        self.bars.pop()
    }

    /// Get a reference to all bars (time-sorted).
    pub fn bars(&self) -> &[OhlcBar] {
        &self.bars
    }

    /// Number of bars in the data layer.
    pub fn len(&self) -> usize {
        self.bars.len()
    }

    /// Whether the data layer is empty.
    pub fn is_empty(&self) -> bool {
        self.bars.is_empty()
    }

    /// Binary search for a bar by Unix timestamp.
    /// Returns Ok(index) if found, Err(insert_pos) if not.
    pub fn find_by_time(&self, time: i64) -> Result<usize, usize> {
        self.bars.binary_search_by_key(&time, |b| b.time)
    }

    /// Get bar at a specific index.
    pub fn bar_at(&self, index: usize) -> Option<&OhlcBar> {
        self.bars.get(index)
    }

    /// Get bar by timestamp (exact match).
    pub fn bar_at_time(&self, time: i64) -> Option<&OhlcBar> {
        self.find_by_time(time).ok().map(|i| &self.bars[i])
    }

    /// Get bars in a logical index range.
    pub fn bars_in_range(&self, start: usize, end: usize) -> &[OhlcBar] {
        let start = start.min(self.bars.len());
        let end = end.min(self.bars.len());
        &self.bars[start..end]
    }

    /// Sort bars by timestamp (ascending).
    fn sort(&mut self) {
        self.bars.sort_by_key(|b| b.time);
    }

    /// Take ownership of the internal bar vector (consuming).
    pub fn into_bars(self) -> Vec<OhlcBar> {
        self.bars
    }
}

impl Default for DataLayer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_bar(time: i64, close: f64) -> OhlcBar {
        OhlcBar {
            time,
            open: close - 1.0,
            high: close + 0.5,
            low: close - 1.5,
            close,
        }
    }

    #[test]
    fn test_set_data_sorts() {
        let mut dl = DataLayer::new();
        dl.set_data(vec![
            make_bar(3, 103.0),
            make_bar(1, 101.0),
            make_bar(2, 102.0),
        ]);
        assert_eq!(dl.len(), 3);
        assert_eq!(dl.bars()[0].time, 1);
        assert_eq!(dl.bars()[1].time, 2);
        assert_eq!(dl.bars()[2].time, 3);
    }

    #[test]
    fn test_update_append() {
        let mut dl = DataLayer::from_bars(vec![make_bar(1, 101.0), make_bar(2, 102.0)]);

        // Append a new bar (time=3, doesn't exist yet)
        dl.update(make_bar(3, 103.0));
        assert_eq!(dl.len(), 3);
        assert_eq!(dl.bars()[2].time, 3);
        assert!((dl.bars()[2].close - 103.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_update_replace() {
        let mut dl = DataLayer::from_bars(vec![make_bar(1, 101.0), make_bar(2, 102.0)]);

        // Replace bar at time=2
        dl.update(make_bar(2, 999.0));
        assert_eq!(dl.len(), 2); // Same count
        assert!((dl.bars()[1].close - 999.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_update_insert_middle() {
        let mut dl = DataLayer::from_bars(vec![make_bar(1, 101.0), make_bar(3, 103.0)]);

        // Insert between time=1 and time=3
        dl.update(make_bar(2, 102.0));
        assert_eq!(dl.len(), 3);
        assert_eq!(dl.bars()[0].time, 1);
        assert_eq!(dl.bars()[1].time, 2);
        assert_eq!(dl.bars()[2].time, 3);
    }

    #[test]
    fn test_pop() {
        let mut dl = DataLayer::from_bars(vec![make_bar(1, 101.0), make_bar(2, 102.0)]);
        let popped = dl.pop();
        assert!(popped.is_some());
        assert_eq!(popped.unwrap().time, 2);
        assert_eq!(dl.len(), 1);
    }

    #[test]
    fn test_pop_empty() {
        let mut dl = DataLayer::new();
        assert!(dl.pop().is_none());
    }

    #[test]
    fn test_find_by_time() {
        let dl = DataLayer::from_bars(vec![
            make_bar(10, 100.0),
            make_bar(20, 200.0),
            make_bar(30, 300.0),
        ]);
        assert_eq!(dl.find_by_time(20), Ok(1));
        assert_eq!(dl.find_by_time(25), Err(2)); // insert pos
        assert_eq!(dl.find_by_time(5), Err(0));
    }

    #[test]
    fn test_bar_at_time() {
        let dl = DataLayer::from_bars(vec![make_bar(10, 100.0), make_bar(20, 200.0)]);
        assert!(dl.bar_at_time(10).is_some());
        assert!((dl.bar_at_time(10).unwrap().close - 100.0).abs() < f64::EPSILON);
        assert!(dl.bar_at_time(15).is_none());
    }

    #[test]
    fn test_bars_in_range() {
        let dl = DataLayer::from_bars(vec![
            make_bar(1, 101.0),
            make_bar(2, 102.0),
            make_bar(3, 103.0),
            make_bar(4, 104.0),
        ]);
        let slice = dl.bars_in_range(1, 3);
        assert_eq!(slice.len(), 2);
        assert_eq!(slice[0].time, 2);
        assert_eq!(slice[1].time, 3);
    }

    #[test]
    fn test_from_bars_sorts() {
        let dl = DataLayer::from_bars(vec![make_bar(5, 105.0), make_bar(1, 101.0)]);
        assert_eq!(dl.bars()[0].time, 1);
        assert_eq!(dl.bars()[1].time, 5);
    }

    #[test]
    fn test_empty() {
        let dl = DataLayer::new();
        assert!(dl.is_empty());
        assert_eq!(dl.len(), 0);
        assert!(dl.bars().is_empty());
    }
}
