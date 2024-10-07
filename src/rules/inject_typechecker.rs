use log::error;
use serde::{Deserialize, Serialize};

use crate::process::{DefaultVisitor, NodeProcessor, NodeVisitor};
use crate::nodes::{Arguments, BinaryExpression, BinaryOperator, Block, Expression, FunctionCall, FunctionStatement, Identifier, IfBranch, IfStatement, LastStatement, Prefix, ReturnStatement, Statement, StringExpression, Type};
use super::{Context, FlawlessRule, RuleConfiguration, RuleConfigurationError, RuleProperties};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "snake_case", tag = "name")]
pub struct TypecheckerType {
    identifier: String,
    method: Option<String>,
    call: Option<String>
}

#[derive(Debug)]
struct Processor {
    use_typeof: String,
    return_errors: bool,
    error_message: String,
    error_call: String,
    types: Option<Vec<TypecheckerType>>,
}

impl Processor {
    pub fn new(use_typeof: String, return_errors: bool, error_message: String, error_call: String, types: Option<Vec<TypecheckerType>>) -> Self {
        Self {
            use_typeof: use_typeof,
            return_errors: return_errors,
            error_message: error_message,
            error_call: error_call,
            types: types,
        }
    }
}

impl NodeProcessor for Processor {

    fn process_function_statement(&mut self, fn_stmt: &mut FunctionStatement) {
        if !fn_stmt.has_parameters() {
            return;
        }

        // inject_typechecker 옵션 추가해야할거

        // 특정 주석 조건
        // 인덱스된 함수에만 추가하는가? 옵션

        // types: [
        //   {
        //       identifier: "Signal", // 만약 함수 파라매터 타입이 이거인가
        //       method: "is", // signal:is()로 체크함
        //       call: "type" // type(signal)로 체크함
        //   }
        // ]

        // 문자열, 숫자 호환성 고려 & runtime variable

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
                    let stmt_type_of = FunctionCall::new(Prefix::from_name(self.use_typeof.as_str()), stmt_argument.clone(), None);
                    let stmt_string_expression = StringExpression::from_value(type_name.get_name());

                    let error_message = self.error_message.as_str();
                    let format_string_count = error_message.matches("{get_type}").count();
                    let mut format_str = str::replace(error_message, "{argument}", index.to_string().as_str());
                    format_str = str::replace(format_str.as_str(), "{func_name}", fn_stmt.get_name().get_name().clone().into_name().as_str());
                    format_str = str::replace(format_str.as_str(), "{original_type}", type_name.get_name());
                    format_str = str::replace(format_str.as_str(), "{get_type}", "%s");

                    let mut format_string_argument = Arguments::String(StringExpression::from_value(format_str));
                    let format_typeof_call = stmt_type_of.clone();

                    for _ in 1..format_string_count+1 {
                        format_string_argument = format_string_argument.clone().with_argument(Expression::Call(Box::new(format_typeof_call.clone())));
                    }

                    let call_expression = Expression::Call(Box::new(FunctionCall::new(Prefix::from_name("string.format"), format_string_argument , None)));
                    let call_argument = Arguments::with_argument(Arguments::default(), call_expression);
                    
                    let return_error = FunctionCall::new(Prefix::from_name("error"), call_argument, None);
                    let mut error_vec: Vec<Statement> = Vec::new();
                    let mut option_last_statement: Option<LastStatement> = None;
                    
                    if self.error_call != "" {
                        error_vec.push(Statement::Call(FunctionCall::new(Prefix::from_name(self.error_call.as_str()), Arguments::default(), None)));
                    }

                    if !self.return_errors {
                        error_vec.push(Statement::Call(return_error.clone()));
                    } else {
                        let last_statement = ReturnStatement::one(Expression::Call(Box::new(return_error))).into();
                        option_last_statement = Some(last_statement);
                    }
                    
                    if self.types != None {
                        if let Some(vec_typechecker) = &self.types {
                            for typechcker_type in vec_typechecker {
                                if &typechcker_type.identifier != type_name.get_name() {
                                    let new_block = Block::new(error_vec.clone(), option_last_statement.clone());
                                    if_stmts.push(IfBranch::new(BinaryExpression::new(BinaryOperator::NotEqual, stmt_type_of.clone(), stmt_string_expression.clone()), new_block));
                                    continue;
                                }
                                // let mut use_method: bool = false;
                                if &typechcker_type.call != &None {
                                    if &typechcker_type.method != &None {
                                        error!("In the types setting, only one of call and method can be used.")
                                    } else {
                                        // use call
                                        if let Some(call) = typechcker_type.call.clone() {
                                            let identifier = typechcker_type.identifier.clone();
                                            let fn_call = FunctionCall::new(Prefix::from_name(call), stmt_argument.clone(), None);

                                            let new_block = Block::new(error_vec.clone(), option_last_statement.clone());
                                            if_stmts.push(IfBranch::new(BinaryExpression::new(BinaryOperator::NotEqual, fn_call, Expression::String(StringExpression::from_value(identifier))), new_block));
                                        }
                                    }
                                } else if &typechcker_type.method != &None {
                                    // use method
                                    if let Some(method) = typechcker_type.method.clone() {
                                        let call = FunctionCall::new(Prefix::from_name(format!(" not {}", method)), stmt_argument.clone(), None);

                                        let new_block = Block::new(error_vec.clone(), option_last_statement.clone());
                                        if_stmts.push(IfBranch::new(Expression::Call(Box::new(call)), new_block));
                                    }
                                } else {
                                    error!("One of method and call must have a value.")
                                }
                            }
                        }
                    } else {
                        let new_block = Block::new(error_vec.clone(), option_last_statement.clone());

                        if_stmts.push(IfBranch::new( BinaryExpression::new(BinaryOperator::NotEqual, stmt_type_of, stmt_string_expression.clone()), new_block));
                    }
                }
            }
        };

        fn_stmt.mutate_block().insert_statement(0, IfStatement::new(if_stmts, None));
    }
}

pub const INJECT_TYPECHECKER_RULE_NAME: &str = "inject_typechecker";

#[derive(Debug, PartialEq)]
pub struct InjectTypechecker {
    use_typeof: String,
    return_errors: bool,
    error_message: String,
    error_call: String,
    types: Option<Vec<TypecheckerType>>,
}

impl Default for InjectTypechecker {
    fn default() -> Self {
        Self {
            use_typeof: "typeof".to_string(),
            return_errors: false,
            error_message: "invalid argument #{argument} to '{func_name}' ({original_type} expected, got {get_type})".to_string(),
            error_call: "".to_string(),
            types: None,
        }
    }
}

impl FlawlessRule for InjectTypechecker {
    fn flawless_process(&self, block: &mut Block, _: &Context) {
        let mut processor = Processor::new(self.use_typeof.clone(), self.return_errors, self.error_message.clone(), self.error_call.clone(), self.types.clone()) ;
        DefaultVisitor::visit_block(block, &mut processor);
    }
}

impl RuleConfiguration for InjectTypechecker {
    fn configure(&mut self, properties: RuleProperties) -> Result<(), RuleConfigurationError> {

        for (key, value) in properties {
            match key.as_str() {
                "use_typeof" => {
                    self.use_typeof = if value.expect_bool(&key)? {
                        "typeof".to_string()
                    } else {
                        "type".to_string()
                    };
                },
                "return_errors" => {
                    self.return_errors = value.expect_bool(&key)?;
                },   
                "error_message" => {
                    
                    self.error_message = value.expect_string(&key)?;
                },
                "error_call" => {
                    self.error_call = value.expect_string(&key)?;
                },
                "types" => {
                    self.types = Some(value.expect_inject_typechecker_types(&key)?);
                },
                _ => return Err(RuleConfigurationError::UnexpectedProperty(key)),
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
