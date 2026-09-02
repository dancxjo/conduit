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
    "/book/a-form-you-can-run/",
    "/book/faces-backs-and-implementation/",
    "/book/hosts-make-forms-real/",
    "/book/one-form-across-several-hosts/",
    "/book/the-body-one-computer-one-machine-or-many/",
    "/book/many-forms-one-body-wide-realization/",
    "/book/birth-spores-and-the-creche/",
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
