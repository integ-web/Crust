use std::fmt::Debug;

/// Data that has come from an untrusted source (e.g., web scraping, user input).
/// It cannot be directly used in a TrustedAction.
#[derive(Debug, Clone, PartialEq)]
pub struct UntrustedValue<T> {
    data: T,
    source: String,
}

impl<T> UntrustedValue<T> {
    pub fn new(data: T, source: String) -> Self {
        Self { data, source }
    }

    /// Access the underlying data read-only. This doesn't sanitize it,
    /// but allows inspecting the tainted data.
    pub fn peek(&self) -> &T {
        &self.data
    }
}

/// Data that has passed through a PrincipalChecker and is deemed safe.
#[derive(Debug, Clone, PartialEq)]
pub struct TrustedValue<T> {
    data: T,
}

impl<T> TrustedValue<T> {
    pub fn into_inner(self) -> T {
        self.data
    }
}

/// The Gatekeeper: The Principal Checker.
/// Evaluates untrusted data. If it passes the policy, it upgrades to a TrustedValue.
pub struct PrincipalChecker;

impl PrincipalChecker {
    /// Applies a policy function to the untrusted value.
    /// If the policy returns `true`, the value is considered sanitized.
    pub fn sanitize<T, F>(untrusted: UntrustedValue<T>, policy: F) -> Result<TrustedValue<T>, String>
    where
        F: Fn(&T) -> bool,
    {
        if policy(&untrusted.data) {
            Ok(TrustedValue { data: untrusted.data })
        } else {
            Err(format!(
                "Taint Analysis Failed: Data from source '{}' violated security policy.",
                untrusted.source
            ))
        }
    }
}

/// A trait for actions that strictly require trusted input.
pub trait TrustedAction {
    type Input;
    type Output;

    /// The execution method specifically demands a TrustedValue, making it impossible
    /// (at compile time) to accidentally pass an UntrustedValue directly.
    fn execute(&self, input: TrustedValue<Self::Input>) -> Self::Output;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FileSystemWriteAction;

    impl TrustedAction for FileSystemWriteAction {
        type Input = String;
        type Output = bool;

        fn execute(&self, input: TrustedValue<Self::Input>) -> Self::Output {
            let data = input.into_inner();
            // Actually perform the write here...
            println!("Writing to file system safely: {}", data);
            true
        }
    }

    #[test]
    fn test_taint_flow_pass() {
        let untrusted_input = UntrustedValue::new("safe_command".to_string(), "web_scraper".to_string());

        // Policy: Must not contain "rm -rf"
        let policy = |data: &String| !data.contains("rm -rf");

        let trusted_input = PrincipalChecker::sanitize(untrusted_input, policy).unwrap();

        let action = FileSystemWriteAction;
        let result = action.execute(trusted_input);

        assert!(result);
    }

    #[test]
    fn test_taint_flow_fail() {
        let untrusted_input = UntrustedValue::new("rm -rf /".to_string(), "malicious_user".to_string());

        // Policy: Must not contain "rm -rf"
        let policy = |data: &String| !data.contains("rm -rf");

        let result = PrincipalChecker::sanitize(untrusted_input, policy);

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "Taint Analysis Failed: Data from source 'malicious_user' violated security policy."
        );
    }
}
