//! All 76 original Source class derivations. Locators are private references into
//! the retained book; complete program identity and ordered full I/O establish
//! parentage whenever a result is observed.
use crate::{
    parser_session_canonical_ingress::ParserCanonicalSourceExecutor,
    parser_session_numeric_custody::ParserNumericExecutor,
    parser_session_window8_atoms::{PreparedWindow8Atoms, Window8Atom as A},
    parser_session_window8_book::Window8Book,
    parser_session_window8_ports::{PORTS, port_index},
    parser_session_window8_queries::PreparedWindow8Queries,
    parser_session_window8_stage::Window8RevisionStage,
    parser_session_window8_values::{field, unsigned, view, View},
};
#[derive(Clone, Copy)]
pub(crate) struct Window8ClassDerivation {
    code: u8,
    index: usize,
    relations: usize,
    value: usize,
}
#[derive(Debug)]
pub(crate) struct ClassDerivationRefusal;
impl Window8ClassDerivation {
    pub(crate) fn code(&self) -> u8 { self.code }
    pub(crate) fn value<'a>(&self, book: &'a Window8Book) -> Result<View<'a>, ClassDerivationRefusal> {
        let names = ["language-window8-class-index", "language-window8-class-relations", "language-window8-class-relation"];
        let indices = [self.index, self.relations, self.value];
        for (name, index) in names.into_iter().zip(indices) {
            let history = book.source.get(index).ok_or(ClassDerivationRefusal)?;
            let port = &PORTS[port_index(name).ok_or(ClassDerivationRefusal)?];
            if !history.matches_fixed(port.original_programs, port.original_custody, port.input, port.output) {
                return Err(ClassDerivationRefusal);
            }
        }
        for pair in indices.windows(2) {
            if book.source[pair[0]].output_bytes() != book.source[pair[1]].input_bytes() {
                return Err(ClassDerivationRefusal);
            }
        }
        let query = view(book.source[self.index].input_bytes()).map_err(|_| ClassDerivationRefusal)?;
        if unsigned(field(query, "code").map_err(|_| ClassDerivationRefusal)?)
            .map_err(|_| ClassDerivationRefusal)? != u64::from(self.code) {
            return Err(ClassDerivationRefusal);
        }
        view(book.source[self.value].output_bytes()).map_err(|_| ClassDerivationRefusal)
    }
}
pub(crate) fn derive_all<S: ParserCanonicalSourceExecutor, N: ParserNumericExecutor>(
    stage: &mut Window8RevisionStage<'_, S, N>,
    queries: &mut PreparedWindow8Queries,
    atoms: &mut PreparedWindow8Atoms,
) -> Result<[Window8ClassDerivation; 76], ClassDerivationRefusal> {
    let mut classes = [Window8ClassDerivation { code: 0, index: 0, relations: 0, value: 0 }; 76];
    for (code, destination) in classes.iter_mut().enumerate() {
        atoms.unsigned(code as u64).map_err(|_| ClassDerivationRefusal)?;
        let query = queries.record_fields("language-window8-class-index", &[
            ("code", view(atoms.encoded(A::Unsigned).map_err(|_| ClassDerivationRefusal)?)
                .map_err(|_| ClassDerivationRefusal)?),
            ("default_relation", view(atoms.encoded(A::DefaultRelation).map_err(|_| ClassDerivationRefusal)?)
                .map_err(|_| ClassDerivationRefusal)?),
        ]).map_err(|_| ClassDerivationRefusal)?;
        let index = stage.source_named("language-window8-class-index", query, 0, 0)
            .map_err(|_| ClassDerivationRefusal)?;
        let query = queries.copy_record("language-window8-class-relations",
            view(stage.book().source[index].output_bytes()).map_err(|_| ClassDerivationRefusal)?)
            .map_err(|_| ClassDerivationRefusal)?;
        let relations = stage.source_named("language-window8-class-relations", query, 0, 0)
            .map_err(|_| ClassDerivationRefusal)?;
        let query = queries.copy_record("language-window8-class-relation",
            view(stage.book().source[relations].output_bytes()).map_err(|_| ClassDerivationRefusal)?)
            .map_err(|_| ClassDerivationRefusal)?;
        let value = stage.source_named("language-window8-class-relation", query, 0, 0)
            .map_err(|_| ClassDerivationRefusal)?;
        *destination = Window8ClassDerivation { code: code as u8, index, relations, value };
    }
    Ok(classes)
}
