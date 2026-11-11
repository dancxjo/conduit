// No computation Back is installed. Native admission remains independent.
pub const ENTRY_COUNT: usize = 0;
pub const VALUE_SLOT_BYTES: usize = 0;
#[derive(Debug)]
pub enum PreparationRefusal {}
impl PreparationRefusal {
    pub fn entry(&self) -> usize {
        match *self {}
    }
}
pub struct Initialized;
impl Initialized {
    pub fn prepare() -> Result<Self, PreparationRefusal> {
        Ok(Self)
    }
    pub fn offers(&self) -> [Option<&'static [u8]>; 0] {
        []
    }
    pub fn available_identity(
        &self,
        _: usize,
    ) -> Option<
        &'static conduit_plan_lowering::selected_assigned_expression::ResidentExpressionIdentity,
    > {
        None
    }
    pub fn available_value_slot_bytes(&self, _: usize) -> Option<usize> {
        None
    }
    pub fn identify_assignments(
        &self,
        bytes: &[u8],
    ) -> Result<
        conduit_plan_lowering::resident_inventory::ResidentInventorySelection<0>,
        conduit_plan_lowering::selected_assigned_expression::ResidentExpressionBindingRefusal,
    > {
        conduit_plan_lowering::resident_inventory::select_resident_inventory(bytes, |_| None)
    }
    pub fn identify_mixed_assignments(
        &self,
        bytes: &[u8],
        other: &[conduit_plan_lowering::resident_inventory::OtherNodeIdentity],
    ) -> Result<
        conduit_plan_lowering::resident_inventory::ResidentInventorySelection<0>,
        conduit_plan_lowering::selected_assigned_expression::ResidentExpressionBindingRefusal,
    > {
        conduit_plan_lowering::resident_inventory::select_resident_inventory_with_other_nodes(
            bytes,
            other,
            |_| None,
        )
    }
    pub fn check_entry(
        &self,
        _: usize,
        _: conduit_plan_lowering::selected_assigned_expression::SelectedAssignedExpressionPlan<'_>,
    ) -> Result<
        u16,
        conduit_plan_lowering::selected_assigned_expression::ResidentExpressionBindingRefusal,
    > {
        Err(conduit_plan_lowering::selected_assigned_expression::ResidentExpressionBindingRefusal::Selection)
    }
    pub fn prepare_selected_array(
        &mut self,
        _: [Option<
            conduit_plan_lowering::selected_assigned_expression::SelectedAssignedExpressionPlan<'_>,
        >; 0],
    ) -> Result<
        PreparedResidentLoans<'_>,
        conduit_plan_lowering::selected_assigned_expression::ResidentExpressionBindingRefusal,
    > {
        Ok(PreparedResidentLoans { _owner: self })
    }
    pub fn borrow_selected(
        &mut self,
        selected: [Option<
            conduit_plan_lowering::selected_assigned_expression::SelectedAssignedExpressionPlan<'_>,
        >; 0],
    ) -> Result<
        (),
        conduit_plan_lowering::selected_assigned_expression::ResidentExpressionBindingRefusal,
    > {
        self.prepare_selected_array(selected).map(|_| ())
    }
    pub fn borrow_selected_array(
        &mut self,
        selected: [Option<
            conduit_plan_lowering::selected_assigned_expression::SelectedAssignedExpressionPlan<'_>,
        >; 0],
    ) -> Result<
        [Option<(u16, BorrowedBack<'_>)>; 0],
        conduit_plan_lowering::selected_assigned_expression::ResidentExpressionBindingRefusal,
    > {
        Ok(self.prepare_selected_array(selected)?.bind())
    }
    pub fn into_backs(self) {}
}
pub struct PreparedResidentLoans<'a> {
    _owner: &'a mut Initialized,
}
impl<'a> PreparedResidentLoans<'a> {
    pub fn terminal_transductions<const P: usize>(
        &self,
    ) -> [Option<[Option<conduit_kernel::scheduler::AssignedTerminalTransduction>; P]>; 0] {
        []
    }
    pub fn nodes(&self) -> [Option<u16>; 0] {
        []
    }
    pub fn split(self) -> [Option<PreparedResidentEntryLoan<'a>>; 0] {
        []
    }
    pub fn bind(self) -> [Option<(u16, BorrowedBack<'a>)>; 0] {
        []
    }
}
// Impossible to construct: zero inventory cannot produce a computation Back.
pub struct BorrowedBack<'a> {
    never: core::convert::Infallible,
    _loan: core::marker::PhantomData<&'a mut Initialized>,
}
impl BorrowedBack<'_> {
    pub fn finish_retired(self) -> Result<(), Self> {
        match self.never {}
    }
}
impl<const P: usize> conduit_kernel::scheduler::StepBack<P> for BorrowedBack<'_> {
    fn step(
        &mut self,
        _: &mut conduit_kernel::scheduler::StepIo<P>,
        _: &conduit_kernel::scheduler::StepInputBytes<'_, P>,
    ) -> conduit_kernel::scheduler::StepOutcome {
        match self.never {}
    }
    fn prepared_output(&self, _: conduit_kernel::PortId) -> Option<&[u8]> {
        match self.never {}
    }
    fn step_committed(&mut self) {
        match self.never {}
    }
    fn cancel(&mut self) {
        match self.never {}
    }
}

pub struct PreparedResidentEntryLoan<'a> {
    never: core::convert::Infallible,
    _loan: core::marker::PhantomData<&'a mut Initialized>,
}
impl<'a> PreparedResidentEntryLoan<'a> {
    pub fn terminal_transductions<const P: usize>(
        &self,
    ) -> [Option<conduit_kernel::scheduler::AssignedTerminalTransduction>; P] {
        match self.never {}
    }
    pub fn node(&self) -> u16 {
        match self.never {}
    }
    pub fn bind(self) -> (u16, BorrowedBack<'a>) {
        match self.never {}
    }
}
