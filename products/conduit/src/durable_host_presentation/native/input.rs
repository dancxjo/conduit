//! Physical occurrences keep their original Show and pending action custody.
use super::*;

impl NativePresentation {
    pub(super) fn finish_input(&mut self, runtime: &mut DurableHostRuntime) -> Result<(), String> {
        if self.pending_input.is_some() {
            runtime
                .presentation_host_mut()?
                .retire_selected_native_interaction(&mut self.pending_input)?;
        }
        self.current_input_route = None;
        Ok(())
    }

    pub(super) fn retire_input(&mut self, runtime: &mut DurableHostRuntime) -> Result<(), String> {
        self.finish_input(runtime)?;
        retire_frame(runtime)
    }

    pub(in crate::durable_host) fn input(
        &mut self,
        runtime: &mut DurableHostRuntime,
        output: &mut Scanout<'_>,
        event: conduit_linux_display::NativeInput,
    ) -> Result<(), String> {
        use conduit_linux_display::NativeInput;
        use conduit_std_host::SelectedNativeMaskInput;
        // Track physical button state even without a current owner or Show.
        // A held touch never becomes a new click when a replacement Show arrives.
        let activate = if let NativeInput::Pointer(sample) = &event {
            let rising = sample.primary_pressed && !self.primary_pressed;
            self.primary_pressed = sample.primary_pressed;
            rising
        } else {
            false
        };
        if self.pending_input.is_some() {
            return Ok(());
        }
        let (Some(owner), Some(route)) = (&mut self.owner, &self.current_input_route) else {
            return Ok(());
        };
        self.input_sequence = self
            .input_sequence
            .checked_add(1)
            .ok_or("native interaction sequence exhausted")?;
        let input = output
            .prepared_input()
            .map_err(|error| format!("native input lost:{error:?}"))?;
        let host = runtime.presentation_host_mut()?;
        let result = match event {
            NativeInput::Key(key) => host.selected_native_mask_key(
                owner,
                route,
                &input,
                key,
                self.input_sequence,
                output,
            )?,
            NativeInput::Pointer(sample) => host.selected_native_mask_pointer(
                owner,
                route,
                &input,
                sample,
                activate,
                self.input_sequence,
                output,
            )?,
            _ => return Ok(()),
        };
        if let SelectedNativeMaskInput::Submitted(interaction) = result {
            self.pending_input = Some(interaction);
            let pending = self
                .pending_input
                .as_ref()
                .ok_or("native input custody absent")?;
            let request = pending.correlation().interaction.clone();
            let outcome = match owner {
                NativeOwner::Member(owner) => {
                    owner.begin_action(pending.show(), request).map(|()| false)
                }
                NativeOwner::Joining(owner) => {
                    owner
                        .action(pending.show(), request)
                        .and_then(|(reply, _)| {
                            if reply["accepted"].as_bool() == Some(true) {
                                Ok(true)
                            } else {
                                Err(format!("native owner refused:{}", reply["code"]))
                            }
                        })
                }
            };
            match outcome {
                Ok(false) => (), // The same carrier continues presence while waiting.
                Ok(true) => {
                    self.finish_input(runtime)?;
                    self.next_refresh = Instant::now();
                }
                Err(error) => {
                    // The action may have reached its owner. Retire, never replay.
                    self.retire_input(runtime)?;
                    self.owner = None;
                    self.next_refresh =
                        Instant::now() + self.reconnect_delay.unwrap_or(REFRESH_PERIOD);
                    eprintln!("native action outcome unavailable; no replay:{error}");
                }
            }
        }
        Ok(())
    }
}
