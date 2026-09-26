//! Extraction of module specifiers referenced by JavaScript source text.
//!
//! Script fetching is synchronous (the HTML spec requires classic scripts to
//! execute in document order, blocking parsing), so embedders with asynchronous
//! networking prefetch the transitive module graph before execution and serve
//! the sources from memory via a custom [`ScriptFetcher`]. This module provides
//! the scanner used to discover that graph.

use boa_engine::Source;
use boa_engine::ast::scope::Scope;
use boa_engine::interner::Interner;
use boa_engine::parser::Parser;

/// Module specifiers referenced by `code`: static `import` declarations,
/// re-exports (`export ... from "..."`), and string-literal dynamic
/// `import("...")` calls, in source order.
///
/// Parse errors produce an empty list for the static declarations; literal
/// dynamic imports are still reported. Specifiers are returned unresolved, so
/// embedders should resolve them against the importing module's URL.
pub fn module_specifiers(code: &str) -> Vec<String> {
    // These keywords cannot contain escapes in valid JavaScript. Most classic
    // scripts have neither, so avoid constructing and resolving a full module
    // AST a second time just to find that there are no dependencies to fetch.
    if !code.contains("import") && !code.contains("export") {
        return Vec::new();
    }

    let mut interner = Interner::new();
    let mut parser = Parser::new(Source::from_bytes(code));
    let mut specifiers: Vec<String> = parser
        .parse_module(&Scope::new_global(), &mut interner)
        .map(|module| {
            module
                .items()
                .requests()
                .iter()
                .filter_map(|sym| Some(interner.resolve(*sym)?.utf8()?.to_string()))
                .collect()
        })
        .unwrap_or_default();
    specifiers.extend(dynamic_import_specifiers(code));
    specifiers
}

/// String-literal specifiers passed to dynamic `import(...)` calls.
fn dynamic_import_specifiers(code: &str) -> Vec<String> {
    let chars: Vec<char> = code.chars().collect();
    let mut specifiers = Vec::new();
    let mut index = 0;
    while index + 6 <= chars.len() {
        if chars[index..index + 6].iter().copied().eq("import".chars()) {
            let before = index.checked_sub(1).map(|position| chars[position]);
            let after = chars.get(index + 6).copied();
            let is_identifier = |character: Option<char>| {
                character.is_some_and(|character| {
                    character.is_ascii_alphanumeric() || character == '_' || character == '$'
                })
            };
            if !is_identifier(before) && !is_identifier(after) {
                let mut position = index + 6;
                while chars.get(position).is_some_and(|c| c.is_whitespace()) {
                    position += 1;
                }
                if chars.get(position) == Some(&'(') {
                    let mut literal = position + 1;
                    while chars.get(literal).is_some_and(|c| c.is_whitespace()) {
                        literal += 1;
                    }
                    if let Some(specifier) = read_string_literal(&chars, literal) {
                        specifiers.push(specifier);
                    }
                }
            }
        }
        index += 1;
    }
    specifiers
}

/// Read a (minimally unescaped) string literal starting at `start`.
fn read_string_literal(chars: &[char], start: usize) -> Option<String> {
    let quote = match chars.get(start) {
        Some(quote @ ('\'' | '"')) => *quote,
        _ => return None,
    };
    let mut value = String::new();
    let mut index = start + 1;
    while index < chars.len() {
        match chars[index] {
            '\\' => {
                index += 1;
                if let Some(character) = chars.get(index) {
                    value.push(*character);
                }
            }
            character if character == quote => return Some(value),
            character => value.push(character),
        }
        index += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::module_specifiers;

    #[test]
    fn collects_static_reexport_and_dynamic_imports() {
        let code = r#"
            import Fuse from "./fuse.min.mjs";
            import "./side-effect.js";
            export { thing } from "./reexport.js";
            const lazy = () => import("./chunk-a.js");
            const named = import(
                "./chunk-b.js"
            );
            import * as ns from "https://cdn.example.com/lib.mjs";
            import.meta.url;
        "#;
        assert_eq!(
            module_specifiers(code),
            vec![
                "./fuse.min.mjs",
                "./side-effect.js",
                "./reexport.js",
                "https://cdn.example.com/lib.mjs",
                "./chunk-a.js",
                "./chunk-b.js",
            ]
        );
    }

    #[test]
    fn unparseable_input_still_reports_literal_dynamic_imports() {
        let code = "this is not javascript; import(\"still-found.js\");";
        assert_eq!(module_specifiers(code), vec!["still-found.js"]);
    }

    #[test]
    fn import_identifier_is_not_treated_as_a_call() {
        let code = "const importx = 1; function importer() {}; import.meta.url;";
        assert!(module_specifiers(code).is_empty());
    }
}
