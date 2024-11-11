use std::collections::HashMap;

use regex::Regex;

pub(crate) type VariableMap<'a, 'b> = HashMap<&'a str, &'b str>;

pub(crate) struct PathVariables<'a> {
    variables: Vec<&'a str>,
    match_pattern: String,
}

impl<'a> PathVariables<'a> {
    pub(crate) fn new(mut source: &'a str) -> PathVariables<'a> {
        if source.starts_with('/') {
            source = &source[1..];
        }

        if source.ends_with('/') {
            source = &source[..source.len() - 1];
        }

        let mut match_pattern = String::from("^");
        let mut variables = Vec::new();

        for segment in source.split('/') {
            if segment.starts_with('{') && segment.ends_with('}') {
                let variable_name = &segment[1..segment.len() - 1];
                if !variable_name.is_empty() {
                    variables.push(variable_name);
                }
                match_pattern.push_str("/([^/]+)");
            } else {
                match_pattern.push('/');
                match_pattern.push_str(segment);
            }
        }

        match_pattern.push('$');

        PathVariables {
            variables,
            match_pattern,
        }
    }
}

pub(crate) fn extract_variables<'a, 'b>(
    variables: &PathVariables<'a>,
    path: &'b str,
) -> anyhow::Result<VariableMap<'a, 'b>> {
    let r = Regex::new(&variables.match_pattern)?;

    if !r.is_match(path) {
        anyhow::bail!("source and path do not match")
    }

    let mut map = VariableMap::new();

    if let Some(captures) = r.captures(path) {
        for (i, &var_name) in variables.variables.iter().enumerate() {
            if let Some(value) = captures.get(i + 1) {
                map.insert(var_name, value.as_str());
            }
        }
    }

    Ok(map)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::variable::{extract_variables, PathVariables};

    #[test]
    fn test_path_variables() {
        let src = "/foo/{a}/{b}/baz";
        let path = PathVariables::new(src);

        assert_eq!(path.match_pattern, "^/foo/([^/]+)/([^/]+)/baz$");
        assert_eq!(path.variables, ["a", "b"])
    }

    #[test]
    fn test_path_variables_empty() {
        let src = "/foo/{}/{}/baz";
        let path = PathVariables::new(src);

        assert_eq!(path.match_pattern, "^/foo/([^/]+)/([^/]+)/baz$");
        assert!(path.variables.is_empty())
    }

    #[test]
    fn test_extract_variables_should_successes() {
        let src = "/foo/{a}/{b}/baz";
        let path = "/foo/hello/world/baz";

        let expected = HashMap::from([("a", "hello"), ("b", "world")]);

        let variables = PathVariables::new(src);

        assert_eq!(extract_variables(&variables, path).unwrap(), expected)
    }

    #[test]
    fn test_extract_variables_should_fail() {
        let src = "/foo/{a}/{b}/baz";
        let path = "/foo/hello";

        let variables = PathVariables::new(src);

        assert!(extract_variables(&variables, path).is_err())
    }

    #[test]
    fn test_extract_variables_should() {
        let src = "/foo/{a}/{}/baz";
        let path = "/foo/hello/foo/baz";

        let expected = HashMap::from([("a", "hello")]);

        let variables = PathVariables::new(src);

        assert_eq!(extract_variables(&variables, path).unwrap(), expected)
    }

    #[test]
    fn test_extract_variables_should2() {
        let src = "/foo/{}/{}/baz";
        let path = "/foo/hello/foo/baz";

        let expected = HashMap::new();

        let variables = PathVariables::new(src);

        assert_eq!(extract_variables(&variables, path).unwrap(), expected)
    }
}
