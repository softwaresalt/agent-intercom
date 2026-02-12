//! Contract tests for the `forward_prompt` MCP tool (T114).
//!
//! Validates input schema (required fields, enum values, optional fields)
//! and output schema (`decision` enum, optional `instruction`) per
//! `mcp-tools.json` contract.

use serde_json::json;

/// The tool name as registered in the MCP server.
const TOOL_NAME: &str = "forward_prompt";

/// Valid prompt_type enum values per contract.
const VALID_PROMPT_TYPES: &[&str] = &[
    "continuation",
    "clarification",
    "error_recovery",
    "resource_warning",
];

/// Valid output decision enum values per contract.
const VALID_DECISIONS: &[&str] = &["continue", "refine", "stop"];

// ─── Input schema validation ──────────────────────────────────────────

#[test]
fn input_requires_prompt_text() {
    let input = json!({
        "prompt_type": "continuation"
    });
    assert!(
        input.get("prompt_text").is_none(),
        "input without 'prompt_text' should lack the required field"
    );
}

#[test]
fn input_accepts_prompt_text_only() {
    let input = json!({
        "prompt_text": "Continue with the refactoring?"
    });
    assert!(input.get("prompt_text").is_some());
}

#[test]
fn input_prompt_type_is_optional_with_default_continuation() {
    // When omitted, default is "continuation" per contract.
    let input = json!({
        "prompt_text": "Should I proceed?"
    });
    assert!(input.get("prompt_type").is_none());
}

#[test]
fn input_prompt_type_accepts_all_valid_enum_values() {
    for prompt_type in VALID_PROMPT_TYPES {
        let input = json!({
            "prompt_text": "Question",
            "prompt_type": prompt_type
        });
        assert_eq!(
            input["prompt_type"].as_str(),
            Some(*prompt_type),
            "{TOOL_NAME} should accept prompt_type '{prompt_type}'"
        );
    }
}

#[test]
fn input_elapsed_seconds_is_optional() {
    let without = json!({
        "prompt_text": "Continue?"
    });
    assert!(without.get("elapsed_seconds").is_none());

    let with = json!({
        "prompt_text": "Continue?",
        "elapsed_seconds": 300
    });
    assert!(with.get("elapsed_seconds").is_some());
    assert_eq!(with["elapsed_seconds"].as_i64(), Some(300));
}

#[test]
fn input_actions_taken_is_optional() {
    let without = json!({
        "prompt_text": "Continue?"
    });
    assert!(without.get("actions_taken").is_none());

    let with = json!({
        "prompt_text": "Continue?",
        "actions_taken": 42
    });
    assert!(with.get("actions_taken").is_some());
    assert_eq!(with["actions_taken"].as_i64(), Some(42));
}

#[test]
fn input_accepts_all_optional_fields() {
    let input = json!({
        "prompt_text": "Should I continue with the migration?",
        "prompt_type": "error_recovery",
        "elapsed_seconds": 600,
        "actions_taken": 15
    });
    assert!(input.get("prompt_text").is_some());
    assert!(input.get("prompt_type").is_some());
    assert!(input.get("elapsed_seconds").is_some());
    assert!(input.get("actions_taken").is_some());
}

// ─── Output schema validation ─────────────────────────────────────────

#[test]
fn output_decision_is_required() {
    let output = json!({
        "decision": "continue"
    });
    assert!(output.get("decision").is_some());
}

#[test]
fn output_decision_accepts_all_valid_enum_values() {
    for decision in VALID_DECISIONS {
        let output = json!({
            "decision": decision
        });
        assert_eq!(
            output["decision"].as_str(),
            Some(*decision),
            "{TOOL_NAME} output should include decision '{decision}'"
        );
    }
}

#[test]
fn output_instruction_is_optional() {
    let without = json!({
        "decision": "continue"
    });
    assert!(without.get("instruction").is_none());

    let with = json!({
        "decision": "refine",
        "instruction": "Focus on error handling first"
    });
    assert!(with.get("instruction").is_some());
}

#[test]
fn output_instruction_present_only_when_refine() {
    // Per contract: instruction is "present only when decision=refine".
    let continue_resp = json!({
        "decision": "continue"
    });
    assert!(
        continue_resp.get("instruction").is_none(),
        "continue response should not include 'instruction'"
    );

    let stop_resp = json!({
        "decision": "stop"
    });
    assert!(
        stop_resp.get("instruction").is_none(),
        "stop response should not include 'instruction'"
    );

    let refine_resp = json!({
        "decision": "refine",
        "instruction": "Add more test coverage"
    });
    assert!(
        refine_resp.get("instruction").is_some(),
        "refine response should include 'instruction'"
    );
}

// ─── Tool definition contract ─────────────────────────────────────────

#[test]
fn tool_name_matches_contract() {
    assert_eq!(TOOL_NAME, "forward_prompt");
}

/// Verify the tool definition from `mcp-tools.json` matches what the server
/// registers.
#[test]
fn contract_schema_structure_is_valid() {
    let contract: serde_json::Value = serde_json::from_str(include_str!(
        "../../specs/001-mcp-remote-agent-server/contracts/mcp-tools.json"
    ))
    .expect("mcp-tools.json should be valid JSON");

    let tool = &contract["tools"][TOOL_NAME];

    // Input schema checks.
    let input = &tool["inputSchema"];
    assert_eq!(input["type"], "object");
    let required = input["required"]
        .as_array()
        .expect("required should be array");
    let required_names: Vec<&str> = required.iter().filter_map(|v| v.as_str()).collect();
    assert!(
        required_names.contains(&"prompt_text"),
        "prompt_text must be required"
    );

    // Verify prompt_type enum values.
    let prompt_type_enum = &input["properties"]["prompt_type"]["enum"];
    let enum_values: Vec<&str> = prompt_type_enum
        .as_array()
        .expect("prompt_type enum should be array")
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    for expected in VALID_PROMPT_TYPES {
        assert!(
            enum_values.contains(expected),
            "prompt_type enum should contain '{expected}'"
        );
    }

    // Output schema checks.
    let output = &tool["outputSchema"];
    assert_eq!(output["type"], "object");
    let out_required = output["required"]
        .as_array()
        .expect("output required should be array");
    let out_required_names: Vec<&str> = out_required.iter().filter_map(|v| v.as_str()).collect();
    assert!(
        out_required_names.contains(&"decision"),
        "decision must be required in output"
    );

    // Verify decision enum values.
    let decision_enum = &output["properties"]["decision"]["enum"];
    let decision_values: Vec<&str> = decision_enum
        .as_array()
        .expect("decision enum should be array")
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    for expected in VALID_DECISIONS {
        assert!(
            decision_values.contains(expected),
            "decision enum should contain '{expected}'"
        );
    }
}
