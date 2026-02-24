//! CLI fuzzing tests using arbitrary-generated CLI instances.

use arbitrary::Arbitrary;
use facet::Facet;
use facet::Type;
use facet::UserType;
use rand::TryRngCore;
use rand::rngs::OsRng;
use vanity::cli::Cli;
use vanity::cli::ToArgs;

#[derive(Clone, Debug)]
struct CommandBranch {
    cli_name: String,
    effective_name: String,
    node: CommandNode,
}

#[derive(Clone, Debug, Default)]
struct CommandNode {
    positional_count: usize,
    named_flag_consumes_value: std::collections::BTreeMap<String, bool>,
    subcommands: Vec<CommandBranch>,
}

fn parse_cli_from_args(args: &[std::ffi::OsString]) -> Result<Cli, figue::DriverError> {
    let cli_args = args
        .iter()
        .map(|arg| arg.to_string_lossy().to_string())
        .collect::<Vec<_>>();

    figue::Driver::new(
        figue::builder::<Cli>()
            .expect("schema should be valid")
            .cli(|c| c.args(cli_args).strict())
            .build(),
    )
    .run()
    .into_result()
    .map(|output| output.get_silent())
}

fn to_kebab_case(name: &str) -> String {
    let mut out = String::new();
    let mut previous_is_alphanumeric = false;

    for character in name.chars() {
        if character == '_' {
            out.push('-');
            previous_is_alphanumeric = false;
            continue;
        }

        if character.is_ascii_uppercase() {
            if previous_is_alphanumeric {
                out.push('-');
            }
            out.push(character.to_ascii_lowercase());
            previous_is_alphanumeric = true;
            continue;
        }

        out.push(character);
        previous_is_alphanumeric = character.is_ascii_alphanumeric();
    }

    out
}

fn normalize_command_token(token: &str) -> String {
    token.replace('_', "-").to_ascii_lowercase()
}

fn unwrap_option_shape(mut shape: &'static facet::Shape) -> &'static facet::Shape {
    while let Ok(option_def) = shape.def.into_option() {
        shape = option_def.t;
    }
    shape
}

fn shape_struct_fields(shape: &'static facet::Shape) -> Option<&'static [facet::Field]> {
    let shape = unwrap_option_shape(shape);
    match shape.ty {
        Type::User(UserType::Struct(struct_type)) => Some(struct_type.fields),
        _ => None,
    }
}

fn shape_enum_variants(shape: &'static facet::Shape) -> Option<&'static [facet::Variant]> {
    let shape = unwrap_option_shape(shape);
    match shape.ty {
        Type::User(UserType::Enum(enum_type)) => Some(enum_type.variants),
        _ => None,
    }
}

fn field_is_bool_flag(field: &facet::Field) -> bool {
    let shape = unwrap_option_shape(field.shape());
    shape.type_identifier.eq_ignore_ascii_case("bool")
}

fn node_from_shape(shape: &'static facet::Shape) -> CommandNode {
    shape_struct_fields(shape).map_or_else(CommandNode::default, node_from_fields)
}

fn node_from_variant(variant: &facet::Variant) -> CommandNode {
    if variant.data.fields.is_empty() {
        return CommandNode::default();
    }

    let has_direct_arg_attributes = variant.data.fields.iter().any(|field| {
        field.has_attr(Some("args"), "positional")
            || field.has_attr(Some("args"), "named")
            || field.has_attr(Some("args"), "subcommand")
    });

    if has_direct_arg_attributes {
        return node_from_fields(variant.data.fields);
    }

    if variant.data.fields.len() == 1 {
        return node_from_shape(variant.data.fields[0].shape());
    }

    CommandNode::default()
}

fn node_from_fields(fields: &'static [facet::Field]) -> CommandNode {
    let mut node = CommandNode::default();

    for field in fields {
        if field.has_attr(Some("args"), "positional") {
            node.positional_count += 1;
            continue;
        }

        if field.has_attr(Some("args"), "named") {
            let flag_name = to_kebab_case(field.effective_name());
            let consumes_value =
                !field.has_attr(Some("args"), "counted") && !field_is_bool_flag(field);
            node.named_flag_consumes_value
                .insert(flag_name, consumes_value);
            continue;
        }

        if field.has_attr(Some("args"), "subcommand") {
            if let Some(variants) = shape_enum_variants(field.shape()) {
                for variant in variants {
                    node.subcommands.push(CommandBranch {
                        cli_name: to_kebab_case(variant.effective_name()),
                        effective_name: variant.effective_name().to_owned(),
                        node: node_from_variant(variant),
                    });
                }
            }
        }
    }

    node
}

fn collect_command_paths(root: &CommandNode) -> Vec<Vec<String>> {
    fn visit(node: &CommandNode, current: &mut Vec<String>, output: &mut Vec<Vec<String>>) {
        if node.subcommands.is_empty() {
            if !current.is_empty() {
                output.push(current.clone());
            }
            return;
        }

        for branch in &node.subcommands {
            current.push(branch.effective_name.clone());
            visit(&branch.node, current, output);
            let _ = current.pop();
        }
    }

    let mut output = Vec::new();
    let mut current = Vec::new();
    visit(root, &mut current, &mut output);
    output
}

fn extract_subcommand_path_from_args(
    args: &[std::ffi::OsString],
    root: &CommandNode,
) -> Vec<String> {
    let tokens = args
        .iter()
        .map(|arg| arg.to_string_lossy().to_string())
        .collect::<Vec<_>>();

    fn walk(node: &CommandNode, tokens: &[String], index: &mut usize, output: &mut Vec<String>) {
        let mut positionals_seen = 0usize;

        while *index < tokens.len() {
            let token = &tokens[*index];

            if token.starts_with("--") {
                let flag_name = token.trim_start_matches("--");
                if let Some(consumes_value) = node.named_flag_consumes_value.get(flag_name) {
                    if *consumes_value {
                        *index = (*index + 2).min(tokens.len());
                    } else {
                        *index += 1;
                    }
                } else {
                    *index += 1;
                    if *index < tokens.len() && !tokens[*index].starts_with('-') {
                        *index += 1;
                    }
                }
                continue;
            }

            if token.starts_with('-') {
                *index += 1;
                continue;
            }

            if positionals_seen < node.positional_count {
                positionals_seen += 1;
                *index += 1;
                continue;
            }

            if let Some(branch) = node
                .subcommands
                .iter()
                .find(|branch| branch.cli_name == normalize_command_token(token))
            {
                output.push(branch.effective_name.clone());
                *index += 1;
                walk(&branch.node, tokens, index, output);
            }
            return;
        }
    }

    let mut index = 0usize;
    let mut output = Vec::new();
    walk(root, &tokens, &mut index, &mut output);
    output
}

#[test]
fn fuzz_cli_args_consistency() {
    // Test that the same CLI instance always produces the same args
    let mut data = vec![123u8; 1024];
    let mut rng = arbitrary::Unstructured::new(&data);

    for i in 0..5000 {
        let cli = match Cli::arbitrary(&mut rng) {
            Ok(cli) => cli,
            Err(_) => {
                data = vec![i as u8 * 2; 1024];
                rng = arbitrary::Unstructured::new(&data);
                Cli::arbitrary(&mut rng).expect("Failed to generate CLI instance")
            }
        };

        let args1 = cli.to_args();
        let args2 = cli.to_args();

        assert_eq!(
            args1, args2,
            "CLI.to_args() should be deterministic for iteration {i}",
        );
    }
}

#[test]
fn fuzz_cli_args_roundtrip() {
    // Enumerate subcommand enum permutations from Facet shape and ensure roundtrip
    // coverage for each command path while fuzzing non-command arguments.
    let command_tree = node_from_shape(Cli::SHAPE);
    let command_paths = collect_command_paths(&command_tree);

    assert!(
        !command_paths.is_empty(),
        "Expected at least one command path in CLI schema"
    );

    let samples_per_path = 2usize;
    let max_attempts_per_path = 30_000usize;

    let mut data = vec![0u8; 1024];
    let mut os_rng = OsRng;

    for path in command_paths {
        let mut matched = 0usize;
        let mut attempts = 0usize;

        while matched < samples_per_path && attempts < max_attempts_per_path {
            attempts += 1;

            os_rng
                .try_fill_bytes(&mut data)
                .expect("Failed to get OS random bytes");
            let mut attempt_rng = arbitrary::Unstructured::new(&data);
            let cli = match Cli::arbitrary(&mut attempt_rng) {
                Ok(cli) => cli,
                Err(_) => continue,
            };

            let args = cli.to_args();
            let extracted_path = extract_subcommand_path_from_args(&args, &command_tree);
            if extracted_path != path {
                continue;
            }

            let parsed_cli = parse_cli_from_args(&args).unwrap_or_else(|error| {
                panic!(
                    "Failed to parse CLI args for path {:?}: {error:?}\nOriginal CLI: {cli:?}\nArgs: {args:?}",
                    path
                )
            });

            assert_eq!(
                cli, parsed_cli,
                "CLI roundtrip failed for path {:?}: original={cli:?} parsed={parsed_cli:?} args={args:?}",
                path
            );

            matched += 1;
        }

        assert!(
            matched >= samples_per_path,
            "Insufficient coverage for command path {:?}: matched {matched} samples after {attempts} attempts",
            path
        );
    }
}
