use crate::error::{PostProcessError, PreProcessError};
use async_nats::Subject;
use async_nats::jetstream::Context;
use async_nats::subject::ToSubject;
use compact_str::CompactString;
use std::fmt::Display;
use std::future::Future;
use std::marker::PhantomData;
use std::ops::Deref;
use kanau::message::{MessageDe, MessageSer, SerializeError};

/// Message in NATS JetStream that can be published.
pub trait JetStreamMessageSendTrait: MessageSer + DynamicSubjectMessage + Send + Sync + Sized {
    #[doc(hidden)]
    /// Publish the message to the NATS server.
    ///
    /// **DO NOT OVERRIDE THIS FUNCTION.**
    fn publish(
        self,
        js_context: &Context,
    ) -> impl Future<Output = Result<(), PostProcessError>> + Send {
        async {
            js_context
                .publish(
                    self.subject(),
                    self.to_bytes()
                        .map_err(Into::<SerializeError>::into)?
                        .to_vec()
                        .into(),
                )
                .await?;
            Ok(())
        }
    }
}

impl<T: MessageSer + DynamicSubjectMessage + Send + Sync + Sized> JetStreamMessageSendTrait for T {}

// ---------------------------------------------

/// Message in NATS RPC services.
pub trait NatsRpcCallTrait<Response: MessageDe>: MessageSer + Send + Sync + Sized {
    /// The subject of the message. Must be static.
    ///
    /// A message can be used as multiple RPC calls' request. They can have multiple subjects.
    ///
    /// We bind the response type here as phantom data to ensure the subject is matched.
    fn subject() -> (NatsSubjectPath, PhantomData<Response>);

    #[doc(hidden)]
    /// Call the RPC service and get the response.
    ///
    /// **DO NOT OVERRIDE THIS FUNCTION.**
    fn call(
        self,
        nats_connection: &async_nats::Client,
    ) -> impl Future<Output = Result<Response, crate::Error>> + Send {
        async move {
            let (subject, _): (_, PhantomData<Response>) = Self::subject();
            let bytes = self
                .to_bytes()
                .map_err(|err| PostProcessError::SerializeError(err.into()))
                .map_err(crate::Error::PostProcessError)?;
            let response = nats_connection
                .request(subject, bytes.to_vec().into())
                .await
                .map_err(crate::Error::RpcCallRequestError)?;
            let data = Response::from_bytes(response.payload.as_ref())
                .map_err(|err| PreProcessError::DeserializeError(err.into()))
                .map_err(crate::Error::PreProcessError)?;
            Ok(data)
        }
    }
}

// ---------------------------------------------

/// NATS subject path.
pub struct NatsSubjectPath(pub Box<[CompactString]>);

#[macro_export]
/// Creates a `NatsSubjectPath` from 
/// 
/// - a sequence of string literals
/// - a sequence of expressions that implement `AsRef<str>`
macro_rules! subject_path {
    [$( $segment:literal ),* $(,)?] => {
        NatsSubjectPath(Box::new([
            $(compact_str::CompactString::const_new($segment),)*
        ]))
    };
    [$( $segment:expr ),* $(,)?] => {
        NatsSubjectPath(Box::new([
            $(compact_str::CompactString::new($segment),)*
        ]))
    };
}

impl ToSubject for NatsSubjectPath {
    fn to_subject(&self) -> Subject {
        let joined = self.0.join(".");
        Subject::from(joined)
    }
}

impl Display for NatsSubjectPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.join("."))
    }
}

impl From<NatsSubjectPath> for String {
    fn from(val: NatsSubjectPath) -> Self {
        val.0.join(".")
    }
}

impl Deref for NatsSubjectPath {
    type Target = [CompactString];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<Vec<CompactString>> for NatsSubjectPath {
    fn from(value: Vec<CompactString>) -> Self {
        Self(value.into_boxed_slice())
    }
}

impl From<Vec<&str>> for NatsSubjectPath {
    fn from(value: Vec<&str>) -> Self {
        Self(
            value
                .into_iter()
                .map(|s| CompactString::new(s))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        )
    }
}

impl From<Vec<String>> for NatsSubjectPath {
    fn from(value: Vec<String>) -> Self {
        Self(
            value
                .into_iter()
                .map(|s| CompactString::new(s))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        )
    }
}

/// NATS Message that has a subject.
///
/// Can be dynamic or static. Can be NATS core message or JetStream message.
pub trait DynamicSubjectMessage {
    /// The subject of the message. Can be dynamic.
    fn subject(&self) -> NatsSubjectPath;
}

/// NATS Message that has a subject.
///
/// Must be static. Can be NATS core message or JetStream message.
pub trait StaticSubjectMessage {
    /// The subject of the message. Must be static.
    fn subject() -> NatsSubjectPath;
}

impl<T> DynamicSubjectMessage for T
where
    T: StaticSubjectMessage,
{
    fn subject(&self) -> NatsSubjectPath {
        T::subject()
    }
}

// ---------------------------------------------

/// The field of a subject matcher.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubjectMatcherField {
    /// Static field.
    ///
    /// for example `foo.bar` has two static fields `foo` and `bar`.
    Static(CompactString),

    /// Wildcard field.
    ///
    /// for example `foo.*` has one static field `foo` and one wildcard field `*`.
    Wildcard,

    /// Recursive wildcard field.
    ///
    /// for example `foo.>` has one static field `foo` and one recursive wildcard field `>`.
    RecursiveWildcard,
}

impl PartialEq<CompactString> for SubjectMatcherField {
    fn eq(&self, other: &CompactString) -> bool {
        match self {
            SubjectMatcherField::Static(s) => s == other,
            _ => true,
        }
    }
}

/// The subject matcher.
///
/// Can check if a subject matches the matcher.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubjectMatcher(pub Box<[SubjectMatcherField]>);

impl Deref for SubjectMatcher {
    type Target = [SubjectMatcherField];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<Vec<SubjectMatcherField>> for SubjectMatcher {
    fn from(value: Vec<SubjectMatcherField>) -> Self {
        Self(value.into_boxed_slice())
    }
}

impl SubjectMatcher {
    /// Checks if a NATS subject matches this matcher pattern
    ///
    /// For example:
    /// - "foo.bar.baz" matches "foo.bar.baz"
    /// - "foo.bar.baz" matches "foo.*.baz"
    /// - "foo.bar.baz" matches "foo.>"
    /// - "foo.bar.baz" does not match "foo.bar"
    /// - "foo.bar.baz" does not match "foo.*"
    /// - "foo.bar2.baz" matches "foo.*.baz"
    ///
    /// ```rust
    /// # use shizuku::subject_matcher;
    /// # use shizuku::subject_path;
    /// # use shizuku::core::message::NatsSubjectPath;
    ///
    /// let path = subject_path!["foo", "bar", "baz"];
    ///
    /// let matcher = subject_matcher!["foo", "bar", "baz"];
    /// assert!(matcher.matches(&path));
    ///
    /// let matcher = subject_matcher!["foo", "*", "baz"];
    /// assert!(matcher.matches(&path));
    ///
    /// let matcher = subject_matcher!["foo", "bar"];
    /// assert!(!matcher.matches(&path));
    ///
    /// let matcher = subject_matcher!["foo", "*"];
    /// assert!(!matcher.matches(&path));
    ///
    /// let matcher = subject_matcher!["foo", ">"];
    /// assert!(matcher.matches(&path));
    ///
    /// let path = subject_path!["foo", "bar2", "baz"];
    /// let matcher = subject_matcher!["foo", "*", "baz"];
    /// assert!(matcher.matches(&path));
    /// ```
    pub fn matches(&self, subject: &NatsSubjectPath) -> bool {
        let subject = &**subject;
        let matcher = &**self;

        // Early return if subject has fewer segments than matcher
        // e.g. "foo" cannot match "foo.bar"
        if subject.len() < matcher.len() {
            return false;
        }

        // Early return if subject has more segments than matcher,
        // unless matcher ends with '>' (recursive wildcard)
        // e.g. "foo.bar.baz" cannot match "foo.bar", but can match "foo.>"
        if subject.len() > matcher.len()
            && matcher.last() != Some(&SubjectMatcherField::RecursiveWildcard)
        {
            return false;
        }

        let len = matcher.len();
        for i in 0..len {
            match matcher[i] {
                // For static segments, check exact string match
                // e.g. "foo" must match "foo"
                SubjectMatcherField::Static(ref s) => {
                    if subject[i] != *s {
                        return false;
                    }
                }
                // For '*' wildcard, any single segment matches
                // e.g. "foo.*" matches "foo.bar" or "foo.baz"
                SubjectMatcherField::Wildcard => {}
                // For '>' recursive wildcard, matches all remaining segments
                // Must be the last segment in matcher pattern
                SubjectMatcherField::RecursiveWildcard => {
                    if i != len - 1 {
                        unreachable!("Recursive wildcard must be the last field");
                    }
                }
            }
        }
        true
    }
}

/// Creates a `SubjectMatcher` from a sequence of tokens.
///
/// Syntax:
/// - Static segments: string literals or identifiers
/// - "*": single-level wildcard
/// - ">": multi-level wildcard (must be last)
///
/// Examples:
/// ```
/// # use shizuku::subject_matcher;
/// # use shizuku::core::message::SubjectMatcherField;
///
/// let matcher = subject_matcher!["foo", "*", "bar"];  // matches "foo.{any}.bar"
/// assert_eq!(
///     matcher.0.as_ref(),
///     &[
///         SubjectMatcherField::Static("foo".into()),
///         SubjectMatcherField::Wildcard,
///         SubjectMatcherField::Static("bar".into()),
///     ]
/// );
///
/// let matcher = subject_matcher!["foo", "bar", ">"];  // matches "foo.bar.{anything...}"
/// assert_eq!(
///     matcher.0.as_ref(),
///     &[
///         SubjectMatcherField::Static("foo".into()),
///         SubjectMatcherField::Static("bar".into()),
///         SubjectMatcherField::RecursiveWildcard,
///     ]
/// );
///
/// ```
#[macro_export]
macro_rules! subject_matcher {
    // Base case: empty array
    [] => {
        $crate::core::message::SubjectMatcher(Box::new([]))
    };

    // Match array of tokens
    [$($segment:expr),* $(,)?] => {{
        use $crate::core::message::SubjectMatcherField;
        use compact_str::CompactString;

        let segments = vec![
            $(
                match $segment {
                    "*" => SubjectMatcherField::Wildcard,
                    ">" => SubjectMatcherField::RecursiveWildcard,
                    s => SubjectMatcherField::Static(CompactString::new(s)),
                }
            ),*
        ];

        // Validate that '>' only appears as the last segment
        let recursive_count = segments.iter()
            .filter(|s| matches!(s, SubjectMatcherField::RecursiveWildcard))
            .count();

        if recursive_count > 1 {
            panic!("Multiple '>' wildcards are not allowed in subject matcher");
        }

        if recursive_count == 1 && !matches!(segments.last(), Some(SubjectMatcherField::RecursiveWildcard)) {
            panic!("'>' wildcard must be the last segment in subject matcher");
        }

        $crate::core::message::SubjectMatcher(segments.into_boxed_slice())
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subject_matcher_macro() {
        // Test static segments
        let matcher = subject_matcher!["foo", "bar"];
        assert_eq!(
            matcher.0.as_ref(),
            &[
                SubjectMatcherField::Static("foo".into()),
                SubjectMatcherField::Static("bar".into()),
            ]
        );

        // Test wildcards
        let matcher = subject_matcher!["foo", "*", "bar"];
        assert_eq!(
            matcher.0.as_ref(),
            &[
                SubjectMatcherField::Static("foo".into()),
                SubjectMatcherField::Wildcard,
                SubjectMatcherField::Static("bar".into()),
            ]
        );

        // Test recursive wildcard
        let matcher = subject_matcher!["foo", "bar", ">"];
        assert_eq!(
            matcher.0.as_ref(),
            &[
                SubjectMatcherField::Static("foo".into()),
                SubjectMatcherField::Static("bar".into()),
                SubjectMatcherField::RecursiveWildcard,
            ]
        );
    }

    #[test]
    #[should_panic(expected = "Multiple '>' wildcards are not allowed")]
    fn test_multiple_recursive_wildcards() {
        subject_matcher!["foo", ">", ">"];
    }

    #[test]
    #[should_panic(expected = "'>' wildcard must be the last segment")]
    fn test_recursive_wildcard_not_last() {
        subject_matcher!["foo", ">", "bar"];
    }

    #[test]
    fn test_subject_matcher() {
        let matcher = subject_matcher!["foo", "bar", "baz"];
        assert!(matcher.matches(&subject_path!["foo", "bar", "baz"]));
        assert!(!matcher.matches(&subject_path!["foo", "bar"]));
    }

    #[test]
    fn test_subject_matcher_comprehensive() {
        // Test exact matching
        let matcher = subject_matcher!["foo", "bar", "baz"];
        assert!(matcher.matches(&subject_path!["foo", "bar", "baz"]));
        assert!(!matcher.matches(&subject_path!["foo", "bar"]));
        assert!(!matcher.matches(&subject_path!["foo", "bar", "baz", "qux"]));
        assert!(!matcher.matches(&subject_path!["foo", "baz", "bar"]));

        // Test single wildcard matching
        let matcher = subject_matcher!["foo", "*", "baz"];
        assert!(matcher.matches(&subject_path!["foo", "bar", "baz"]));
        assert!(matcher.matches(&subject_path!["foo", "anything", "baz"]));
        assert!(!matcher.matches(&subject_path!["foo", "bar"]));
        assert!(!matcher.matches(&subject_path!["foo", "bar", "baz", "qux"]));

        // Test recursive wildcard matching
        let matcher = subject_matcher!["foo", ">"];
        assert!(matcher.matches(&subject_path!["foo", "bar"]));
        assert!(matcher.matches(&subject_path!["foo", "bar", "baz"]));
        assert!(matcher.matches(&subject_path!["foo", "bar", "baz", "qux"]));
        assert!(!matcher.matches(&subject_path!["not_foo", "bar", "baz"]));

        // Test mixed wildcards
        let matcher = subject_matcher!["foo", "*", ">"];
        assert!(matcher.matches(&subject_path!["foo", "bar", "baz"]));
        assert!(matcher.matches(&subject_path!["foo", "anything", "baz", "qux"]));
        assert!(!matcher.matches(&subject_path!["foo"]));

        // Test empty matcher
        let matcher = subject_matcher![];
        assert!(matcher.matches(&subject_path![]));
        assert!(!matcher.matches(&subject_path!["foo"]));

        // Test empty subject
        let matcher = subject_matcher!["foo", "bar"];
        assert!(!matcher.matches(&subject_path![]));
    }
}
