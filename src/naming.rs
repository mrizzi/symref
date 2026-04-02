/// Abbreviate an array field name for use in variable references.
///
/// Multi-word fields (containing `_`): first letter of each word.
///   e.g. `acceptance_criteria` -> `AC`
///
/// Single-word fields: first 3 characters.
///   e.g. `requirements` -> `REQ`
pub fn abbreviate_array_field(field: &str) -> String {
    if field.contains('_') {
        field
            .split('_')
            .filter_map(|w| w.chars().next())
            .collect::<String>()
            .to_uppercase()
    } else {
        field.chars().take(3).collect::<String>().to_uppercase()
    }
}

/// Generate a variable name for an array element (1-based index).
/// Returns the name with `$` prefix: `$PREFIX_FIELD_N`.
pub fn array_var_name(prefix: &str, field: &str, index: usize) -> String {
    let abbrev = abbreviate_array_field(field);
    format!("${}_{}_{}", prefix, abbrev, index)
}

/// Generate a variable name for a scalar (non-array) field.
/// Returns the name with `$` prefix: `$PREFIX_FIELD`.
pub fn scalar_var_name(prefix: &str, field: &str) -> String {
    format!("${}_{}", prefix, field.to_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abbreviate_single_word() {
        assert_eq!(abbreviate_array_field("requirements"), "REQ");
    }

    #[test]
    fn abbreviate_multi_word() {
        assert_eq!(abbreviate_array_field("acceptance_criteria"), "AC");
    }

    #[test]
    fn abbreviate_short_word() {
        assert_eq!(abbreviate_array_field("id"), "ID");
    }

    #[test]
    fn abbreviate_three_words() {
        assert_eq!(abbreviate_array_field("user_login_attempts"), "ULA");
    }

    #[test]
    fn array_var_name_simple() {
        assert_eq!(array_var_name("X7F", "requirements", 1), "$X7F_REQ_1");
    }

    #[test]
    fn array_var_name_multi_word() {
        assert_eq!(array_var_name("X7F", "acceptance_criteria", 1), "$X7F_AC_1");
    }

    #[test]
    fn scalar_var_name_simple() {
        assert_eq!(scalar_var_name("X7F", "background"), "$X7F_BACKGROUND");
    }

    #[test]
    fn abbreviate_empty_string() {
        assert_eq!(abbreviate_array_field(""), "");
    }

    #[test]
    fn abbreviate_single_char() {
        assert_eq!(abbreviate_array_field("x"), "X");
    }

    #[test]
    fn abbreviate_exactly_three_chars() {
        assert_eq!(abbreviate_array_field("foo"), "FOO");
    }

    #[test]
    fn abbreviate_leading_underscore() {
        // "_foo_bar" splits to ["", "foo", "bar"] → first chars: none, 'f', 'b' → "FB"
        assert_eq!(abbreviate_array_field("_foo_bar"), "FB");
    }

    #[test]
    fn abbreviate_trailing_underscore() {
        // "foo_bar_" splits to ["foo", "bar", ""] → first chars: 'f', 'b', none → "FB"
        assert_eq!(abbreviate_array_field("foo_bar_"), "FB");
    }

    #[test]
    fn abbreviate_consecutive_underscores() {
        // "a__b" splits to ["a", "", "b"] → first chars: 'a', none, 'b' → "AB"
        assert_eq!(abbreviate_array_field("a__b"), "AB");
    }

    #[test]
    fn abbreviate_single_word_longer_than_three() {
        assert_eq!(abbreviate_array_field("vulnerabilities"), "VUL");
    }

    #[test]
    fn array_var_name_large_index() {
        assert_eq!(array_var_name("P", "items", 100), "$P_ITE_100");
    }

    #[test]
    fn array_var_name_index_one() {
        assert_eq!(array_var_name("P", "items", 1), "$P_ITE_1");
    }

    #[test]
    fn scalar_var_name_mixed_case() {
        assert_eq!(scalar_var_name("X7F", "myField"), "$X7F_MYFIELD");
    }

    #[test]
    fn scalar_var_name_already_uppercase() {
        assert_eq!(scalar_var_name("X7F", "STATUS"), "$X7F_STATUS");
    }
}
