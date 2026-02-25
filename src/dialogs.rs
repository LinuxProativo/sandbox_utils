//! # UI and Dialog Utilities
//!
//! This module provides functions for terminal formatting, including boxes for commands,
//! tables for configuration diffs, and standardized error/success messages.

use serde::Serialize;
use serde_json::Value;
use std::error::Error;

/// A visual horizontal separator line used in terminal output.
pub const SEPARATOR: &str = "════════════════════════════════════════════════════════════";

/// Generates a formatted ASCII box containing a command.
///
/// # Arguments
/// * `command` - The string slice representing the command to be displayed.
/// * `indent` - Optional number of spaces to indent the entire box.
/// * `size` - Optional preferred width for the box.
///
/// # Returns
/// * `Ok(String)` - The formatted box as a string.
/// * `Err` - If formatting fails.
pub fn get_cmd_box(
    command: &str,
    indent: Option<usize>,
    size: Option<usize>,
) -> Result<String, Box<dyn Error>> {
    let padding = " ".repeat(indent.unwrap_or(0));
    let width = size.unwrap_or(50).max(command.len() + 4);
    let inner_width = width - 2;

    let line = "═".repeat(inner_width);
    let top = format!("{padding}╔{line}╗");
    let bottom = format!("{padding}╚{line}╝");

    let trailing_spaces = " ".repeat(inner_width - command.len() - 1);
    let middle = format!("{padding}║ {command}{trailing_spaces}║");

    Ok(format!("{top}\n{middle}\n{bottom}"))
}

/// Returns a formatted error message when the rootfs directory is not found.
///
/// # Arguments
/// * `run_command` - The command that the user should run to fix the issue.
/// * `path` - The expected path where the rootfs should have been located.
///
/// # Returns
/// * `Err` - A boxed error containing the complete formatted message.
pub fn failed_exist_rootfs(run_command: &str, path: &str) -> Result<(), Box<dyn Error>> {
    let cmd_box = get_cmd_box(&format!("$ {run_command}"), Some(2), None)?;

    Err(format!(
        "{s}\n  Error: rootfs directory not found.\n\n  Expected location:\n    -> {path}\n\n  Please run the following command to set it up:\n{cmd_box}\n{s}",
        s = SEPARATOR,
    ).into())
}

/// Prints a success message and instructions after a successful setup.
///
/// # Arguments
/// * `run_command` - The command the user can use to enter the new environment.
///
/// # Returns
/// * `Ok(())` - If the message was printed successfully.
pub fn success_finish_setup(run_command: &str) -> Result<(), Box<dyn Error>> {
    let cmd_box = get_cmd_box(&format!("$ {run_command}"), Some(2), None)?;

    println!(
        "{s}\n  Installation completed successfully!\n\n  To start the environment, run:\n\n{cmd_box}\n{s}",
        s = SEPARATOR,
    );
    Ok(())
}

/// Renders a visually aligned table in the terminal.
///
/// It automatically calculates column widths and compensates for ANSI color codes
/// when displaying differences.
///
/// # Arguments
/// * `rows` - A vector of tuples containing (Key, Value) pairs to be displayed.
pub fn render_table(rows: Vec<(String, String)>) {
    let key_width = rows.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
    let val_width = rows
        .iter()
        .map(|(_, v)| {
            if v.contains("->") {
                v.len().saturating_sub(22)
            } else {
                v.len()
            }
        })
        .max()
        .unwrap_or(0);

    let kw = "═".repeat(key_width);
    let vw = "═".repeat(val_width);

    println!("╔═{kw}═══╦═{vw}═══╗");

    for (k, v) in rows {
        let padding = if v.contains("->") {
            val_width + 22
        } else {
            val_width
        };
        println!("║ {:<key_width$}   ║ {:<padding$}   ║", k, v);
    }
    println!("╚═{kw}═══╩═{vw}═══╝");
}

/// Compares two serializable structures and returns a list of differences.
///
/// Changed values are formatted with ANSI colors (Red for old, Green for new).
///
/// # Arguments
/// * `old` - The base configuration structure.
/// * `new` - The updated configuration structure.
///
/// # Returns
/// A `Vec` of tuples where the first element is the field name and the second is the display value.
pub fn get_config_diff<T: Serialize>(old: &T, new: &T) -> Vec<(String, String)> {
    let old_val = serde_json::to_value(old).unwrap_or(Value::Null);
    let new_val = serde_json::to_value(new).unwrap_or(Value::Null);

    let mut rows = Vec::new();
    if let Value::Object(new_map) = new_val {
        for (key, new_v) in new_map {
            let old_v = old_val.get(&key).cloned().unwrap_or(Value::Null);

            let new_str = json_to_display_str(&new_v);
            let old_str = json_to_display_str(&old_v);

            let value_to_show = if old_v != new_v && !old_v.is_null() {
                format!("\x1b[1;31m{old_str}\x1b[0m -> \x1b[1;32m{new_str}\x1b[0m")
            } else {
                new_str
            };

            rows.push((key, value_to_show));
        }
    }

    rows
}

/// Internal helper to convert a JSON value into a user-friendly string.
///
/// # Arguments
/// * `v` - The `serde_json::Value` to be converted.
///
/// # Returns
/// A string representation of the value, with specific handling for empty strings and nulls.
fn json_to_display_str(v: &Value) -> String {
    match v {
        Value::String(s) => {
            if s.is_empty() {
                "Current Directory or Home Fallback".to_string()
            } else {
                s.clone()
            }
        }
        Value::Null => "None".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        _ => v.to_string(),
    }
}
