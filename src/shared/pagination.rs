use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct PaginationQuery {
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

impl PaginationQuery {
    pub fn normalized(&self, default_size: i64, max_size: i64) -> (i64, i64, i64) {
        let page = self.page.unwrap_or(1).max(1);
        let page_size = self.page_size.unwrap_or(default_size).clamp(1, max_size);
        let offset = (page - 1) * page_size;
        (page, page_size, offset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_page_one_and_given_size() {
        let q = PaginationQuery {
            page: None,
            page_size: None,
        };
        let (page, size, offset) = q.normalized(20, 100);
        assert_eq!((page, size, offset), (1, 20, 0));
    }

    #[test]
    fn explicit_page_and_size() {
        let q = PaginationQuery {
            page: Some(3),
            page_size: Some(15),
        };
        let (page, size, offset) = q.normalized(20, 100);
        assert_eq!((page, size, offset), (3, 15, 30));
    }

    #[test]
    fn page_zero_clamped_to_one() {
        let q = PaginationQuery {
            page: Some(0),
            page_size: Some(10),
        };
        let (page, _, offset) = q.normalized(20, 100);
        assert_eq!(page, 1);
        assert_eq!(offset, 0);
    }

    #[test]
    fn negative_page_clamped_to_one() {
        let q = PaginationQuery {
            page: Some(-5),
            page_size: Some(10),
        };
        let (page, _, _) = q.normalized(20, 100);
        assert_eq!(page, 1);
    }

    #[test]
    fn page_size_clamped_to_max() {
        let q = PaginationQuery {
            page: Some(1),
            page_size: Some(500),
        };
        let (_, size, _) = q.normalized(20, 100);
        assert_eq!(size, 100);
    }

    #[test]
    fn page_size_zero_clamped_to_one() {
        let q = PaginationQuery {
            page: Some(1),
            page_size: Some(0),
        };
        let (_, size, _) = q.normalized(20, 100);
        assert_eq!(size, 1);
    }

    #[test]
    fn offset_calculation_for_page_two() {
        let q = PaginationQuery {
            page: Some(2),
            page_size: Some(25),
        };
        let (_, _, offset) = q.normalized(20, 100);
        assert_eq!(offset, 25);
    }
}
