//! What the loader reports when a data tree is wrong.
//!
//! The standard these messages are held to (T1.2.10): every one names **the
//! file**, **where in it**, and **what to do**. A message that only says
//! something is invalid has failed.

use std::fmt;

/// Whether a problem stops the data being used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Reported, but the data is still usable.
    Warning,
    /// The data cannot be used.
    Error,
}

/// A position in a file. Lines and columns start at 1, as editors show them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize)]
pub struct Location {
    pub line: u32,
    pub col: u32,
}

/// What kind of problem this is.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DataErrorKind {
    /// The file could not be read at all.
    Io,
    /// The file is not well-formed RON.
    Syntax { at: Location },
    /// The file parses but does not match its schema.
    ///
    /// RON 0.12 gives a position for these as well as a field path, so both
    /// are recorded: the position takes an author straight there, and the path
    /// says which field it is when a file has many alike (T1.2.1 §1).
    Schema {
        at: Location,
        field_path: Option<String>,
    },
    /// The file is in a place the layout does not allow, or its name does not
    /// match its contents.
    Layout,
    /// Something it names does not exist.
    Reference,
    /// It contradicts itself, or another file.
    Consistency,
}

impl DataErrorKind {
    /// The position in the file, where there is one.
    pub fn location(&self) -> Option<Location> {
        match self {
            DataErrorKind::Syntax { at } | DataErrorKind::Schema { at, .. } => Some(*at),
            _ => None,
        }
    }

    /// The field path, where there is one.
    pub fn field_path(&self) -> Option<&str> {
        match self {
            DataErrorKind::Schema { field_path, .. } => field_path.as_deref(),
            _ => None,
        }
    }
}

/// One problem with one file.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct DataError {
    /// The file, as a forward-slash path relative to the data root.
    pub file: String,
    #[serde(flatten)]
    pub kind: DataErrorKind,
    pub severity: Severity,
    /// What is wrong and what to do about it.
    pub message: String,
}

impl DataError {
    /// A problem with no position in the file.
    pub fn new(file: impl Into<String>, kind: DataErrorKind, message: impl Into<String>) -> Self {
        DataError {
            file: file.into(),
            kind,
            severity: Severity::Error,
            message: message.into(),
        }
    }

    /// A reference that does not resolve.
    pub fn reference(file: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(file, DataErrorKind::Reference, message)
    }

    /// A contradiction.
    pub fn consistency(file: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(file, DataErrorKind::Consistency, message)
    }

    /// A file in the wrong place, or with the wrong name.
    pub fn layout(file: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(file, DataErrorKind::Layout, message)
    }

    /// Turns this into a warning.
    pub fn as_warning(mut self) -> Self {
        self.severity = Severity::Warning;
        self
    }

    /// Whether this stops the data being used.
    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }
}

impl fmt::Display for DataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.kind.location(), self.kind.field_path()) {
            (Some(at), Some(path)) => {
                write!(
                    f,
                    "{}:{}:{}: {}: {}",
                    self.file, at.line, at.col, path, self.message
                )
            }
            (Some(at), None) => write!(f, "{}:{}:{}: {}", self.file, at.line, at.col, self.message),
            (None, Some(path)) => write!(f, "{}: {}: {}", self.file, path, self.message),
            (None, None) => write!(f, "{}: {}", self.file, self.message),
        }
    }
}

/// Everything wrong with a data tree.
///
/// Holds warnings as well as errors, because a tree can be usable and still
/// worth complaining about.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DataErrors(pub Vec<DataError>);

impl DataErrors {
    /// Whether anything at all was reported.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The problems that stop the data being used.
    pub fn errors(&self) -> impl Iterator<Item = &DataError> {
        self.0.iter().filter(|problem| problem.is_error())
    }

    /// The problems that do not.
    pub fn warnings(&self) -> impl Iterator<Item = &DataError> {
        self.0.iter().filter(|problem| !problem.is_error())
    }

    /// How many stop the data being used.
    pub fn error_count(&self) -> usize {
        self.errors().count()
    }

    /// How many do not.
    pub fn warning_count(&self) -> usize {
        self.warnings().count()
    }

    /// How many files have at least one problem.
    pub fn file_count(&self) -> usize {
        let mut files: Vec<&str> = self.0.iter().map(|problem| problem.file.as_str()).collect();
        files.sort_unstable();
        files.dedup();
        files.len()
    }

    /// Whether the data can be used.
    pub fn is_fatal(&self) -> bool {
        self.error_count() > 0
    }

    /// Sorts by file, then by position, so output is stable between runs.
    pub fn sort(&mut self) {
        self.0.sort_by(|left, right| {
            let position = |problem: &DataError| {
                problem
                    .kind
                    .location()
                    .map(|at| (at.line, at.col))
                    .unwrap_or((0, 0))
            };
            left.file
                .cmp(&right.file)
                .then_with(|| position(left).cmp(&position(right)))
                .then_with(|| left.message.cmp(&right.message))
        });
    }

    /// The one-line summary that closes a report.
    pub fn summary(&self) -> String {
        let errors = self.error_count();
        let warnings = self.warning_count();
        let files = self.file_count();
        format!(
            "{errors} error{}, {warnings} warning{} in {files} file{}",
            plural(errors),
            plural(warnings),
            plural(files)
        )
    }
}

fn plural(count: usize) -> &'static str {
    if count == 1 { "" } else { "s" }
}

impl fmt::Display for DataErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut last_file: Option<&str> = None;
        for problem in &self.0 {
            if last_file != Some(problem.file.as_str()) {
                if last_file.is_some() {
                    writeln!(f)?;
                }
                writeln!(f, "{}:", problem.file)?;
                last_file = Some(&problem.file);
            }
            let marker = if problem.is_error() {
                "error"
            } else {
                "warning"
            };
            match (problem.kind.location(), problem.kind.field_path()) {
                (Some(at), Some(path)) => writeln!(
                    f,
                    "  {}:{} {marker}: {}: {}",
                    at.line, at.col, path, problem.message
                )?,
                (Some(at), None) => {
                    writeln!(f, "  {}:{} {marker}: {}", at.line, at.col, problem.message)?
                }
                (None, Some(path)) => writeln!(f, "  {marker}: {}: {}", path, problem.message)?,
                (None, None) => writeln!(f, "  {marker}: {}", problem.message)?,
            }
        }
        if !self.0.is_empty() {
            writeln!(f)?;
        }
        write!(f, "{}", self.summary())
    }
}

impl std::error::Error for DataErrors {}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(line: u32, col: u32) -> Location {
        Location { line, col }
    }

    #[test]
    fn a_syntax_error_reads_like_a_compiler_message() {
        let problem = DataError::new(
            "skills/mesmer/panic.ron",
            DataErrorKind::Syntax { at: at(4, 17) },
            "expected a closing parenthesis",
        );
        assert_eq!(
            problem.to_string(),
            "skills/mesmer/panic.ron:4:17: expected a closing parenthesis"
        );
    }

    #[test]
    fn a_schema_error_names_the_field_as_well_as_the_place() {
        let problem = DataError::new(
            "skills/mesmer/panic.ron",
            DataErrorKind::Schema {
                at: at(9, 5),
                field_path: Some("extracted.scaled[1].r12".to_owned()),
            },
            "expected an integer",
        );
        assert_eq!(
            problem.to_string(),
            "skills/mesmer/panic.ron:9:5: extracted.scaled[1].r12: expected an integer"
        );
    }

    #[test]
    fn a_reference_error_has_no_position_but_still_names_the_file() {
        let problem = DataError::reference(
            "creatures/foes/kournan/kournan-seer.ron",
            "skill `power-spike` has no file",
        );
        assert_eq!(
            problem.to_string(),
            "creatures/foes/kournan/kournan-seer.ron: skill `power-spike` has no file"
        );
    }

    #[test]
    fn warnings_do_not_make_a_tree_unusable() {
        let mut problems = DataErrors(vec![
            DataError::consistency("a.ron", "something odd").as_warning(),
        ]);
        assert!(!problems.is_fatal());
        assert_eq!(problems.error_count(), 0);
        assert_eq!(problems.warning_count(), 1);

        problems
            .0
            .push(DataError::consistency("a.ron", "something wrong"));
        assert!(problems.is_fatal());
    }

    #[test]
    fn the_summary_counts_files_not_problems() {
        let problems = DataErrors(vec![
            DataError::consistency("a.ron", "one"),
            DataError::consistency("a.ron", "two"),
            DataError::consistency("b.ron", "three").as_warning(),
        ]);
        assert_eq!(problems.summary(), "2 errors, 1 warning in 2 files");
    }

    #[test]
    fn the_summary_uses_singular_forms() {
        let problems = DataErrors(vec![DataError::consistency("a.ron", "one")]);
        assert_eq!(problems.summary(), "1 error, 0 warnings in 1 file");
    }

    #[test]
    fn sorting_is_stable_by_file_then_position() {
        let mut problems = DataErrors(vec![
            DataError::new("b.ron", DataErrorKind::Syntax { at: at(1, 1) }, "b1"),
            DataError::new("a.ron", DataErrorKind::Syntax { at: at(9, 1) }, "a9"),
            DataError::new("a.ron", DataErrorKind::Syntax { at: at(2, 1) }, "a2"),
        ]);
        problems.sort();
        let order: Vec<&str> = problems.0.iter().map(|p| p.message.as_str()).collect();
        assert_eq!(order, ["a2", "a9", "b1"]);
    }

    #[test]
    fn the_report_groups_by_file() {
        let mut problems = DataErrors(vec![
            DataError::new("a.ron", DataErrorKind::Syntax { at: at(2, 3) }, "first"),
            DataError::consistency("a.ron", "second"),
            DataError::reference("b.ron", "third").as_warning(),
        ]);
        problems.sort();
        let report = problems.to_string();

        assert!(report.contains("a.ron:\n"), "{report}");
        assert!(report.contains("  2:3 error: first"), "{report}");
        assert!(report.contains("  warning: third"), "{report}");
        assert!(
            report.ends_with("2 errors, 1 warning in 2 files"),
            "{report}"
        );
    }
}
