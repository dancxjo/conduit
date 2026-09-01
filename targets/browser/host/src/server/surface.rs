/// One independently deliverable browser product surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProductSurface {
    Host,
    Book,
    Creche,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ProductDocument {
    Book,
    Creche,
}

const BOOK_DOCUMENT_PATHS: &[&str] = &[
    "/book/",
    "/book/meet-one-gear/",
    "/book/connect-gears/",
    "/book/edit-one-gear/",
    "/book/branch-a-cord/",
    "/book/meet-the-face/",
    "/book/same-face-different-implementation/",
    "/book/meet-the-host/",
    "/book/state-over-time/",
    "/book/use-two-hosts/",
    "/book/plans-and-plays/",
    "/book/keep-one-body-through-change/",
    "/book/birth-belongs-in-the-creche/",
    "/book/give-the-body-a-host/",
    "/book/graduate-from-the-creche/",
];

const CRECHE_DOCUMENT_PATHS: &[&str] = &[
    "/creche/",
    "/creche/birth/",
    "/creche/first-host/",
    "/creche/physical-host/",
    "/creche/graduate/",
];

impl ProductSurface {
    pub(super) fn permits(self, request: Option<&str>) -> bool {
        match self {
            Self::Host => true,
            Self::Book => request.is_some_and(|line| line.starts_with("GET /book/")),
            Self::Creche => request.is_some_and(|line| line.starts_with("GET /creche/")),
        }
    }

    pub(super) fn document(self, request: Option<&str>) -> Option<ProductDocument> {
        let path = request?.strip_prefix("GET ")?.strip_suffix(" HTTP/1.1")?;
        match self {
            Self::Book if BOOK_DOCUMENT_PATHS.contains(&path) => Some(ProductDocument::Book),
            Self::Creche if CRECHE_DOCUMENT_PATHS.contains(&path) => Some(ProductDocument::Creche),
            _ => None,
        }
    }
}
