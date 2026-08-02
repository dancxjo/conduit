use conduit_runtime::{
    CurrentUpdateRequest, CurrentValueCell, CurrentValueMutationAuthorizer, CurrentValueUpdateError,
};

struct Authorizer {
    admit: bool,
    requests: Vec<CurrentUpdateRequest>,
}

impl CurrentValueMutationAuthorizer for Authorizer {
    type Error = &'static str;

    fn authorize(&mut self, request: CurrentUpdateRequest) -> Result<(), Self::Error> {
        self.requests.push(request);
        if self.admit {
            Ok(())
        } else {
            Err("grant-denied")
        }
    }
}

#[test]
fn late_subscriber_receives_the_current_value_immediately() {
    let cell = CurrentValueCell::new("initial");
    let observation = cell.observe();
    assert_eq!(observation.generation, 0);
    assert_eq!(observation.value, &"initial");
}

#[test]
fn update_without_a_subscriber_is_retained_for_reconnect() {
    let mut cell = CurrentValueCell::new(1_u8);
    let disconnected_at = cell.observe().generation;
    let mut authorizer = Authorizer {
        admit: true,
        requests: Vec::new(),
    };
    cell.replace(2, &mut authorizer).unwrap();
    cell.replace(3, &mut authorizer).unwrap();

    let reconnected = cell.observe_since(disconnected_at).unwrap().unwrap();
    assert_eq!(reconnected.value, &3);
    assert!(reconnected.skipped_replacements);
}

#[test]
fn mutation_authority_is_separate_and_checked_before_replacement() {
    let mut cell = CurrentValueCell::new(1_u8);
    let mut authorizer = Authorizer {
        admit: false,
        requests: Vec::new(),
    };
    assert_eq!(
        cell.replace(2, &mut authorizer),
        Err(CurrentValueUpdateError::Unauthorized("grant-denied"))
    );
    assert_eq!(cell.observe().value, &1);
    assert_eq!(
        authorizer.requests,
        vec![CurrentUpdateRequest {
            current_generation: 0,
            next_generation: 1,
        }]
    );
}

#[test]
fn equal_updates_are_replacements_not_implicit_deduplication() {
    let mut cell = CurrentValueCell::new(7_u8);
    let mut authorizer = Authorizer {
        admit: true,
        requests: Vec::new(),
    };
    cell.replace(7, &mut authorizer).unwrap();
    assert_eq!(cell.observe().generation, 1);
}
