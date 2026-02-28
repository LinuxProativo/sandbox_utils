//! Command-line argument parsing and package matching macros.
//!
//! This module provides a set of utility macros for the ALPack CLI to handle
//! string manipulation, path construction, and argument validation with a
//! focus on memory efficiency and clear user feedback.

/// Unified macro for generating "invalid argument" errors.
///
/// It constructs a formatted error message that includes the command context,
/// the offending argument, and a helpful tip to use the `--help` flag.
#[macro_export]
macro_rules! invalid_arg {
    ($sub:expr, $other:expr) => {{
        let c = $crate::app_name();
        let context = if $sub.is_empty() {
            c.to_string()
        } else {
            format!("{c}: {}", $sub)
        };

        Err(format!(
            "{}: invalid argument '{}'\nUse '{c} --help' to see available options.",
            context, $other
        )
        .into())
    }};

    ($other:expr) => {
        $crate::invalid_arg!("", $other)
    };
}

/// Unified error reporter for missing parameters.
///
/// Supports two levels of severity:
/// 1. **Default**: General missing parameter error.
/// 2. **Essential**: Used when a core parameter required for the operation is absent.
#[macro_export]
macro_rules! missing_arg {
    ($sub:expr, essential) => {{
        let err = format!(
            "{c}: {s}: no essential parameter specified\nUse '{c} --help' to see available options.",
            c = $crate::app_name(), s = $sub
        );
        Err(err.into())
    }};

    ($sub:expr) => {{
        let err = format!(
            "{c}: {s}: no parameter specified\nUse '{c} --help' to see available options.",
            c = $crate::app_name(), s = $sub
        );
        Err(err.into())
    }};
}


/// Parses key-value pairs in both `--key=value` and `--key value` formats.
///
/// PERFORMANCE: Use `AsRef<str>` to handle both `String` and `&str` inputs
/// without forced cloning. Only allocates a new `String` when a value is
/// successfully extracted or an error message is generated.
///
/// # Returns
/// - `Ok(String)`: The extracted value.
/// - `Err(String)`: A detailed usage message if the value is missing.
#[macro_export]
macro_rules! parse_value {
    ($sub:expr, $val_name:expr, $arg:expr, $next:expr) => {{
        let arg_ref: &str = $arg.as_ref();

        let extracted: Option<String> = if let Some(pos) = arg_ref.find('=') {
            let val = &arg_ref[pos + 1..];
            if val.is_empty() {
                None
            } else {
                Some(val.to_string())
            }
        } else {
            $next.and_then(|n| {
                let n_ref: &str = n.as_ref();
                if n_ref.is_empty() || n_ref.starts_with('-') {
                    None
                } else {
                    Some(n_ref.to_string())
                }
            })
        };

        match extracted {
            Some(value) => Ok(value),
            None => {
                let cmd = std::env::current_exe()
                    .ok()
                    .and_then(|p| p.file_name()?.to_str().map(|s| s.to_string()))
                    .unwrap_or_else(|| $crate::app_name());

                let key = arg_ref.split('=').next().unwrap_or(arg_ref);
                let sp = if arg_ref.contains('=') { "=" } else { " " };

                Err(format!(
                    "{}: {}: {} requires a <{}>.\nUsage: {} {} {}{}<{}>",
                    cmd, $sub, key, $val_name, cmd, $sub, key, sp, $val_name
                ))
            }
        }
    }};

    ($sub:expr, $val_name:expr, $arg:expr) => {
        $crate::parse_value!($sub, $val_name, $arg, Option::<&str>::None)
    };
}
