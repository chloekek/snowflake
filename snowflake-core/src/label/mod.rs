//! Identifying elements of a build.

use {snowflake_util::basename::Basename, std::{ffi::OsStr, sync::Arc}};

mod display;

/// Identifies a package.
#[allow(missing_docs)]
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct PackageLabel
{
    pub segments: Arc<[Basename<Arc<OsStr>>]>,
}

/// Identifies a rule.
#[allow(missing_docs)]
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct RuleLabel
{
    pub package: PackageLabel,
    pub rule: Arc<str>,
}

/// Identifies an action.
#[allow(missing_docs)]
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ActionLabel
{
    pub rule: RuleLabel,
    pub action: u32,
}

/// Identifies an output of a rule.
#[allow(missing_docs)]
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct RuleOutputLabel
{
    pub rule: RuleLabel,
    pub output: Basename<Arc<OsStr>>,
}

/// Identifies an output of an action.
#[allow(missing_docs)]
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ActionOutputLabel
{
    pub action: ActionLabel,
    pub output: u32,
}
