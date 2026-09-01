/// One independently deliverable browser product surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProductSurface {
    Host,
    Book,
    Creche,
}

impl ProductSurface {
    pub(super) fn permits(self, request: Option<&str>) -> bool {
        match self {
            Self::Host => true,
            Self::Book => request.is_some_and(|line| line.starts_with("GET /book/")),
            Self::Creche => request.is_some_and(|line| line.starts_with("GET /creche/")),
        }
    }
}
