//! Generated Native admission of an immutable actual early-vocative receipt.
use conduit_language::{LanguageParserIndependentProtectedAdmission, LanguageTextFinality};
use conduit_plot::rust_binding::NativeRustBinding;

#[test]
#[ignore = "requires retained actual receipt via CONDUIT_INDEPENDENT_RECEIPT_PATH"]
fn actual_partial_vocative_receipt_readmits_without_contiguous_root_commit() {
    let path = std::env::var("CONDUIT_INDEPENDENT_RECEIPT_PATH")
        .expect("exact retained acquisition receipt path");
    let bytes = std::fs::read(path).unwrap();
    let receipt = LanguageParserIndependentProtectedAdmission::decode(&bytes).unwrap();
    let admission = receipt.admission();
    let query = admission.fact().query();
    assert_eq!(*query.dependent(), 2);
    assert_eq!(*admission.head(), 0);
    assert_eq!(
        *query.beam().lexical().tape().source().finality(),
        LanguageTextFinality::Partial
    );
    assert_eq!(
        query.beam().lexical().tape().source().material().text(),
        "Hello, Travis "
    );
    assert!(receipt.output().active()[2]);
    assert_eq!(
        receipt.output().edge2().origin_basis(),
        query.beam().basis()
    );
    assert_eq!(
        receipt.output().edge2().current_basis(),
        query.beam().basis()
    );
    assert_eq!(receipt.clone().encode().unwrap(), bytes);
    assert!(
        LanguageParserIndependentProtectedAdmission::decode(&bytes[..bytes.len() - 1]).is_err()
    );
    assert!(LanguageParserIndependentProtectedAdmission::decode(&[]).is_err());
}
