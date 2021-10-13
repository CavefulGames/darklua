//! A module that contains the main [LuaGenerator](trait.LuaGenerator.html) trait
//! and its implementations.

use crate::nodes::{
    Expression, FieldExpression, FunctionCall, IndexExpression, Prefix, Statement,
    StringExpression, Variable,
};

const FORCE_QUOTED_STRING_THRESHOLD: usize = 40;

pub fn is_relevant_for_spacing(character: &char) -> bool {
    character.is_ascii_alphabetic() || character.is_digit(10) || *character == '_'
}

pub fn break_long_string(last_str: &str) -> bool {
    if let Some(last_char) = last_str.chars().last() {
        last_char == '['
    } else {
        false
    }
}

pub fn break_variable_arguments(last_string: &str) -> bool {
    if let Some('.') = last_string.chars().last() {
        true
    } else if let Some(first_char) = last_string.chars().next() {
        first_char == '.' || first_char.is_digit(10)
    } else {
        false
    }
}

pub fn break_minus(last_string: &str) -> bool {
    if let Some(last_char) = last_string.chars().last() {
        last_char == '-'
    } else {
        false
    }
}

pub fn break_concat(last_string: &str) -> bool {
    if let Some('.') = last_string.chars().last() {
        true
    } else if let Some(first_char) = last_string.chars().next() {
        first_char == '.' || first_char.is_digit(10)
    } else {
        false
    }
}

pub fn ends_with_prefix(statement: &Statement) -> bool {
    match statement {
        Statement::Assign(assign) => {
            if let Some(value) = assign.get_values().last() {
                expression_ends_with_call(value)
            } else {
                false
            }
        }
        Statement::CompoundAssign(assign) => expression_ends_with_call(assign.get_value()),
        Statement::Call(_) => true,
        Statement::Repeat(repeat) => expression_ends_with_call(repeat.get_condition()),
        Statement::LocalAssign(assign) => {
            if let Some(value) = assign.get_values().last() {
                expression_ends_with_call(value)
            } else {
                false
            }
        }
        _ => false,
    }
}

pub fn starts_with_parenthese(statement: &Statement) -> bool {
    match statement {
        Statement::Assign(assign) => {
            if let Some(variable) = assign.get_variables().first() {
                match variable {
                    Variable::Identifier(_) => false,
                    Variable::Field(field) => field_starts_with_parenthese(field),
                    Variable::Index(index) => index_starts_with_parenthese(index),
                }
            } else {
                false
            }
        }
        Statement::CompoundAssign(assign) => match assign.get_variable() {
            Variable::Identifier(_) => false,
            Variable::Field(field) => field_starts_with_parenthese(field),
            Variable::Index(index) => index_starts_with_parenthese(index),
        },
        Statement::Call(call) => call_starts_with_parenthese(call),
        _ => false,
    }
}

fn expression_ends_with_call(expression: &Expression) -> bool {
    match expression {
        Expression::Binary(binary) => expression_ends_with_call(binary.right()),
        Expression::Call(_)
        | Expression::Parenthese(_)
        | Expression::Identifier(_)
        | Expression::Field(_)
        | Expression::Index(_) => true,
        Expression::Unary(unary) => expression_ends_with_call(unary.get_expression()),
        _ => false,
    }
}

fn prefix_starts_with_parenthese(prefix: &Prefix) -> bool {
    match prefix {
        Prefix::Parenthese(_) => true,
        Prefix::Call(call) => call_starts_with_parenthese(call),
        Prefix::Field(field) => field_starts_with_parenthese(field),
        Prefix::Index(index) => index_starts_with_parenthese(index),
        Prefix::Identifier(_) => false,
    }
}

#[inline]
fn call_starts_with_parenthese(call: &FunctionCall) -> bool {
    prefix_starts_with_parenthese(call.get_prefix())
}

#[inline]
fn field_starts_with_parenthese(field: &FieldExpression) -> bool {
    prefix_starts_with_parenthese(field.get_prefix())
}

#[inline]
fn index_starts_with_parenthese(index: &IndexExpression) -> bool {
    prefix_starts_with_parenthese(index.get_prefix())
}

fn needs_escaping(character: char) -> bool {
    !(character.is_ascii_graphic() || character == ' ') || character == '\\'
}

fn needs_quoted_string(character: char) -> bool {
    !(character.is_ascii_graphic() || character == ' ' || character == '\n')
}

fn escape(character: char) -> String {
    match character {
        '\n' => "\\n".to_owned(),
        '\t' => "\\t".to_owned(),
        '\\' => "\\\\".to_owned(),
        '\r' => "\\r".to_owned(),
        '\u{7}' => "\\a".to_owned(),
        '\u{8}' => "\\b".to_owned(),
        '\u{B}' => "\\v".to_owned(),
        '\u{C}' => "\\f".to_owned(),
        _ => {
            if character.is_ascii() {
                format!("\\{}", character as u8)
            } else {
                format!("\\u{{{:x}}}", character as u32)
            }
        }
    }
}

pub fn write_string(string: &StringExpression) -> String {
    let value = string.get_value();

    if value.is_empty() {
        return "''".to_owned();
    }

    if value.len() == 1 {
        let character = value
            .chars()
            .next()
            .expect("string should have at least one character");
        match character {
            '\'' => return "\"'\"".to_owned(),
            '"' => return "'\"'".to_owned(),
            _ => {
                if needs_escaping(character) {
                    return format!("'{}'", escape(character));
                } else {
                    return format!("'{}'", character);
                }
            }
        }
    }

    if value.len() < FORCE_QUOTED_STRING_THRESHOLD || value.contains(needs_quoted_string) {
        write_quoted(value)
    } else {
        write_long_bracket(value)
    }
}

fn write_long_bracket(value: &str) -> String {
    let mut i = if value.ends_with(']') { 1 } else { 0 };
    let mut equals = "=".repeat(i);
    loop {
        if !value.contains(&format!("]{}]", equals)) {
            break;
        } else {
            i += 1;
            equals = "=".repeat(i);
        };
    }
    let needs_extra_new_line = if value.starts_with('\n') { "\n" } else { "" };
    format!("[{}[{}{}]{}]", equals, needs_extra_new_line, value, equals)
}

fn write_quoted(value: &str) -> String {
    let mut quoted = String::new();
    quoted.reserve(value.len() + 2);

    let quote_symbol = get_quote_symbol(value);
    quoted.push(quote_symbol);

    for character in value.chars() {
        if character == quote_symbol {
            quoted.push('\\');
            quoted.push(quote_symbol);
        } else if needs_escaping(character) {
            quoted.push_str(&escape(character));
        } else {
            quoted.push(character);
        }
    }

    quoted.push(quote_symbol);
    quoted.shrink_to_fit();
    quoted
}

fn get_quote_symbol(value: &str) -> char {
    if value.contains('"') {
        '\''
    } else if value.contains('\'') {
        '"'
    } else {
        '\''
    }
}

#[cfg(test)]
mod test {
    use super::*;

    mod write_string {
        use super::*;

        macro_rules! test_output {
            ($($name:ident($input:literal) => $value:literal),* $(,)?) => {
                $(
                    #[test]
                    fn $name() {
                        assert_eq!($value, write_string(&StringExpression::from_value($input)));
                    }
                )*
            };
        }

        test_output!(
            empty("") => "''",
            single_letter("a") => "'a'",
            single_digit("8") => "'8'",
            single_symbol("!") => "'!'",
            single_space(" ") => "' '",
            abc("abc") => "'abc'",
            three_spaces("   ") => "'   '",
            new_line("\n") => "'\\n'",
            bell("\u{7}") => "'\\a'",
            backspace("\u{8}") => "'\\b'",
            form_feed("\u{c}") => "'\\f'",
            tab("\t") => "'\\t'",
            carriage_return("\u{D}") => "'\\r'",
            vertical_tab("\u{B}") => "'\\v'",
            backslash("\\") => "'\\\\'",
            single_quote("'") => "\"'\"",
            double_quote("\"") => "'\"'",
            null("\0") => "'\\0'",
            escape("\u{1B}") => "'\\27'",
            unicode("\u{10FFFF}") => "'\\u{10ffff}'",
            im_cool("I'm cool") => "\"I'm cool\"",
            ends_with_closing_bracket("oof]") => "'oof]'",
            multiline_ends_with_closing_bracket("oof\noof]") => "'oof\\noof]'",
            large_multiline_does_not_end_with_closing_bracket("ooof\nooof\nooof\nooof\nooof\nooof\nooof\nooof\noof")
                => "[[ooof\nooof\nooof\nooof\nooof\nooof\nooof\nooof\noof]]",
            large_multiline_ends_with_closing_bracket("ooof\nooof\nooof\nooof\nooof\nooof\nooof\nooof\noof]")
                => "[=[ooof\nooof\nooof\nooof\nooof\nooof\nooof\nooof\noof]]=]",
            large_multiline_starts_with_new_line("\nooof\nooof\nooof\nooof\nooof\nooof\nooof\nooof\noof")
                => "[[\n\nooof\nooof\nooof\nooof\nooof\nooof\nooof\nooof\noof]]",

            large_multiline_with_unicode("\nooof\nooof\nooof\nooof\nooof\nooof\nooof\nooof\noof\u{10FFFF}")
                => "'\\nooof\\nooof\\nooof\\nooof\\nooof\\nooof\\nooof\\nooof\\noof\\u{10ffff}'",
        );
    }
}
