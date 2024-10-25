use std::collections::HashMap;

use regex::Regex;

use super::runtime_identifier::RuntimeIdentifierBuilder;
use super::{
    Context, Rule, RuleConfiguration, RuleConfigurationError, RuleProcessResult,
    RuleProperties,
};
use crate::nodes::{
    Arguments, BinaryExpression, BinaryOperator, Block, Expression, FunctionCall, FunctionStatement, Identifier, IfBranch, IfStatement, LastStatement, LocalFunctionStatement, Prefix, ReturnStatement, Statement, StringExpression, Type, TypedIdentifier
};
use crate::process::{DefaultVisitor, NodeProcessor, NodeVisitor};

struct ErrorCallBuilder {
    name: String,
    message_format: String,
}

impl ErrorCallBuilder {
    fn build(
        &self,
        index: usize,
        name: &str,
        expected_type: &str,
        actual_type: &str,
    ) -> FunctionCall {
        let index = index.to_string();
        let mut vars = HashMap::new();
        vars.insert("index".to_owned(), index.as_str());
        vars.insert("name".to_owned(), name);
        vars.insert("expected_type".to_owned(), expected_type);
        vars.insert("actual_type".to_owned(), actual_type);

        FunctionCall::new(
            Prefix::Identifier(Identifier::new(self.name.as_str())),
            Arguments::String(StringExpression::from_value(self.message_format.as_str())),
            None
        )
    }
}

trait TypeCheckerBuilder: std::fmt::Debug {
    fn build(&self, parameters: &Vec<TypedIdentifier>) -> (Statement);
}

struct TypeCheckerConfig {
    ignore_comment_pattern: Option<Regex>,
    type_variable_name: String,
    error_call_builder: ErrorCallBuilder,
    return_errors: bool,
    indexed_functions_only: bool,
}

fn get_possible_type_names(t: &Type) -> Option<Vec<(String, bool)>> {
    match t {
        Type::Array(_) => {
            Some(vec![("table".to_owned(), false)])
        }
        Type::False(_) => {
            Some(vec![("boolean".to_owned(), false)])
        }
        Type::Function(_) => {
            Some(vec![("function".to_owned(), false)])
        }
        Type::Intersection(intersection_type) => {
            let mut type_names: Vec<(String, bool)> = Vec::new();
            let left_type = intersection_type.get_left();
            if let Some(names) = get_possible_type_names(left_type) {
                for name in names {
                    type_names.push(name);
                }
            }

            let right_type = intersection_type.get_right();
            if let Some(names) = get_possible_type_names(right_type) {
                for name in names {
                    type_names.push(name);
                }
            }

            let first = &type_names[0];

            if type_names.iter().all(|x| x == first) {
                Some(vec![first.clone()])
            } else {
                None
            }
        }
        Type::Name(name) => {
            let name = name.get_type_name().get_name().as_str();
            match name {
                "nil" | "string" | "number" | "boolean" | "thread" | "buffer" => {
                    Some(vec![(name.to_owned(), false)])
                },
                _ => {
                    None
                }
            }
        }
        Type::Nil(_) => {
            Some(vec![("nil".to_owned(), false)])
        }
        Type::Optional(optional_type) => {
            let inner_type = optional_type.get_inner_type();
            if let Some(names) = get_possible_type_names(inner_type) {
                Some(names.iter().map(|(name, _)| (name.clone(), true)).collect())
            } else {
                None
            }
        }
        Type::Parenthese(parenthese_type) => {
            let inner_type = parenthese_type.get_inner_type();
            get_possible_type_names(inner_type)
        }
        Type::String(_) => {
            Some(vec![("nil".to_owned(), false)])
        }
        Type::Table(_) => {
            Some(vec![("table".to_owned(), false)])
        }
        Type::True(_) => {
            Some(vec![("boolean".to_owned(), false)])
        }
        Type::Union(union_type) => {
            let mut type_names: Vec<(String, bool)> = Vec::new();
            let left_type = union_type.get_left();
            if let Some(names) = get_possible_type_names(left_type) {
                for name in names {
                    type_names.push(name);
                }
            }

            let right_type = union_type.get_right();
            if let Some(names) = get_possible_type_names(right_type) {
                for name in names {
                    type_names.push(name);
                }
            }

            if type_names.is_empty() {
                None
            } else {
                Some(type_names)
            }
        }
        _ => {
            None
        }
    }
}

struct NonStrict {
    config: TypeCheckerConfig,
}

impl TypeCheckerBuilder for NonStrict {
    fn build(&self, parameters: &Vec<TypedIdentifier>) -> Statement {

    }
}

struct Strict {
    config: TypeCheckerConfig,
}

impl TypeCheckerBuilder for Strict {
    fn build(&self, parameters: &Vec<TypedIdentifier>) -> Statement {
        let branches: Vec<IfBranch> = Vec::new();
        parameters
            .iter()
            .filter_map(|param| param.get_type())
            .filter_map(|t| get_possible_type_names(t))
            .for_each(|t| { // condition: type(x) ~= "x"
                let condition = BinaryExpression::new(BinaryOperator::NotEqual, left, right);
                let if_branch = IfBranch::new(condition, block);
                branches.push(if_branch);
            });

        IfStatement::new(branches, None).into()
    }
}

struct FunctionProcessor {
    type_checker_builder: Box<dyn TypeCheckerBuilder>,
}

impl NodeProcessor for FunctionProcessor {
    fn process_statement(&mut self, stmt: &mut Statement) {
        match stmt {
            Statement::Function(func_stmt) => {
                if !func_stmt.has_parameters() {
                    return;
                }
                let stmt = self.type_checker_builder.build(func_stmt.get_parameters());
                let block = func_stmt.mutate_block();
                block.insert_statement(0, stmt);
            }
            Statement::LocalFunction(local_func_stmt) => {
                if !local_func_stmt.has_parameters() {
                    return;
                }
                let stmt = self.type_checker_builder.build(local_func_stmt.get_parameters());
                let block = local_func_stmt.mutate_block();
                block.insert_statement(0, stmt);
            }
            _ => {}
        }
    }
}

pub const INJECT_TYPE_CHECKER_RULE_NAME: &str = "inject_type_checker";

/// A rule that injects type checkers in function statements.
#[derive(Debug, PartialEq)]
pub struct InjectTypeChecker {
    error_call: String,
    error_message_format: String,
    return_errors: bool,
    ignore_comment_pattern: Option<String>,
    indexed_functions_only: bool,
    strict: bool,
    runtime_identifier_format: String,
    ignore_local_functions: bool,
}

impl Default for InjectTypeChecker {
    fn default() -> Self {
        Self {
            error_call: "error".to_owned(),
            error_message_format: "invalid argument #{index} to '{name}' ({expected_type} expected, got {actual_type})".to_owned(),
            return_errors: false,
            ignore_comment_pattern: None,
            indexed_functions_only: false,
            strict: false,
            runtime_identifier_format: "__DARKLUA_INJECT_TYPE_CHECKER_{name}{hash}".to_owned(),
            ignore_local_functions: false,
        }
    }
}

impl Rule for InjectTypeChecker {
    fn process(&self, block: &mut Block, context: &Context) -> RuleProcessResult {
        let pattern = if let Some(pattern) = &self.ignore_comment_pattern {
            Some(Regex::new(pattern.as_str()).map_err(|err| err.to_string())?)
        } else {
            None
        };

        let identifier_builder = RuntimeIdentifierBuilder::new(
            self.runtime_identifier_format.as_str(),
            context.original_code.as_bytes(),
            None,
        )?;

        let error_call_builder = ErrorCallBuilder {
            name: self.error_call.clone(),
            message_format: self.error_message_format.clone(),
        };

        let config = TypeCheckerConfig {
            ignore_comment_pattern: pattern,
            type_variable_name: identifier_builder.build("t")?,
            error_call_builder,
            return_errors: self.return_errors,
            indexed_functions_only: self.indexed_functions_only,
        };

        let type_checker_builder = Box::new(Strict { config });

        let mut function_processor = FunctionProcessor { type_checker_builder };

        DefaultVisitor::visit_block(block, &mut function_processor);

        Ok(())
    }
}

impl RuleConfiguration for InjectTypeChecker {
    fn configure(&mut self, properties: RuleProperties) -> Result<(), RuleConfigurationError> {
        for (key, value) in properties {
            match key.as_str() {
                "error_call" => {
                    self.error_call = value.expect_string(&key)?;
                }
                "error_message_format" => {
                    self.error_message_format = value.expect_string(&key)?;
                }
                "return_errors" => {
                    self.return_errors = value.expect_bool(&key)?;
                }
                "ignore_comment_pattern" => {
                    self.ignore_comment_pattern = Some(value.expect_string(&key)?);
                }
                "indexed_functions_only" => {
                    self.indexed_functions_only = value.expect_bool(&key)?;
                }
                "strict" => {
                    self.strict = value.expect_bool(&key)?;
                }
                "ignore_local_functions" => {
                    self.ignore_local_functions = value.expect_bool(&key)?;
                }
                "runtime_identifier" => {
                    self.runtime_identifier_format = value.expect_string(&key)?;
                }
                _ => return Err(RuleConfigurationError::UnexpectedProperty(key)),
            }
        }

        Ok(())
    }

    fn get_name(&self) -> &'static str {
        INJECT_TYPE_CHECKER_RULE_NAME
    }

    fn serialize_to_properties(&self) -> super::RuleProperties {
        RuleProperties::new()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::rules::Rule;

    use insta::assert_json_snapshot;

    fn new_rule() -> InjectTypeChecker {
        InjectTypeChecker::default()
    }

    #[test]
    fn serialize_default_rule() {
        let rule: Box<dyn Rule> = Box::new(new_rule());

        assert_json_snapshot!("default_inject_type_checker", rule);
    }

    #[test]
    fn configure_with_extra_field_error() {
        let result = json5::from_str::<Box<dyn Rule>>(
            r#"{
            rule: 'inject_type_checker',
            error_call: 'error',
            error_message_format: 'foo',
            return_errors: false,
            indexed_functions_only: false,
            strict: true,
            ignore_local_functions: false,
            runtime_identifier: "{name}",
            prop: "something",
        }"#,
        );
        pretty_assertions::assert_eq!(result.unwrap_err().to_string(), "unexpected field 'prop'");
    }
}
