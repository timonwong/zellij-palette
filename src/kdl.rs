// Minimal escaper for KDL "quoted" string values.
//
// We emit `theme "<name>"` fragments through `reconfigure(...)`. If a
// theme name ever contains `"` or `\` it would break the surrounding KDL
// or, worse, smuggle extra directives into the live config. KDL's
// quoted-string grammar matches JSON's for these two characters, so
// the JSON-style escape is enough.
pub fn escape_kdl_string(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::escape_kdl_string;

    #[test]
    fn plain_names_pass_through() {
        assert_eq!(escape_kdl_string("gruvbox-dark"), "gruvbox-dark");
        assert_eq!(escape_kdl_string(""), "");
    }

    #[test]
    fn quotes_and_backslashes_are_escaped() {
        assert_eq!(escape_kdl_string(r#"weird"name"#), r#"weird\"name"#);
        assert_eq!(escape_kdl_string(r"with\backslash"), r"with\\backslash");
        assert_eq!(escape_kdl_string(r#"both"and\here"#), r#"both\"and\\here"#,);
    }

    #[test]
    fn injection_attempt_stays_inside_one_kdl_string() {
        // Theme name that tries to close the surrounding quote and
        // append another KDL directive. After escaping, every `"` and
        // `\` must be prefixed with a literal `\` so the KDL parser
        // keeps reading the same string value.
        let evil = "x\"\\ntheme \"real-one";
        let escaped = escape_kdl_string(evil);
        assert_eq!(escaped, r#"x\"\\ntheme \"real-one"#);
    }
}
