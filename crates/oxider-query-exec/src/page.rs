//! One page of a result, plus the total it was taken from.
//!
//! Paging needs two answers - which rows, and how many there are altogether -
//! and getting them separately means the caller writes the counting query by
//! hand and keeps it in step with the real one. [`Page`] is what
//! [`Db::fetch_page`](crate::Db::fetch_page) returns so those two answers travel
//! together.

/// A slice of a result set, and the size of the set it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page<T> {
    /// The rows of this page.
    pub items: Vec<T>,
    /// How many rows the query returns in total, ignoring `LIMIT`/`OFFSET`.
    pub total: u64,
    /// The page size that was asked for.
    pub size: u64,
    /// The zero-based index of this page.
    pub number: u64,
}

impl<T> Page<T> {
    /// How many pages of this size the total divides into.
    ///
    /// Zero rows is zero pages, not one empty one, so a caller rendering "page
    /// 1 of N" does not print "1 of 1" over an empty table.
    pub fn total_pages(&self) -> u64 {
        if self.size == 0 {
            return 0;
        }
        self.total.div_ceil(self.size)
    }

    /// Whether a page after this one exists.
    pub fn has_next(&self) -> bool {
        self.number + 1 < self.total_pages()
    }

    /// Whether a page before this one exists.
    pub fn has_previous(&self) -> bool {
        self.number > 0 && self.total > 0
    }
}

#[cfg(test)]
mod tests {
    use super::Page;

    fn page(total: u64, size: u64, number: u64) -> Page<()> {
        Page {
            items: Vec::new(),
            total,
            size,
            number,
        }
    }

    #[test]
    fn a_partial_last_page_still_counts() {
        assert_eq!(page(21, 10, 0).total_pages(), 3);
        assert_eq!(page(20, 10, 0).total_pages(), 2);
    }

    #[test]
    fn an_empty_result_has_no_pages() {
        let empty = page(0, 10, 0);
        assert_eq!(empty.total_pages(), 0);
        assert!(!empty.has_next());
        assert!(!empty.has_previous());
    }

    /// A zero page size renders `LIMIT 0` and returns nothing, which is
    /// harmless. The accessors still have to stay total rather than dividing by
    /// it.
    #[test]
    fn a_zero_size_reports_no_pages_rather_than_dividing_by_zero() {
        assert_eq!(page(5, 0, 0).total_pages(), 0);
    }

    #[test]
    fn the_last_page_has_no_next() {
        assert!(page(21, 10, 1).has_next());
        assert!(!page(21, 10, 2).has_next());
        assert!(page(21, 10, 2).has_previous());
    }
}
