use conduit_plot::rust_binding::NativeRustBinding;
use conduit_web::HttpContractError;

#[test]
fn http_contract_errors_have_native_identity_and_exact_round_trips() {
    for value in [
        HttpContractError::EmptyAuthority,
        HttpContractError::InvalidScheme,
        HttpContractError::InvalidTarget,
        HttpContractError::TooManyHeaders,
        HttpContractError::EmptyHeaderName,
        HttpContractError::InvalidHeaderName,
        HttpContractError::SensitiveHeaderRequiresProtectedPath,
        HttpContractError::FramingHeaderIsDerived,
        HttpContractError::HeaderNameOverflow,
        HttpContractError::HeaderValueOverflow,
        HttpContractError::RequestBodyOverflow,
        HttpContractError::ResponseBodyOverflow,
        HttpContractError::InvalidStatus,
        HttpContractError::MalformedEncoding,
        HttpContractError::EncodedValueOverflow,
        HttpContractError::TrailingBytes,
    ] {
        let structured = value.into_structured().unwrap();
        assert_eq!(
            HttpContractError::from_structured(structured).unwrap(),
            value
        );
    }
}
