// Built-in templates embedded into the binary.

use super::Template;
use crate::core::error::Result;

const NEWTOKI_JSON: &str = include_str!("../../templates/newtoki.json");

/// Look up a built-in template by name. Returns None if unknown.
pub fn get(name: &str) -> Option<Result<Template>> {
    let raw = match name {
        "newtoki" => NEWTOKI_JSON,
        _ => return None,
    };
    Some(Template::from_json_str(raw))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_newtoki() {
        let t = get("newtoki").expect("newtoki present").expect("parse");
        assert_eq!(t.name, "newtoki");
    }
}
