/// Macro for exhaustive matching that ensures all enum variants are covered
///
/// This macro wraps a match expression and automatically extracts enum variants
/// from the match arms to check exhaustiveness at compile time using `assert_enum_variants!`.
///
/// # Example with Option<Type>
///
/// ```rust
/// use skim::exhaustive_match;
///
/// enum Status { Active, Inactive, Pending }
/// enum Output { Success, Failure, Unknown }
///
/// fn check(status: Status) -> Option<Output> {
///     exhaustive_match! {
///         status => Option<Output>;
///         {
///             Status::Active => Some(Success),
///             Status::Inactive => Some(Failure),
///             Status::Pending => Some(Unknown),
///         }
///         default _ => None
///     }
/// }
/// ```
///
/// # Example with plain Type
///
/// ```rust
/// use skim::exhaustive_match;
///
/// enum KeyCode { Enter, Char(char) }
///
/// fn parse(key: &str) -> KeyCode {
///     exhaustive_match! {
///         key => KeyCode;
///         {
///             "enter" => Enter,
///             "space" => Char(' '),
///         }
///         default s => panic!("Unknown key {}", s)
///     }
/// }
/// ```
#[macro_export]
macro_rules! exhaustive_match {
    // Version with Option<Type> in header (with Some() wrapper and default case)
    (
        $expr:expr => Option<$return_ty:ty>;
        {
            $($pattern:pat => Some($variant:ident $($rest:tt)*)),+ $(,)?
        }
        default $default_pattern:pat => $default:expr
    ) => {{
        use $return_ty::*;
        // Assert that all variants are accounted for at compile time
        ::assert_enum_variants::assert_enum_variants!($return_ty, { $($variant),+ });

        // Perform the actual match
        match $expr {
            $($pattern => Some($variant $($rest)*),)+
            $default_pattern => $default
        }
    }};

    // Version with plain Type in header (direct variant usage, with default)
    (
        $expr:expr => $return_ty:ty;
        {
            $($pattern:pat => $variant:ident $(($($args:tt)*))? ),+ $(,)?
        }
        default $default_pattern:pat => $default:expr
    ) => {{
        use $return_ty::*;
        // Assert that all variants are accounted for at compile time
        ::assert_enum_variants::assert_enum_variants!($return_ty, { $($variant),+ });

        // Perform the actual match
        match $expr {
            $($pattern => $variant $(($($args)*))? ,)+
            $default_pattern => $default
        }
    }};

    // Version with plain Type in header (direct variant usage, no default)
    (
        $expr:expr => $return_ty:ty;
        {
            $($pattern:pat => $variant:ident $(($($args:tt)*))? ),+ $(,)?
        }
    ) => {{
        use $return_ty::*;
        // Assert that all variants are accounted for at compile time
        ::assert_enum_variants::assert_enum_variants!($return_ty, { $($variant),+ });

        // Perform the actual match
        match $expr {
            $($pattern => $variant $(($($args)*))? ),+
        }
    }};
}
