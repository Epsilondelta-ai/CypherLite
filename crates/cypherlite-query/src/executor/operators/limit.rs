// LimitOp and SkipOp

use crate::executor::Record;

/// Limit output to at most `count` records.
pub fn execute_limit(records: Vec<Record>, count: usize) -> Vec<Record> {
    records.into_iter().take(count).collect()
}

/// Skip the first `count` records.
pub fn execute_skip(records: Vec<Record>, count: usize) -> Vec<Record> {
    records.into_iter().skip(count).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executor::Value;

    fn make_records(n: usize) -> Vec<Record> {
        (0..n)
            .map(|i| {
                let mut r = Record::new();
                r.insert("i".to_string(), Value::Int64(i as i64));
                r
            })
            .collect()
    }

    #[test]
    fn test_limit_basic() {
        let records = make_records(5);
        let result = execute_limit(records, 3);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].get("i"), Some(&Value::Int64(0)));
        assert_eq!(result[2].get("i"), Some(&Value::Int64(2)));
    }

    #[test]
    fn test_limit_exceeds_count() {
        let records = make_records(3);
        let result = execute_limit(records, 10);
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_limit_zero() {
        let records = make_records(5);
        let result = execute_limit(records, 0);
        assert!(result.is_empty());
    }

    #[test]
    fn test_skip_basic() {
        let records = make_records(5);
        let result = execute_skip(records, 2);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].get("i"), Some(&Value::Int64(2)));
    }

    #[test]
    fn test_skip_all() {
        let records = make_records(3);
        let result = execute_skip(records, 10);
        assert!(result.is_empty());
    }

    #[test]
    fn test_skip_zero() {
        let records = make_records(3);
        let result = execute_skip(records, 0);
        assert_eq!(result.len(), 3);
    }
}
