use std::fmt::Debug;

use crate::nodes::{
    AssignStatement, Block, Expression, IfStatement, LastStatement,
    LocalAssignStatement, RepeatStatement, Statement, TypedIdentifier, UnaryExpression,
    UnaryOperator, Variable,
};
use crate::process::{DefaultVisitor, NodeProcessor, NodeVisitor};
use crate::rules::{
    Context, RuleConfiguration, RuleConfigurationError, RuleProperties,
};

use super::runtime_variable::RuntimeVariableBuilder;
use super::{Rule, RuleProcessResult};

#[derive(Default)]
struct Processor {
    break_variable_name: String,
    continue_variable_name: String
}

fn count_continue_break(block: &Block) -> (usize, usize) {
    let (mut continue_count, mut break_count) = if let Some(last_stmt) = block.get_last_statement()
    {
        (
            if let LastStatement::Continue(_) = last_stmt {
                1
            } else {
                0
            },
            if let LastStatement::Break(_) = last_stmt {
                1
            } else {
                0
            },
        )
    } else {
        (0, 0)
    };
    for stmt in block.iter_statements() {
        match stmt {
            Statement::If(if_stmt) => {
                for branch in if_stmt.iter_branches() {
                    let (c, b) = count_continue_break(branch.get_block());
                    continue_count += c;
                    break_count += b;
                }
            }
            Statement::Do(do_stmt) => {
                let (c, b) = count_continue_break(do_stmt.get_block());
                continue_count += c;
                break_count += b;
            }
            _ => {}
        }
    }
    (continue_count, break_count)
}

fn continues_to_breaks(block: &mut Block) {
    if let Some(last_stmt) = block.mutate_last_statement() {
        if matches!(last_stmt, LastStatement::Continue(_)) {
            *last_stmt = LastStatement::new_break();
        }
    }
    for stmt in block.iter_mut_statements() {
        match stmt {
            Statement::If(if_stmt) => {
                for branch in if_stmt.mutate_branches().iter_mut() {
                    continues_to_breaks(branch.mutate_block());
                }
            }
            Statement::Do(do_stmt) => {
                continues_to_breaks(do_stmt.mutate_block());
            }
            _ => {}
        }
    }
}

impl Processor {
    fn process(&self, block: &mut Block) {
        let (continue_count, break_count) = count_continue_break(block);

        if continue_count > 0 {
            let (mut stmts, break_variable_handler) = if break_count > 0 {
                let with_continue_statement = continue_count < break_count;
                let break_block = Block::new(vec![], Some(LastStatement::new_break()));
                let (break_variable_handler, var) = if with_continue_statement {
                    let var = TypedIdentifier::new(self.continue_variable_name.as_str());
                    (IfStatement::create(
                        UnaryExpression::new(
                            UnaryOperator::Not,
                            var.get_identifier().clone(),
                        ),
                        break_block,
                    ), var)
                } else {
                    let var = TypedIdentifier::new(self.break_variable_name.as_str());
                    (IfStatement::create(
                        var.get_identifier().clone(),
                        break_block,
                    ), var)
                };

                self.continues_with_breaks_to_breaks(block, with_continue_statement);

                let initial_value = Expression::False(None);
                let local_assign_stmt = LocalAssignStatement::new(vec![var], vec![initial_value]);

                (vec![local_assign_stmt.into()], Some(break_variable_handler))
            } else {
                continues_to_breaks(block);
                (Vec::new(), None)
            };
            let repeat_stmt = RepeatStatement::new(block.clone(), true);
            stmts.push(repeat_stmt.into());
            if let Some(break_variable_handler) = break_variable_handler {
                stmts.push(break_variable_handler.into());
            }
            *block = Block::new(stmts, None);
        }
    }

    fn continues_with_breaks_to_breaks(&self, block: &mut Block, with_continue_statement: bool) {
        if let Some(last_stmt) = block.mutate_last_statement() {
            let (continue_statement, break_statement) = if with_continue_statement {
                let var = Variable::new(self.continue_variable_name.as_str());
                (Some(AssignStatement::from_variable(var, true)), None)
            } else {
                let var = Variable::new(self.break_variable_name.as_str());
                (None, Some(AssignStatement::from_variable(var, true)))
            };
            match last_stmt {
                LastStatement::Continue(_) => {
                    if let Some(stmt) = continue_statement {
                        block.push_statement(stmt);
                    }
                    block.set_last_statement(LastStatement::new_break());
                }
                LastStatement::Break(_) => {
                    if let Some(stmt) = break_statement {
                        block.push_statement(stmt);
                    }
                    block.set_last_statement(LastStatement::new_break());
                }
                _ => {}
            }
        }
        for stmt in block.iter_mut_statements() {
            match stmt {
                Statement::If(if_stmt) => {
                    for branch in if_stmt.mutate_branches().iter_mut() {
                        self.continues_with_breaks_to_breaks(
                            branch.mutate_block(),
                            with_continue_statement,
                        );
                    }
                }
                Statement::Do(do_stmt) => {
                    self.continues_with_breaks_to_breaks(
                        do_stmt.mutate_block(),
                        with_continue_statement,
                    );
                }
                _ => {}
            }
        }
    }
}

impl NodeProcessor for Processor {
    fn process_statement(&mut self, statement: &mut Statement) {
        match statement {
            Statement::NumericFor(numeric_for) => self.process(numeric_for.mutate_block()),
            Statement::GenericFor(generic_for) => self.process(generic_for.mutate_block()),
            Statement::Repeat(repeat_stmt) => self.process(repeat_stmt.mutate_block()),
            Statement::While(while_stmt) => self.process(while_stmt.mutate_block()),
            _ => (),
        }
    }
}

pub const REMOVE_CONTINUE_RULE_NAME: &str = "remove_continue";

/// A rule that removes continue statements and convert into breaks.
#[derive(Debug, PartialEq, Eq)]
pub struct RemoveContinue {
    runtime_variable_format: String,
}

impl Default for RemoveContinue {
    fn default() -> Self {
        Self {
            runtime_variable_format: "_DARKLUA_REMOVE_CONTINUE_{name}{hash}".to_string()
        }
    }
}

impl Rule for RemoveContinue {
    fn process(&self, block: &mut Block, _: &Context) -> RuleProcessResult {
        let var_builder = RuntimeVariableBuilder::new(
            self.runtime_variable_format.as_str(),
            format!("{block:?}").as_bytes(),
            None
        );
        let mut processor = Processor {
            break_variable_name: var_builder.build("break")?,
            continue_variable_name: var_builder.build("continue")?
        };
        DefaultVisitor::visit_block(block, &mut processor);
		Ok(())
    }
}

impl RuleConfiguration for RemoveContinue {
    fn configure(&mut self, properties: RuleProperties) -> Result<(), RuleConfigurationError> {
        for (key, value) in properties {
            match key.as_str() {
                "runtime_variable_format" => {
                    self.runtime_variable_format = value.expect_string(&key)?;
                }
                _ => return Err(RuleConfigurationError::UnexpectedProperty(key)),
            }
        }

        Ok(())
    }

    fn get_name(&self) -> &'static str {
        REMOVE_CONTINUE_RULE_NAME
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

    fn new_rule() -> RemoveContinue {
        RemoveContinue::default()
    }

    #[test]
    fn serialize_default_rule() {
        let rule: Box<dyn Rule> = Box::new(new_rule());

        assert_json_snapshot!("default_remove_continue", rule);
    }

    #[test]
    fn configure_with_extra_field_error() {
        let result = json5::from_str::<Box<dyn Rule>>(
            r#"{
            rule: 'remove_continue',
            no_hash: false,
            prop: "something",
        }"#,
        );
        pretty_assertions::assert_eq!(result.unwrap_err().to_string(), "unexpected field 'prop'");
    }
}
