use crate::nodes::{AssignStatement, Block, Expression, FunctionCall, FunctionExpression, Identifier, IndexExpression, LastStatement, LocalAssignStatement, ParentheseExpression, ReturnStatement, Statement, TableEntry, TableExpression, TableIndexEntry, TypedIdentifier, Variable};
use crate::process::{DefaultVisitor, Evaluator, LuaValue, NodeProcessor, NodeVisitor};
use crate::rules::{
    Context, FlawlessRule, RuleConfiguration, RuleConfigurationError, RuleProperties,
};

use super::verify_no_rule_properties;

const TABLE_VARIABLE_NAME: &str = "__DARKLUA_REMOVE_DUPLICATED_KEYS_tbl";

#[derive(Default)]
struct Processor {
	evaluator: Evaluator,
	table_variable_name: String,
	skip_next_table_exp: bool
}

impl Processor {
	fn skip(&mut self, active: bool) {
        self.skip_next_table_exp = active;
    }
}

use std::collections::HashMap;
use std::fmt::Debug;

impl NodeProcessor for Processor {
	fn process_expression(&mut self, exp: &mut Expression) {
		if let Expression::Table(table_exp) = exp {
			if self.skip_next_table_exp {
				self.skip(false);
				return;
			}
			let entries = table_exp.mutate_entries();
			let mut table = HashMap::new();
			let mut num_index: usize = 0;
			let mut side_effect_stmts: Vec<Statement> = Vec::new();
			let mut to_remove: Vec<usize> = Vec::new();

			for i in 0..entries.len() {
				match &entries[i] {
					TableEntry::Index(index_entry) => {
						let value = self.evaluator.evaluate(index_entry.get_key());
						match value {
							LuaValue::Number(lua_index) => {
								if lua_index.fract() == 0.0 && lua_index > 0.0 {
									let key = (lua_index as usize) - 1;
									if side_effect_stmts.is_empty() {
										if let Some(a) = table.get(&key) {
											to_remove.push(*a);
										}
										table.insert(key, i);
									} else {
										let assignment = AssignStatement::from_variable(
											IndexExpression::new(
												Identifier::new(self.table_variable_name.as_str()),
												key + 1
											),
											index_entry.get_value().clone()
										);
										side_effect_stmts.push(assignment.into());
									}
								}
							},
							LuaValue::Unknown => {
								let assignment = AssignStatement::from_variable(
									IndexExpression::new(
										Identifier::new(self.table_variable_name.as_str()),
										index_entry.get_key().clone()
									),
									index_entry.get_value().clone()
								);
								side_effect_stmts.push(assignment.into());
								to_remove.push(i);
							},
							_ => ()
						}
					},
					TableEntry::Value(_) => {
						if let Some(a) = table.get(&num_index) {
							to_remove.push(*a);
						}
						table.insert(num_index, i);
						num_index += 1;
					},
					_ => ()
				}
			}

			let mut keys: Vec<_> = table.keys().collect();
			keys.sort();
			let mut new_entries: Vec<TableEntry> = Vec::new();

			for i in keys {
				let v = table.get(i);
				if let Some(v) = v {
					let entry = &entries[*v];
					let new_entry = match entry {
						TableEntry::Index(index_entry) => {
							if *i <= num_index {
								Some(TableEntry::Value(index_entry.get_value().clone()))
							} else {
								Some(TableEntry::Index(index_entry.clone()))
							}
						},
						TableEntry::Value(exp) => {
							Some(TableEntry::Value(exp.clone()))
						},
						_ => None
					};
					if let Some(new_entry) = new_entry {
						new_entries.push(new_entry);
					}
				}
			}

			entries.clear();
			for ent in new_entries {
				entries.push(ent);
			}

			if !side_effect_stmts.is_empty() {
				let var = Identifier::new(self.table_variable_name.as_str());
				let table_stmt = TableExpression::new(entries.clone());
				self.skip(true);
				let local_assign_stmt = LocalAssignStatement::new(vec![var.clone().into()], vec![table_stmt.into()]);
				side_effect_stmts.insert(0, local_assign_stmt.into());
				let return_stmt = ReturnStatement::one(var);
				let func_block = Block::new(side_effect_stmts, Some(return_stmt.into()));
				let func = Expression::Function(FunctionExpression::from_block(func_block));
				let parenthese_func = ParentheseExpression::new(func);
				let func_call = FunctionCall::from_prefix(parenthese_func);
				let call_exp = Expression::Call(Box::new(func_call));
				*exp = call_exp;
			}
		}
	}
}

pub const REMOVE_DUPLICATED_KEYS_RULE_NAME: &str = "remove_duplicated_keys";

/// A rule that removes duplicated keys in table
#[derive(Debug, Default, PartialEq, Eq)]
pub struct RemoveDuplicatedKeys {}

impl FlawlessRule for RemoveDuplicatedKeys {
    fn flawless_process(&self, block: &mut Block, _: &Context) {
        let hash = blake3::hash(format!("{block:?}").as_bytes());
        let hash_hex = hex::encode(&hash.as_bytes()[..8]);
        let table_variable_name = TABLE_VARIABLE_NAME.to_string() + hash_hex.as_str();
        let mut processor = Processor {
			evaluator: Evaluator::default(),
            table_variable_name,
			skip_next_table_exp: false
        };
        DefaultVisitor::visit_block(block, &mut processor);
    }
}

impl RuleConfiguration for RemoveDuplicatedKeys {
    fn configure(&mut self, properties: RuleProperties) -> Result<(), RuleConfigurationError> {
        verify_no_rule_properties(&properties)?;

        Ok(())
    }

    fn get_name(&self) -> &'static str {
        REMOVE_DUPLICATED_KEYS_RULE_NAME
    }

    fn serialize_to_properties(&self) -> RuleProperties {
        RuleProperties::new()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::rules::Rule;

    use insta::assert_json_snapshot;

    fn new_rule() -> RemoveDuplicatedKeys {
        RemoveDuplicatedKeys::default()
    }

    #[test]
    fn serialize_default_rule() {
        let rule: Box<dyn Rule> = Box::new(new_rule());

        assert_json_snapshot!("default_remove_duplicated_keys", rule);
    }

    #[test]
    fn configure_with_extra_field_error() {
        let result = json5::from_str::<Box<dyn Rule>>(
            r#"{
            rule: 'remove_duplicated_keys',
            prop: "something",
        }"#,
        );
        pretty_assertions::assert_eq!(result.unwrap_err().to_string(), "unexpected field 'prop'");
    }
}