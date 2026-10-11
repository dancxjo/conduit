//! Installed browser physical triad transport and pinned Dimension inspection.
use super::*;

#[test]
fn browser_dimension_introspection_preserves_the_triad_across_record_ports() {
    for (kind, value) in [
        (conduit_plot::dimension_operation::UNIT_KIND, "doublestep"),
        (
            conduit_plot::dimension_operation::QUANTITY_KIND,
            "3doublestep",
        ),
    ] {
        let source = format!("dimension travel\ndimension clock\ndimension velocity = travel / clock\ndimension velocity2 = {{travel:1,clock:-1}}\ntype Motion = quantity {{dimension:velocity}}\nunit stepper : Motion = {{reference:origin,scale:1}}\nunit doublestep : Motion = {{reference:stepper,scale:2}}\ntype PhysicalTriad = {{\n dimension: Dimension\n unit: Unit\n quantity: Motion\n}}\nplot wrap (\n >> input: Dimension\n triad: PhysicalTriad <= 4096B >>\n) = ({{dimension:.,unit:doublestep,quantity:3doublestep}})\nplot inspect (\n >> input: PhysicalTriad <= 4096B\n result: Boolean >>\n) = (.dimension == velocity2 && .unit == doublestep && .quantity == 6stepper)\nplot demo {{\n operation: {kind}(source={value})\n show: presentation/bool-value\n pack: wrap\n check: inspect\n operation.dimension >> pack.input\n pack.triad >> check.input\n check.result >> show.value\n}}.\n");
        let (mut session, effect) =
            TourSession::prepare("browser/dimension", "boot/dimension", &source, 1)
                .unwrap_or_else(|error| panic!("{error}\n{source}"));
        let TourHostEffect::Manifestation(effect) = effect else {
            panic!("Boolean manifestation");
        };
        assert_eq!(effect.text.as_deref(), Some("true"));
        assert_eq!(effect.plan_id, session.fragments[0].plan_id.as_str());
        assert!(effect
            .expanded_gears
            .iter()
            .any(|gear| gear.kind_id == kind));
        let capacity = session.scheduler.values().allocation_capacities();
        let TourProgress::Receipt(receipt) = session.advance().unwrap() else {
            panic!("completion receipt");
        };
        assert_eq!(receipt.disposition, "completed");
        assert_eq!(session.scheduler.values().allocation_capacities(), capacity);
    }
}

#[test]
fn browser_dimension_advertisement_preserves_the_codec_queue_bound() {
    let host = crate::installed_browser::factory::advertisement(
        conduit_core::HostId::from("dimension-host"),
        conduit_core::BootId::from("dimension-boot"),
    );
    for kind in [
        conduit_plot::dimension_operation::UNIT_KIND,
        conduit_plot::dimension_operation::QUANTITY_KIND,
    ] {
        let offer = host
            .capabilities
            .iter()
            .find(|offer| offer.kind_id.as_str() == kind)
            .unwrap();
        assert_eq!(
            offer.limits.max_queue_bytes,
            conduit_core::DIMENSION_ENCODED_LEN as u32
        );
    }
}
