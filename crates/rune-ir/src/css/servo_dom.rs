#[cfg(feature = "servo_selectors")]
use crate::css::servo_selectors::SimpleImpl;
use ego_tree::NodeRef;
use scraper::{ElementRef, Node};
#[cfg(feature = "servo_selectors")]
use selectors::attr::{AttrSelectorOperation, CaseSensitivity, NamespaceConstraint};
#[cfg(feature = "servo_selectors")]
use selectors::{Element as ServoElement, OpaqueElement};

/// Thin DOM wrapper aligned with what Servo's selectors Element trait needs.
/// We intentionally keep only the subset we match (tag/id/class) and ancestry.
#[derive(Clone, Debug)]
pub struct DomElement<'a> {
    inner: ElementRef<'a>,
}

#[allow(dead_code)]
impl<'a> DomElement<'a> {
    pub fn new(inner: ElementRef<'a>) -> Self {
        Self { inner }
    }
    pub fn tag_name(&self) -> &str {
        self.inner.value().name()
    }
    pub fn id(&self) -> Option<&str> {
        self.inner.value().attr("id")
    }
    pub fn classes(&self) -> impl Iterator<Item = &str> {
        self.inner.value().classes()
    }
    pub fn parent(&self) -> Option<DomNode<'a>> {
        self.inner.parent().map(DomNode)
    }
    pub fn as_element_ref(&self) -> ElementRef<'a> {
        self.inner.clone()
    }
}

/// Node wrapper used to walk to ancestors (elements only when wrapped back).
#[derive(Debug)]
#[allow(dead_code)]
pub struct DomNode<'a>(NodeRef<'a, Node>);

#[allow(dead_code)]
impl<'a> DomNode<'a> {
    pub fn parent(&self) -> Option<DomNode<'a>> {
        self.0.parent().map(DomNode)
    }
    pub fn as_element(&self) -> Option<DomElement<'a>> {
        ElementRef::wrap(self.0.clone()).map(DomElement::new)
    }
}

#[cfg(feature = "servo_selectors")]
impl<'a> ServoElement for DomElement<'a> {
    type Impl = SimpleImpl;

    fn opaque(&self) -> OpaqueElement {
        OpaqueElement::new(&self.inner)
    }

    fn parent_element(&self) -> Option<Self> {
        self.inner
            .parent()
            .and_then(|n| ElementRef::wrap(n))
            .map(DomElement::new)
    }

    fn parent_node_is_shadow_root(&self) -> bool {
        false
    }

    fn containing_shadow_host(&self) -> Option<Self> {
        None
    }

    fn prev_sibling_element(&self) -> Option<Self> {
        #[cfg(feature = "servo_selectors_wide")]
        {
            return self
                .inner
                .prev_siblings()
                .find(|sib| sib.value().is_element())
                .and_then(ElementRef::wrap)
                .map(DomElement::new);
        }
        #[allow(unreachable_code)]
        None
    }

    fn next_sibling_element(&self) -> Option<Self> {
        #[cfg(feature = "servo_selectors_wide")]
        {
            return self
                .inner
                .next_siblings()
                .find(|sib| sib.value().is_element())
                .and_then(ElementRef::wrap)
                .map(DomElement::new);
        }
        #[allow(unreachable_code)]
        None
    }

    fn first_element_child(&self) -> Option<Self> {
        #[cfg(feature = "servo_selectors_wide")]
        {
            return self
                .inner
                .children()
                .find(|child| child.value().is_element())
                .and_then(ElementRef::wrap)
                .map(DomElement::new);
        }
        #[allow(unreachable_code)]
        None
    }

    fn is_pseudo_element(&self) -> bool {
        false
    }

    fn is_html_element_in_html_document(&self) -> bool {
        true
    }

    fn has_local_name(
        &self,
        name: &<Self::Impl as selectors::parser::SelectorImpl>::BorrowedLocalName,
    ) -> bool {
        self.inner.value().name().eq_ignore_ascii_case(name)
    }

    fn has_namespace(
        &self,
        _ns: &<Self::Impl as selectors::parser::SelectorImpl>::BorrowedNamespaceUrl,
    ) -> bool {
        true
    }

    fn is_same_type(&self, other: &Self) -> bool {
        self.inner
            .value()
            .name()
            .eq_ignore_ascii_case(other.inner.value().name())
    }

    fn attr_matches(
        &self,
        ns: &NamespaceConstraint<&<Self::Impl as selectors::parser::SelectorImpl>::NamespaceUrl>,
        local_name: &<Self::Impl as selectors::parser::SelectorImpl>::LocalName,
        operation: &AttrSelectorOperation<
            &<Self::Impl as selectors::parser::SelectorImpl>::AttrValue,
        >,
    ) -> bool {
        match ns {
            NamespaceConstraint::Any | NamespaceConstraint::Specific(_) => {}
        }
        if let Some(value) = self.inner.value().attr(local_name.as_ref()) {
            return operation.eval_str(value);
        }
        false
    }

    fn match_non_ts_pseudo_class(
        &self,
        _pc: &<Self::Impl as selectors::parser::SelectorImpl>::NonTSPseudoClass,
        _context: &mut selectors::matching::MatchingContext<Self::Impl>,
    ) -> bool {
        false
    }

    fn match_pseudo_element(
        &self,
        _pe: &<Self::Impl as selectors::parser::SelectorImpl>::PseudoElement,
        _context: &mut selectors::matching::MatchingContext<Self::Impl>,
    ) -> bool {
        false
    }

    fn apply_selector_flags(&self, _flags: selectors::matching::ElementSelectorFlags) {}

    fn is_link(&self) -> bool {
        if !self.inner.value().name().eq_ignore_ascii_case("a") {
            return false;
        }
        self.inner.value().attr("href").is_some()
    }

    fn is_html_slot_element(&self) -> bool {
        false
    }

    fn imported_part(
        &self,
        _name: &<Self::Impl as selectors::parser::SelectorImpl>::Identifier,
    ) -> Option<<Self::Impl as selectors::parser::SelectorImpl>::Identifier> {
        None
    }

    fn is_part(&self, _name: &<Self::Impl as selectors::parser::SelectorImpl>::Identifier) -> bool {
        false
    }

    fn has_id(
        &self,
        id: &<Self::Impl as selectors::parser::SelectorImpl>::Identifier,
        case: CaseSensitivity,
    ) -> bool {
        if let Some(v) = self.inner.value().attr("id") {
            let id_str = id.as_ref();
            return match case {
                CaseSensitivity::CaseSensitive => v == id_str,
                CaseSensitivity::AsciiCaseInsensitive => v.eq_ignore_ascii_case(id_str),
            };
        }
        false
    }

    fn has_class(
        &self,
        name: &<Self::Impl as selectors::parser::SelectorImpl>::Identifier,
        case: CaseSensitivity,
    ) -> bool {
        let needle = name.as_ref();
        for c in self.inner.value().classes() {
            if match case {
                CaseSensitivity::CaseSensitive => c == needle,
                CaseSensitivity::AsciiCaseInsensitive => c.eq_ignore_ascii_case(needle),
            } {
                return true;
            }
        }
        false
    }

    fn is_empty(&self) -> bool {
        self.inner.children().next().is_none()
    }

    fn is_root(&self) -> bool {
        self.parent_element().is_none()
    }
}
