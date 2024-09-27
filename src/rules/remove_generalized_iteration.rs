use crate::nodes::{
    AssignStatement, BinaryExpression, BinaryOperator, Block, DoStatement, Expression,
    FieldExpression, FunctionCall, Identifier, IfBranch, IfStatement, LocalAssignStatement, Prefix,
    Statement, StringExpression, TupleArguments, TypedIdentifier, Variable,
};
use crate::process::{DefaultVisitor, NodeProcessor, NodeVisitor};
use crate::rules::{Context, RuleConfiguration, RuleConfigurationError, RuleProperties};

use super::runtime_variable::{
    RuntimeVariableBuilder, DEFAULT_RUNTIME_VARIABLE_FORMAT,
};
use super::{Rule, RuleProcessResult};

const VARIABLE_PREFIX: &str = "_DARKLUA_REMOVE_GENERALIZED_ITERATION";
const METATABLE_VARIABLE_NAME: &str = "_m";

struct Processor {
    iterator_variable_name: String,
    invariant_variable_name: String,
    control_variable_name: String,
    skip_block_once: bool,
}

fn get_type_condition(arg: Expression, type_name: &str) -> Box<BinaryExpression> {
    let type_call = Box::new(FunctionCall::new(
        Prefix::from_name("type"),
        TupleArguments::new(vec![arg]).into(),
        None,
    ));
    Box::new(BinaryExpression::new(
        BinaryOperator::Equal,
        Expression::Call(type_call),
        Expression::String(StringExpression::from_value(type_name)),
    ))
}

impl Processor {
    fn process_into_do(&self, block: &mut Block) -> Option<Statement> {
        for stmt in block.iter_mut_statements() {
            if let Statement::GenericFor(generic_for) = stmt {
                let exps = generic_for.mutate_expressions();
                if exps.len() == 1 {
                    let mut stmts: Vec<Statement> = Vec::new();
                    let iterator_typed_identifier =
                        TypedIdentifier::new(self.iterator_variable_name.as_str());
                    let iterator_identifier = iterator_typed_identifier.get_identifier().clone();

                    let invariant_typed_identifier =
                        TypedIdentifier::new(self.invariant_variable_name.as_str());
                    let invariant_identifier = invariant_typed_identifier.get_identifier().clone();

                    let control_typed_identifier =
                        TypedIdentifier::new(self.control_variable_name.as_str());
                    let control_identifier = control_typed_identifier.get_identifier().clone();

                    let iterator_local_assign = LocalAssignStatement::new(
                        vec![
                            iterator_typed_identifier,
                            invariant_typed_identifier,
                            control_typed_identifier,
                        ],
                        vec![exps[0].to_owned()],
                    );

                    let iterator_exp = Expression::Identifier(iterator_identifier.clone());
                    exps[0] = iterator_exp.clone();
                    let invariant_exp = Expression::Identifier(invariant_identifier.clone());
                    exps.push(invariant_exp);
                    let control_exp = Expression::Identifier(control_identifier.clone());
                    exps.push(control_exp);

                    let if_table_condition = get_type_condition(iterator_exp.clone(), "table");

                    let mt_typed_identifier = TypedIdentifier::new(METATABLE_VARIABLE_NAME);
                    let mt_identifier = mt_typed_identifier.get_identifier().clone();

                    let get_mt_call = FunctionCall::new(
                        Prefix::from_name("getmetatable"),
                        TupleArguments::new(vec![iterator_exp.clone()]).into(),
                        None,
                    );
                    let mt_local_assign = LocalAssignStatement::new(
                        vec![mt_typed_identifier],
                        vec![get_mt_call.into()],
                    );

                    let if_mt_table_condition =
                        get_type_condition(mt_identifier.clone().into(), "table");
                    let mt_iter = FieldExpression::new(
                        Prefix::Identifier(mt_identifier),
                        Identifier::new("__iter"),
                    );
                    let if_mt_iter_function_condition =
                        get_type_condition(mt_iter.clone().into(), "function");

                    let mt_iter_call = FunctionCall::from_prefix(Prefix::Field(Box::new(mt_iter)));
                    let assign_from_iter = AssignStatement::new(
                        vec![
                            Variable::Identifier(iterator_identifier.clone()),
                            Variable::Identifier(invariant_identifier.clone()),
                            Variable::Identifier(control_identifier.clone()),
                        ],
                        vec![mt_iter_call.into()],
                    );

                    let pairs_call = FunctionCall::new(
                        Prefix::from_name("pairs"),
                        TupleArguments::new(vec![iterator_identifier.clone().into()]).into(),
                        None,
                    );
                    let assign_from_pairs = AssignStatement::new(
                        vec![
                            Variable::Identifier(iterator_identifier),
                            Variable::Identifier(invariant_identifier),
                            Variable::Identifier(control_identifier),
                        ],
                        vec![pairs_call.into()],
                    );

                    let if_mt_table_block = Block::new(vec![assign_from_iter.into()], None);
                    let if_not_mt_table_block = Block::new(vec![assign_from_pairs.into()], None);
                    let if_mt_table_branch = IfBranch::new(
                        Expression::Binary(Box::new(BinaryExpression::new(
                            BinaryOperator::And,
                            Expression::Binary(if_mt_table_condition),
                            Expression::Binary(if_mt_iter_function_condition),
                        ))),
                        if_mt_table_block,
                    );
                    let if_mt_table_stmt =
                        IfStatement::new(vec![if_mt_table_branch], Some(if_not_mt_table_block));

                    let if_table_block =
                        Block::new(vec![mt_local_assign.into(), if_mt_table_stmt.into()], None);
                    let if_table_branch =
                        IfBranch::new(Expression::Binary(if_table_condition), if_table_block);
                    let if_table_stmt = IfStatement::new(vec![if_table_branch], None);

                    stmts.push(iterator_local_assign.into());
                    stmts.push(if_table_stmt.into());
                    stmts.push(generic_for.clone().into());

                    return Some(DoStatement::new(Block::new(stmts, None)).into());
                }
            }
        }
        None
    }
}

impl NodeProcessor for Processor {
    fn process_block(&mut self, block: &mut Block) {
        if self.skip_block_once {
            self.skip_block_once = false;
            return;
        }
        let do_stmt = self.process_into_do(block);
        if let Some(stmt) = do_stmt {
            self.skip_block_once = true;
            block.clear();
            block.insert_statement(0, stmt);
        }
    }
}

pub const REMOVE_GENERALIZED_ITERATION_RULE_NAME: &str = "remove_generalized_iteration";

/// A rule that removes generalized iteration.
#[derive(Debug, PartialEq, Eq)]
pub struct RemoveGeneralizedIteration {
    runtime_variable_format: String,
}

impl Default for RemoveGeneralizedIteration {
    fn default() -> Self {
        Self {
            runtime_variable_format: DEFAULT_RUNTIME_VARIABLE_FORMAT.to_string(),
        }
    }
}

impl Rule for RemoveGeneralizedIteration {
    fn process(&self, block: &mut Block, _: &Context) -> RuleProcessResult {
        let var_builder = RuntimeVariableBuilder::new(
            VARIABLE_PREFIX,
            self.runtime_variable_format.as_str(),
            format!("{block:?}").as_bytes(),
            Some(vec![METATABLE_VARIABLE_NAME.to_string()]),
        );
        let mut processor = Processor {
            iterator_variable_name: var_builder.build("iter")?,
            invariant_variable_name: var_builder.build("invar")?,
            control_variable_name: var_builder.build("control")?,
            skip_block_once: false,
        };
        DefaultVisitor::visit_block(block, &mut processor);
        Ok(())
    }
}

impl RuleConfiguration for RemoveGeneralizedIteration {
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
        REMOVE_GENERALIZED_ITERATION_RULE_NAME
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

    fn new_rule() -> RemoveGeneralizedIteration {
        RemoveGeneralizedIteration::default()
    }

    #[test]
    fn serialize_default_rule() {
        let rule: Box<dyn Rule> = Box::new(new_rule());

        assert_json_snapshot!("default_remove_generalized_iteration", rule);
    }

    #[test]
    fn configure_with_extra_field_error() {
        let result = json5::from_str::<Box<dyn Rule>>(
            r#"{
            rule: 'remove_generalized_iteration',
            prop: "something",
        }"#,
        );
        pretty_assertions::assert_eq!(result.unwrap_err().to_string(), "unexpected field 'prop'");
    }
}
