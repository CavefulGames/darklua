use darklua_core::rules::Rule;

test_rule!(
    remove_redeclared_keys,
    json5::from_str::<Box<dyn Rule>>(
        r#"{
            rule: 'remove_redeclared_keys',
            runtime_variable_format: '{name}'
        }"#
    ).unwrap(),
    redeclared_value_and_index("local a = {1,[1]='A'}") => "local a = {'A'}",
    redeclared_field_and_index("local a = {x=1,['x']=2}") => "local a = {['x']=2}",
    redeclared_string_indexes("local a = {['x']=1,['x']=2}") => "local a = {['x']=2}",
    redeclared_numeric_indexes("local a = {[1]='A',[1]='B'}") => "local a = {'B'}",
    redeclared_values_and_indexes_special("local a = {1,2,3,[3]='A',[4]='B',[6]='C',[7]='D'}")
        => "local a = {1,2,'A','B',[6]='C',[7]='D'}",
    redeclared_side_effects("local a = {1,[f()]='A'}") => "local a = (function() local tbl = {1} tbl[f()] = 'A' return tbl end)()"
);

#[test]
fn deserialize_from_object_notation() {
    json5::from_str::<Box<dyn Rule>>(
        r#"{
        rule: 'remove_redeclared_keys',
		runtime_variable_format: '{name}'
    }"#,
    )
    .unwrap();
}
