use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use crate::nodes::{
    Arguments, Block, FunctionCall, LocalAssignStatement, Prefix, StringExpression, TypedIdentifier,
};
use crate::rules::{Context, RuleConfiguration, RuleConfigurationError, RuleProperties};

use super::require::PathRequireMode;
use super::{verify_required_properties, RequireMode, Rule, RuleProcessResult};

use blake3;
use hex;
use path_slash::PathBufExt as _;
use pathdiff::diff_paths;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct Library {
    name: String,
    path: PathBuf,
}

pub const INJECT_LIBRARIES_RULE_NAME: &str = "inject_libraries";

/// A rule that removes trailing `nil` in local assignments.
#[derive(Debug, PartialEq, Eq)]
pub struct InjectLibraries {
    require_mode: RequireMode,
    libraries: Vec<Library>,
    path: PathBuf,
}

const DEFAULT_LIBRARIES_PATH: &str = "_DARKLUA_libs";

impl Default for InjectLibraries {
    fn default() -> Self {
        Self {
            require_mode: RequireMode::Path(Default::default()),
            libraries: Vec::new(),
            path: PathBuf::from_str(DEFAULT_LIBRARIES_PATH).unwrap(),
        }
    }
}

fn get_require_path(libs_path: &Path, path: &Path, context: &Context) -> PathBuf {
    let lib_file_stem = path
        .file_stem()
        .map(|x| OsString::from(x).into_string().unwrap())
        .unwrap();
    let lib_file_ext = path
        .extension()
        .map(|x| OsString::from(x).into_string().unwrap())
        .unwrap();

    let hash = blake3::hash(path.to_string_lossy().as_bytes());
    let hash_hex = hex::encode(&hash.as_bytes()[..8]);

    let lib_path = libs_path.join(format!("{}{}.{}", lib_file_stem, hash_hex, lib_file_ext));
    fs::copy(path, lib_path.as_path()).unwrap();
    let base_path = context
        .path
        .as_path()
        .parent()
        .expect("Could not find parent path of the source");
    let mut relative_path =
        diff_paths(lib_path.as_path(), base_path).expect("Could not resolve a path");
    relative_path.set_extension("");
    relative_path
}

impl Rule for InjectLibraries {
    fn process(&self, block: &mut Block, context: &Context) -> RuleProcessResult {
        let project_path = context
            .project_location
            .as_ref()
            .expect("Project path is required");
        let libs_path = project_path.join(self.path.as_path());
        fs::create_dir_all(&libs_path).unwrap();
        match self.require_mode.to_owned() {
            RequireMode::Path(_) => {
                for lib in &self.libraries {
                    let string_exp = StringExpression::from_value(
                        get_require_path(&libs_path, &lib.path, context).to_slash_lossy(),
                    );
                    let require_arg = Arguments::String(string_exp);

                    let require_call =
                        FunctionCall::new(Prefix::from_name("require"), require_arg, None);
                    let local_assignment = LocalAssignStatement::new(
                        vec![TypedIdentifier::new(lib.name.as_str())],
                        vec![require_call.into()],
                    );
                    block.insert_statement(0, local_assignment);
                }
            }
            RequireMode::Roblox(mut require_mode) => {
                require_mode
                    .initialize(context)
                    .map_err(|err| err.to_string())?;
                for lib in &self.libraries {
                    let require_path = get_require_path(&libs_path, &lib.path, context);
                    if let Some(require_arg) = require_mode
                        .generate_require(
                            require_path.as_path(),
                            &RequireMode::Path(PathRequireMode::new(require_path.to_slash_lossy())),
                            context,
                        )
                        .unwrap()
                    {
                        let require_call =
                            FunctionCall::new(Prefix::from_name("require"), require_arg, None);
                        let local_assignment = LocalAssignStatement::new(
                            vec![TypedIdentifier::new(lib.name.as_str())],
                            vec![require_call.into()],
                        );
                        block.insert_statement(0, local_assignment);
                    }
                }
            }
        }
        Ok(())
    }
}

impl RuleConfiguration for InjectLibraries {
    fn configure(&mut self, properties: RuleProperties) -> Result<(), RuleConfigurationError> {
        verify_required_properties(&properties, &["require_mode", "libraries"])?;

        for (key, value) in properties {
            match key.as_str() {
                "require_mode" => {
                    self.require_mode = value.expect_require_mode(&key)?;
                }
                "libraries" => {
                    self.libraries = value.expect_libraries(&key)?;
                }
                _ => return Err(RuleConfigurationError::UnexpectedProperty(key)),
            }
        }

        Ok(())
    }

    fn get_name(&self) -> &'static str {
        INJECT_LIBRARIES_RULE_NAME
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

    fn new_rule() -> InjectLibraries {
        InjectLibraries::default()
    }

    #[test]
    fn serialize_default_rule() {
        let rule: Box<dyn Rule> = Box::new(new_rule());

        assert_json_snapshot!("default_inject_libraries", rule);
    }

    #[test]
    fn configure_with_extra_field_error() {
        let result = json5::from_str::<Box<dyn Rule>>(
            r#"{
            rule: "inject_libraries",
            require_mode: {
                name: "roblox"
            },
            libraries: [
                {
                    name: "task",
                    path: "task.luau"
                }
            ],
            prop: "something",
        }"#,
        );
        pretty_assertions::assert_eq!(result.unwrap_err().to_string(), "unexpected field 'prop'");
    }
}
