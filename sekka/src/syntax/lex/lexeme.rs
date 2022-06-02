use {super::super::location::Location, std::{fmt, sync::Arc}};

/// Token along with its location.
#[allow(missing_docs)]
#[derive(Debug)]
pub struct Lexeme
{
    pub location: Location,
    pub token: Token,
}

/// Structured information about a lexeme.
#[derive(Debug, Eq, PartialEq)]
pub enum Token
{
    /** `(` */ LeftParenthesis,
    /** `)` */ RightParenthesis,
    /** `+` */ PlusSign,
    /** `~` */ Tilde,

    /** `fun`  */ FunKeyword,

    /// Identifier.
    Identifier(Arc<str>),

    /// String literal.
    ///
    /// The contained string is the actual string value;
    /// any escape sequences have already been resolved.
    StringLiteral(Arc<[u8]>),
}

impl fmt::Display for Token
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result
    {
        match self {
            Self::LeftParenthesis   => write!(f, "`(`"),
            Self::RightParenthesis  => write!(f, "`)`"),
            Self::PlusSign          => write!(f, "`+`"),
            Self::Tilde             => write!(f, "`~`"),
            Self::FunKeyword        => write!(f, "`fun`"),
            Self::Identifier(..)    => write!(f, "identifier"),
            Self::StringLiteral(..) => write!(f, "string literal"),
        }
    }
}
