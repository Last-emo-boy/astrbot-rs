use std::sync::OnceLock;

use regex::{Captures, Regex};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DependencyConflictKind {
    CoreVersionConflict,
    PackageVersionConflict,
    ImportEnvironmentConflict,
    InstallerFailure,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DependencyConflictReport {
    pub plugin_id: String,
    pub kind: DependencyConflictKind,
    pub message: String,
    details: Vec<String>,
}

impl DependencyConflictReport {
    pub fn new(
        plugin_id: impl Into<String>,
        kind: DependencyConflictKind,
        message: impl Into<String>,
        details: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        let redactor = DependencyErrorRedactor::new();
        let details = details.into_iter().map(Into::into).collect::<Vec<String>>();
        Self {
            plugin_id: plugin_id.into(),
            kind,
            message: redactor.redact(&message.into()),
            details: redactor.redact_lines(details),
        }
    }

    pub fn from_installer_output(
        plugin_id: impl Into<String>,
        output_lines: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> Option<Self> {
        let redactor = DependencyErrorRedactor::new();
        let details = output_lines
            .into_iter()
            .map(|line| redactor.redact(line.as_ref()))
            .collect::<Vec<_>>();
        if details.is_empty() {
            return None;
        }

        let has_constraint = details.iter().any(|line| {
            let normalized = line.to_ascii_lowercase();
            normalized.contains("(constraint)") || normalized.contains("core constraint")
        });
        let has_conflict = details.iter().any(|line| is_conflict_signal(line));
        if !has_constraint && !has_conflict {
            return None;
        }

        let kind = if has_constraint {
            DependencyConflictKind::CoreVersionConflict
        } else {
            DependencyConflictKind::PackageVersionConflict
        };
        let message = match kind {
            DependencyConflictKind::CoreVersionConflict => {
                "dependency conflict blocked by core version constraints"
            }
            DependencyConflictKind::PackageVersionConflict => {
                "dependency conflict detected while resolving plugin packages"
            }
            DependencyConflictKind::ImportEnvironmentConflict
            | DependencyConflictKind::InstallerFailure => unreachable!("kind is classified above"),
        };

        Some(Self::new(
            plugin_id,
            kind,
            message,
            select_relevant_details(details),
        ))
    }

    pub fn import_environment_conflict(
        plugin_id: impl Into<String>,
        message: impl Into<String>,
        details: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self::new(
            plugin_id,
            DependencyConflictKind::ImportEnvironmentConflict,
            message,
            details,
        )
    }

    pub fn installer_failure(
        plugin_id: impl Into<String>,
        message: impl Into<String>,
        details: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self::new(
            plugin_id,
            DependencyConflictKind::InstallerFailure,
            message,
            details,
        )
    }

    pub fn details(&self) -> &[String] {
        &self.details
    }

    pub fn is_core_conflict(&self) -> bool {
        self.kind == DependencyConflictKind::CoreVersionConflict
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DependencyErrorRedactor;

impl DependencyErrorRedactor {
    pub fn new() -> Self {
        Self
    }

    pub fn redact(&self, value: &str) -> String {
        let without_url_credentials = url_credentials_re()
            .replace_all(value, |captures: &Captures<'_>| {
                format!("{}<redacted>@{}", &captures[1], &captures[2])
            })
            .to_string();

        sensitive_assignment_re()
            .replace_all(&without_url_credentials, |captures: &Captures<'_>| {
                format!("{}=****", &captures[1])
            })
            .to_string()
    }

    pub fn redact_lines(&self, lines: impl IntoIterator<Item = impl AsRef<str>>) -> Vec<String> {
        lines
            .into_iter()
            .map(|line| self.redact(line.as_ref()))
            .collect()
    }

    pub fn redact_args(&self, args: &[String]) -> Vec<String> {
        let mut redacted = Vec::with_capacity(args.len());
        let mut redact_next = false;

        for arg in args {
            if redact_next {
                redacted.push("****".to_string());
                redact_next = false;
                continue;
            }

            if is_sensitive_value_key(arg) {
                redacted.push(arg.clone());
                redact_next = true;
                continue;
            }

            redacted.push(self.redact(arg));
        }

        redacted
    }
}

fn is_conflict_signal(line: &str) -> bool {
    let normalized = line.to_ascii_lowercase();
    normalized.contains("resolutionimpossible")
        || normalized.contains("cannot install")
        || normalized.contains("conflict")
        || normalized.contains("depends on")
}

fn select_relevant_details(details: Vec<String>) -> Vec<String> {
    let relevant = details
        .iter()
        .filter(|line| {
            let normalized = line.to_ascii_lowercase();
            is_conflict_signal(line)
                || normalized.contains("(constraint)")
                || normalized.contains("the user requested")
                || normalized.contains("<redacted>")
                || normalized.contains("****")
        })
        .cloned()
        .collect::<Vec<_>>();

    if relevant.is_empty() {
        let mut tail = details.into_iter().rev().take(5).collect::<Vec<_>>();
        tail.reverse();
        tail
    } else {
        relevant
    }
}

fn is_sensitive_value_key(raw_key: &str) -> bool {
    let normalized = raw_key
        .trim_start_matches('-')
        .replace('-', "_")
        .to_ascii_lowercase();
    matches!(
        normalized.as_str(),
        "password" | "passwd" | "pass" | "api_token" | "token" | "auth_token"
    )
}

fn url_credentials_re() -> &'static Regex {
    static URL_CREDENTIALS_RE: OnceLock<Regex> = OnceLock::new();
    URL_CREDENTIALS_RE.get_or_init(|| {
        Regex::new(r"([A-Za-z][A-Za-z0-9+.-]*://)[^\s/@]+@([^\s/]+)")
            .expect("url credential redaction regex")
    })
}

fn sensitive_assignment_re() -> &'static Regex {
    static SENSITIVE_ASSIGNMENT_RE: OnceLock<Regex> = OnceLock::new();
    SENSITIVE_ASSIGNMENT_RE.get_or_init(|| {
        Regex::new(
            r"(?i)((?:--?)?(?:password|passwd|pass|api[-_]?token|auth[-_]?token|token))=([^\s&]+)",
        )
        .expect("sensitive assignment redaction regex")
    })
}
