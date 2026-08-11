//! Shared helper functions.

/// Parse a comma-separated ID list.
///
/// # Arguments
///
/// * `value` - Optional comma-separated integer IDs
///
/// # Returns
///
/// Parsed IDs, or None when no value is provided.
pub(crate) fn parse_comma_separated_ids(value: Option<&str>) -> Result<Option<Vec<usize>>, String> {
    let Some(value) = value else {
        return Ok(None);
    };

    let mut ids = Vec::new();
    for value in value.split(',') {
        let value = value.trim();
        if value.is_empty() {
            return Err("ID cannot be empty".to_string());
        }

        let id = value
            .parse::<usize>()
            .map_err(|_| format!("Invalid ID: {}", value))?;
        if ids.contains(&id) {
            return Err(format!("Duplicate ID: {}", id));
        }
        ids.push(id);
    }

    Ok(Some(ids))
}

#[cfg(test)]
mod tests {
    use super::parse_comma_separated_ids;

    #[test]
    fn parses_optional_ids() {
        assert_eq!(
            parse_comma_separated_ids(Some("0, 2")).unwrap(),
            Some(vec![0, 2])
        );
        assert_eq!(parse_comma_separated_ids(None).unwrap(), None);
    }

    #[test]
    fn rejects_invalid_ids() {
        assert!(parse_comma_separated_ids(Some("0,,2")).is_err());
        assert!(parse_comma_separated_ids(Some("0,a")).is_err());
        assert!(parse_comma_separated_ids(Some("0,0")).is_err());
    }
}
