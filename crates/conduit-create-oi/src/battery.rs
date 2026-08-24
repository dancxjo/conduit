//! Create 1 battery-estimate interpretation below portable robotics meaning.
//!
//! OI v2 reports charge and estimated capacity as independent unsigned
//! 16-bit readings. It does not promise that the two estimates form an
//! already-bounded fraction, so that relationship must not be used to reject
//! an otherwise valid sensor frame.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Create1BatteryEstimate {
    pub reported_charge_mah: u16,
    pub reported_capacity_mah: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Create1BatteryNormalizationDisposition {
    Exact,
    ChargeSaturatedToEstimatedCapacity,
    EstimatedCapacityUnavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NormalizedCreate1BatteryEstimate {
    pub reported: Create1BatteryEstimate,
    pub charge_mah: u16,
    pub capacity_mah: u16,
    pub disposition: Create1BatteryNormalizationDisposition,
}

impl Create1BatteryEstimate {
    pub const fn normalize(self) -> NormalizedCreate1BatteryEstimate {
        if self.reported_capacity_mah == 0 {
            return NormalizedCreate1BatteryEstimate {
                reported: self,
                charge_mah: 0,
                capacity_mah: 0,
                disposition: Create1BatteryNormalizationDisposition::EstimatedCapacityUnavailable,
            };
        }
        if self.reported_charge_mah > self.reported_capacity_mah {
            return NormalizedCreate1BatteryEstimate {
                reported: self,
                charge_mah: self.reported_capacity_mah,
                capacity_mah: self.reported_capacity_mah,
                disposition:
                    Create1BatteryNormalizationDisposition::ChargeSaturatedToEstimatedCapacity,
            };
        }
        NormalizedCreate1BatteryEstimate {
            reported: self,
            charge_mah: self.reported_charge_mah,
            capacity_mah: self.reported_capacity_mah,
            disposition: Create1BatteryNormalizationDisposition::Exact,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_1_estimates_normalize_without_rejecting_canonical_oi_ranges() {
        let exact = Create1BatteryEstimate {
            reported_charge_mah: 1_200,
            reported_capacity_mah: 2_400,
        }
        .normalize();
        assert_eq!((exact.charge_mah, exact.capacity_mah), (1_200, 2_400));
        assert_eq!(
            exact.disposition,
            Create1BatteryNormalizationDisposition::Exact
        );

        let saturated = Create1BatteryEstimate {
            reported_charge_mah: 2_401,
            reported_capacity_mah: 2_400,
        }
        .normalize();
        assert_eq!(saturated.reported.reported_charge_mah, 2_401);
        assert_eq!(
            (saturated.charge_mah, saturated.capacity_mah),
            (2_400, 2_400)
        );
        assert_eq!(
            saturated.disposition,
            Create1BatteryNormalizationDisposition::ChargeSaturatedToEstimatedCapacity
        );

        let unavailable = Create1BatteryEstimate {
            reported_charge_mah: 1_200,
            reported_capacity_mah: 0,
        }
        .normalize();
        assert_eq!(unavailable.reported.reported_charge_mah, 1_200);
        assert_eq!((unavailable.charge_mah, unavailable.capacity_mah), (0, 0));
        assert_eq!(
            unavailable.disposition,
            Create1BatteryNormalizationDisposition::EstimatedCapacityUnavailable
        );
    }
}
