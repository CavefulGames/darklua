use crate::process::{DefaultVisitor, NodeProcessor, NodeVisitor};
use crate::nodes::{Arguments, BinaryExpression, BinaryOperator, Block, Expression, FunctionCall, FunctionStatement, Identifier, IfBranch, IfStatement, LastStatement, Prefix, ReturnStatement, Statement, StringExpression, Type};
use super::{Context, FlawlessRule, RuleConfiguration, RuleConfigurationError, RuleProperties, RulePropertyValue};

#[derive(Debug, Clone)]
struct Processor {
    typecheck_prefix: Prefix,
    return_errors: bool,
    error_message: String,
}

impl Processor {
    pub fn new<P: Into<Prefix>>(prefix: P, return_errors: bool, error_message: String) -> Self {
        Self {
            typecheck_prefix: prefix.into(),
            return_errors: return_errors,
            error_message: error_message,
        }
    }
}

impl NodeProcessor for Processor {

    fn process_function_statement(&mut self, fn_stmt: &mut FunctionStatement) {
        if !fn_stmt.has_parameters() {
            return;
        }

        let mut if_stmts: Vec<IfBranch> = Vec::new();
        let mut index = 0;
        for stmt in fn_stmt.get_parameters() {
            index += 1;
            if let Some(t) = stmt.get_type() { 
                let type_name: Option<&Identifier> = if let Type::Name(t_name) = t { 
                    Some(t_name.get_type_name()) 
                } else {
                    None
                };
              
                if let Some(type_name) = type_name {
                    let stmt_expression = Expression::identifier(stmt.get_identifier().clone());
                    let stmt_argument = Arguments::with_argument(Arguments::default(), stmt_expression);
                    let stmt_type_of = FunctionCall::new(self.typecheck_prefix.clone(), stmt_argument.clone(), None);
                    let stmt_string_expression = StringExpression::from_value(type_name.get_name());
                    let stmt_if = BinaryExpression::new(BinaryOperator::NotEqual, stmt_type_of.clone(), stmt_string_expression);
                    let error_message = self.error_message.as_str();
                    let format_string_count = error_message.matches("{get_type}").count();
                    let mut format_str = str::replace(error_message, "{argument}", index.to_string().as_str());
                    format_str = str::replace(format_str.as_str(), "{func_name}", fn_stmt.get_name().get_name().clone().into_name().as_str());
                    format_str = str::replace(format_str.as_str(), "{original_type}", type_name.get_name());
                    format_str = str::replace(format_str.as_str(), "{get_type}", "%s");

                    // "invalid argument #{argument} to '{func_name}' ({original_type} expected, got {get_type}"

                    // let format_string = format!("invalid argument #{} to '{}' ({} expected, got %s)", 
                    //     index.to_string().as_str(),
                    //     fn_stmt.get_name().get_name().clone().into_name().as_str(),
                    //     type_name.get_name(),
                    // );

                    let mut format_string_argument = Arguments::String(StringExpression::from_value(format_str));
                    let format_typeof_call = stmt_type_of;
                    // let mut format_argument = format_string_argument.with_argument(Expression::Call(Box::new(format_typeof_call)));
                    for _ in 1..format_string_count+1 {
                        format_string_argument = format_string_argument.clone().with_argument(Expression::Call(Box::new(format_typeof_call.clone())));
                    }

                    let call_expression = Expression::Call(Box::new(FunctionCall::new(Prefix::from_name("string.format"), format_string_argument , None)));
                    let call_argument = Arguments::with_argument(Arguments::default(), call_expression);
                    
                    let return_error = FunctionCall::new(Prefix::from_name("error"), call_argument, None);
                    let mut error_vec: Vec<Statement> = Vec::new();
                    let mut option_last_statement: Option<LastStatement> = None;
                    if !self.return_errors {
                        error_vec.push(Statement::Call(return_error.clone()));
                    } else {
                        let last_statement = ReturnStatement::one(Expression::Call(Box::new(return_error))).into();
                        option_last_statement = Some(last_statement);
                    }
                    
                    let new_block = Block::new(error_vec, option_last_statement);
                    // Block::new(Statement::Call(return_error), last_statement);

                    if_stmts.push(IfBranch::new(stmt_if, new_block));
                }
            }
        };

        fn_stmt.mutate_block().insert_statement(0, IfStatement::new(if_stmts, None));
    }
}

pub const INJECT_TYPECHECKER_RULE_NAME: &str = "inject_typechecker";

#[derive(Debug, PartialEq, Eq)]
pub struct InjectTypechecker {
    prefix: Prefix,
    return_errors: bool,
    error_message: String,
}

impl Default for InjectTypechecker {
    fn default() -> Self {
        Self {
            prefix: Prefix::from_name("typeof"),
            return_errors: false,
            error_message: "invalid argument #{argument} to '{func_name}' ({original_type} expected, got {get_type}".to_string(),
        }
    }
}

impl FlawlessRule for InjectTypechecker {
    fn flawless_process(&self, block: &mut Block, _: &Context) {
        let mut processor = Processor::new(self.prefix.clone(), self.return_errors, self.error_message.clone());
        DefaultVisitor::visit_block(block, &mut processor);
    }
}

impl RuleConfiguration for InjectTypechecker {
    fn configure(&mut self, properties: RuleProperties) -> Result<(), RuleConfigurationError> {

        for (key, value) in properties {
            match key.as_str() {
                "use_typeof" => {
                    match value {
                        RulePropertyValue::Boolean(use_typeof) => {
                            if use_typeof {
                                self.prefix = Prefix::from_name("typeof")
                            } else {
                                self.prefix = Prefix::from_name("type")
                            }
                        },
                        _ => return Err(RuleConfigurationError::UnexpectedValueType(key)),
                    }
                },
                "return_errors" => {
                    match value {
                        RulePropertyValue::Boolean(use_typeof) => {
                            if use_typeof {
                                self.return_errors = false
                            } else {
                                self.return_errors = true
                            }
                        },
                        _ => return Err(RuleConfigurationError::UnexpectedValueType(key)),
                    }
                },      
                "error_message" => {
                    match value {
                        RulePropertyValue::String(message) => {
                            self.error_message = message
                        },
                        _ => return Err(RuleConfigurationError::UnexpectedValueType(key)),
                    }
                }
                _ => {
                    self.prefix = Prefix::from_name("typeof");
                    self.return_errors = false
                },
            }
        }

        Ok(())
    }

    fn get_name(&self) -> &'static str {
        INJECT_TYPECHECKER_RULE_NAME
    }

    fn serialize_to_properties(&self) -> super::RuleProperties {
        RuleProperties::new()
    }

}
