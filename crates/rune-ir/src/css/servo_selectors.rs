//! Optional integration surface for Servo's `selectors` crate.
//! This module is behind the `servo_selectors` feature and currently
//! scaffolds types so we can wire in real matching incrementally.

#[cfg(feature = "servo_selectors")]
mod impls {
    use super::super::servo_dom::DomElement;
    use selectors::NthIndexCache;
    use selectors::{
        context::QuirksMode,
        matching::{
            IgnoreNthChildForInvalidation, MatchingContext, MatchingMode, NeedsSelectorFlags,
            matches_selector_list,
        },
        parser::{
            ParseRelative, Parser as SelParser, SelectorImpl, SelectorList, SelectorParseErrorKind,
        },
    };
    // Use cssparser types only in the local parse function to avoid collisions elsewhere.
    use cssparser::{
        CssStringWriter, Parser as CssParser, ParserInput, ToCss, serialize_identifier,
    };

    // Local types to satisfy trait bounds and avoid orphan rule issues.
    #[derive(Clone, Debug, Default, Eq, PartialEq, Hash)]
    pub struct CssIdent(pub String);

    impl AsRef<str> for CssIdent {
        fn as_ref(&self) -> &str {
            &self.0
        }
    }
    impl std::borrow::Borrow<str> for CssIdent {
        fn borrow(&self) -> &str {
            &self.0
        }
    }
    impl<'a> From<&'a str> for CssIdent {
        fn from(s: &'a str) -> Self {
            Self(s.to_owned())
        }
    }
    impl ToCss for CssIdent {
        fn to_css<W>(&self, dest: &mut W) -> std::fmt::Result
        where
            W: std::fmt::Write,
        {
            serialize_identifier(&self.0, dest)
        }
    }

    #[derive(Clone, Debug, Default, Eq, PartialEq, Hash)]
    pub struct CssAttrValue(pub String);

    impl AsRef<str> for CssAttrValue {
        fn as_ref(&self) -> &str {
            &self.0
        }
    }
    impl<'a> From<&'a str> for CssAttrValue {
        fn from(s: &'a str) -> Self {
            Self(s.to_owned())
        }
    }
    impl ToCss for CssAttrValue {
        fn to_css<W>(&self, dest: &mut W) -> std::fmt::Result
        where
            W: std::fmt::Write,
        {
            use std::fmt::Write;
            write!(CssStringWriter::new(dest), "{}", &self.0)
        }
    }

    // A minimal SelectorImpl with no pseudos.
    #[derive(Clone, Debug)]
    pub enum SimpleImpl {}
    impl SelectorImpl for SimpleImpl {
        type ExtraMatchingData<'a> = ();
        type AttrValue = CssAttrValue;
        type Identifier = CssIdent;
        type LocalName = CssIdent;
        type NamespaceUrl = CssIdent;
        type NamespacePrefix = CssIdent;
        type BorrowedNamespaceUrl = str;
        type BorrowedLocalName = str;
        type NonTSPseudoClass = Never;
        type PseudoElement = Never;
    }

    // Zero-variant types used to disable pseudos in this minimal impl.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum Never {}

    impl ToCss for Never {
        fn to_css<W>(&self, _dest: &mut W) -> std::fmt::Result
        where
            W: std::fmt::Write,
        {
            Ok(())
        }
    }

    impl selectors::parser::NonTSPseudoClass for Never {
        type Impl = SimpleImpl;
        fn is_active_or_hover(&self) -> bool {
            false
        }
        fn is_user_action_state(&self) -> bool {
            false
        }
    }

    impl selectors::parser::PseudoElement for Never {
        type Impl = SimpleImpl;
        fn accepts_state_pseudo_classes(&self) -> bool {
            false
        }
        fn valid_after_slotted(&self) -> bool {
            false
        }
    }

    // (No foreign trait impls; ToCss provided via local newtypes above.)

    #[derive(Clone, Debug)]
    pub struct ServoSelectorList(pub SelectorList<SimpleImpl>);

    #[derive(Default)]
    struct BasicParser;

    impl<'i> SelParser<'i> for BasicParser {
        type Impl = SimpleImpl;
        type Error = SelectorParseErrorKind<'i>;
    }

    pub fn parse_list(input: &str) -> Option<ServoSelectorList> {
        let mut input = ParserInput::new(input);
        let mut parser = CssParser::new(&mut input);
        let list =
            SelectorList::parse(&BasicParser::default(), &mut parser, ParseRelative::No).ok()?;
        Some(ServoSelectorList(list))
    }

    pub fn matches_subset(list: &ServoSelectorList, el: &DomElement) -> bool {
        let mut cache = NthIndexCache::default();
        let mut ctx = MatchingContext::new(
            MatchingMode::Normal,
            None,
            &mut cache,
            QuirksMode::NoQuirks,
            NeedsSelectorFlags::No,
            IgnoreNthChildForInvalidation::No,
        );
        matches_selector_list(&list.0, el, &mut ctx)
    }

    pub fn specificity_of(list: &ServoSelectorList) -> u32 {
        // Best-effort: take the maximum specificity among the selector list.
        list.0
            .0
            .iter()
            .map(|sel| sel.specificity())
            .max()
            .unwrap_or(0)
    }
}

// Public, but only compiled when the feature is enabled.
#[cfg(feature = "servo_selectors")]
pub use impls::*;
