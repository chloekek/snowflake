use {super::*, std::{fmt::{Display, Formatter, Result}}};

impl Display for PackageLabel
{
    fn fmt(&self, f: &mut Formatter) -> Result
    {
        if self.segments.is_empty() {
            write!(f, "/")
        } else {
            for segment in self.segments.iter() {
                write!(f, "/{}", segment.as_bytes().escape_ascii())?;
            }
            Ok(())
        }
    }
}

impl Display for RuleLabel
{
    fn fmt(&self, f: &mut Formatter) -> Result
    {
        write!(f, "{}:{}", self.package, self.rule)
    }
}

impl Display for ActionLabel
{
    fn fmt(&self, f: &mut Formatter) -> Result
    {
        write!(f, "{}#{}", self.rule, self.action)
    }
}

impl Display for RuleOutputLabel
{
    fn fmt(&self, f: &mut Formatter) -> Result
    {
        write!(f, "{}|{}", self.rule, self.output.as_bytes().escape_ascii())
    }
}

impl Display for ActionOutputLabel
{
    fn fmt(&self, f: &mut Formatter) -> Result
    {
        write!(f, "{}|{}", self.action, self.output)
    }
}