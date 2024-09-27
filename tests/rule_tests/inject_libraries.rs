use darklua_core::{rules::Rule, Resources};

use super::memory_resources;

test_rule!(
    inject_path_libraries,
    json5::from_str::<Box<dyn Rule>>(
        r#"{
        rule: "inject_libraries",
        require_mode: {
            name: "path"
        },
        path: "injected",
        no_hash: true,
        libraries: [
            {
                name: "buffer",
                path: "libs/buffer.luau"
            },
            {
                name: "task",
                path: "libs/task"
            }
        ]
    }"#,
    ).unwrap(),
    resources = memory_resources!(
        "libs/buffer.luau" => "return nil",
        "libs/task/init.luau" => "return nil",
    ),
    test_file_name = "src/test.lua",
    inject_library("print(buffer)") => "local buffer = require'../injected/buffer' print(buffer)",
    inject_directory_library("print(task)") => "local task = require'../injected/task' print(task)",
);

// test_rule!(
//     inject_roblox_libraries,
//     json5::from_str::<Box<dyn Rule>>(
//         r#"{
// 		rule: "inject_libraries",
// 		require_mode: {
// 			name: "roblox"
// 		},
// 		path: "injected",
// 		no_hash: true,
// 		libraries: [
// 			{
// 				name: "buffer",
// 				path: "libs/buffer.luau"
// 			},
// 			{
// 				name: "task",
// 				path: "libs/task"
// 			}
// 		]
// 	}"#,
//     ).unwrap(),
// 	resources = memory_resources!(
// 		"libs/buffer.luau" => "return nil",
// 		"libs/task/init.luau" => "return nil",
//     ),
//     test_file_name = "src/test.lua",
//     inject_library("print(buffer)") => "local buffer = require(script.Parent) print(buffer)",
//     inject_directory_library("print(task)") => "local task = require'../injected/task' print(task)",
// );

test_rule!(
    inject_libraries_removing,
    json5::from_str::<Box<dyn Rule>>(
        r#"{
        rule: "inject_libraries",
        require_mode: {
            name: "path"
        },
		path: "injected",
        no_hash: true,
        libraries: [
            {
                name: "buffer",
            }
        ]
    }"#,
    ).unwrap(),
    resources = memory_resources!(
        "libs/buffer.luau" => "return nil"
    ),
    test_file_name = "src/test.lua",
    remove_library("print(buffer)") => "local buffer = nil print(buffer)",
);

#[test]
fn deserialize_from_object_notation() {
    json5::from_str::<Box<dyn Rule>>(
        r#"{
        rule: 'inject_libraries',
        require_mode: {
            name: "path"
        },
        libraries: [],
		path: 'something'
    }"#,
    )
    .unwrap();
}
