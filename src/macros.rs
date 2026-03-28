//! Color output macros

/// Green text
///
/// # Examples
/// ```
/// use crate::macros::green;
///
/// println!("{}", green!("This is green"));
/// ```
macro_rules! green {
    ($($arg:tt)*) => {
        format!("\x1b[32m{}\x1b[0m", format!($($arg)*))
    };
}

/// Red text
///
/// # Examples
/// ```
/// use crate::macros::red;
///
/// println!("{}", red!("This is red"));
/// ```
macro_rules! red {
    ($($arg:tt)*) => {
        format!("\x1b[31m{}\x1b[0m", format!($($arg)*))
    };
}

/// Yellow text
///
/// # Examples
/// ```
/// use crate::macros::yellow;
///
/// println!("{}", yellow!("This is yellow"));
/// ```
macro_rules! yellow {
    ($($arg:tt)*) => {
        format!("\x1b[33m{}\x1b[0m", format!($($arg)*))
    };
}

pub(crate) use {green, red, yellow};
