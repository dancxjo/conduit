//! Finite current software-input + actual native raster + linear kernel proof.
//! Request acceptance/source state are fixtures; no scanout or physical HID claim.
use super::tests::{accept, prepared_keys, Pending};
use super::*;
use crate::terminal_face_mask::{
    TerminalError, TerminalInput, TerminalInputOutcome, TerminalMaskExecution,
};
use crate::terminal_mask_execution::HostedVisualMaskExecution;
use conduitos::{
    display::{DisplayError, DisplayFormat, PixelTarget, RetainedPixelTarget},
    native_face_scene::{FaceControl, NativeFaceScene},
};
use sha2::{Digest, Sha256};

struct Pixels(Vec<u32>);
impl PixelTarget for Pixels {
    fn format(&self) -> DisplayFormat {
        DisplayFormat {
            width: 640,
            height: 480,
            pitch: 2560,
            bits_per_pixel: 32,
            red_shift: 16,
            green_shift: 8,
            blue_shift: 0,
        }
    }
    fn write_pixel(&mut self, x: u32, y: u32, value: u32) -> Result<(), DisplayError> {
        self.0[(y * 640 + x) as usize] = value;
        Ok(())
    }
}
impl RetainedPixelTarget for Pixels {
    fn read_pixel(&self, x: u32, y: u32) -> Result<u32, DisplayError> {
        Ok(self.0[(y * 640 + x) as usize])
    }
}
#[derive(Default)]
struct Raster {
    scene: Option<NativeFaceScene>,
    wording: Option<String>,
    digest: Option<[u8; 32]>,
    frames: usize,
}
impl Raster {
    fn paint(&mut self, face: &Face, wording: Option<&str>) -> Result<Vec<u8>, TerminalError> {
        let mut scene = NativeFaceScene::prepare(face.clone(), 640, 480)
            .map_err(|e| TerminalError::Execution(format!("native scene: {e:?}")))?;
        if let Some(wording) = wording {
            scene
                .set_local_notice(wording)
                .map_err(|e| TerminalError::Execution(format!("native instruction: {e:?}")))?;
        }
        let frame = scene
            .frame()
            .map_err(|e| TerminalError::Execution(format!("native frame: {e:?}")))?;
        if let Some(wording) = wording {
            assert!(frame
                .scene
                .commands()
                .iter()
                .any(|command| command.payload() == wording));
        }
        assert!(frame
            .hits
            .iter()
            .any(|hit| hit.action.identity == "game/start"));
        let mut pixels = Pixels(vec![0; 640 * 480]);
        conduitos::display::typography::render_scene(&mut pixels, &frame.scene, |_, command| {
            command.text_role().into()
        })
        .map_err(|e| TerminalError::Execution(format!("raster effect: {e:?}")))?;
        assert!(pixels.0.windows(2).any(|pair| pair[0] != pair[1]));
        let bytes: Vec<u8> = pixels.0.into_iter().flat_map(u32::to_le_bytes).collect();
        self.digest = Some(Sha256::digest(&bytes).into());
        self.wording = wording.map(str::to_owned);
        self.scene = Some(scene);
        self.frames += 1;
        Ok(bytes)
    }
}

#[test]
fn action_prompt_i1_visual_and_linear_current_start_return_same_typed_action() {
    for keyboard_only in [false, true] {
        let (host, plan, play, identity, _, _) = super::super::tests::fixture_with_visual(true);
        let host = host.advertisement();
        let mut map = ParsecAcknowledgedMap::prepare(&plan, host, &play, &identity).unwrap();
        map.confirm(host, &play).unwrap();
        let mut owner = ParsecTerminalOwner::prepare(&plan, host, 80, 24).unwrap();
        let mut output = Vec::new();
        owner.ensure_present(&map, host, &mut output).unwrap();
        let mut adapter = Pending;
        let (mut keys, requests) = prepared_keys(&mut adapter, host, &plan, &play);
        if keyboard_only {
            let (request, kind) = requests
                .iter()
                .find(|(_, kind)| {
                    matches!(
                        kind,
                        crate::installed_std::keyboard_input_host::InputRequestKind::Keyboard
                    )
                })
                .unwrap();
            keys.accept(*request, &[], *kind).unwrap();
        } else {
            accept(&mut keys, &requests);
        }
        owner.sync_prompt(&map, host, &keys, &mut output).unwrap();
        let linear = owner
            .instruction_observation(&map, host, &keys)
            .unwrap()
            .unwrap();
        let face = linear.face().clone();
        let linear_show = linear.show().clone();
        let label = if keyboard_only { "Enter" } else { "Space" };
        assert!(linear
            .prompt_text()
            .contains(&format!("Input: {label} to Begin.")));
        let mut visual = HostedVisualMaskExecution::new_attached(host).unwrap();
        visual.bind_parsec_body_scope(&plan, &play, 4).unwrap();
        let mut raster = Raster::default();
        let shown = visual
            .present(&face, |face, wording| raster.paint(face, wording))
            .unwrap();
        assert_ne!(shown.show_id, linear_show.show_id);
        assert_ne!(
            visual.planned_mask().plan.plan_id,
            linear_show.planned_mask.plan.plan_id
        );
        assert_eq!(visual.frame_digest(), raster.digest);
        let initial_digest = raster.digest;
        let witness = owner
            .qualify_visual_instruction(&map, host, &keys, &visual)
            .unwrap()
            .unwrap();
        let binding = ActionInputBindingProposalInfo::decode(witness.binding_bytes()).unwrap();
        assert_eq!(
            binding.control_identity(),
            linear.binding().control_identity()
        );
        assert_eq!(binding.control_label(), linear.binding().control_label());
        assert_eq!(
            binding.mapping_revision(),
            linear.binding().mapping_revision()
        );
        assert_eq!(binding.offered_action(), linear.binding().offered_action());
        assert_eq!(binding.show_identity(), shown.show_id.as_str());
        let mut forged = owner
            .qualify_visual_instruction(&map, host, &keys, &visual)
            .unwrap()
            .unwrap();
        let mut forged_binding = serde_json::to_value(binding.clone()).unwrap();
        forged_binding["control_label"] = serde_json::json!("Fire");
        let forged_binding: ActionInputBindingProposalInfo =
            serde_json::from_value(forged_binding).unwrap();
        forged.binding = forged_binding.encode().unwrap();
        assert!(owner
            .revalidate_visual_instruction(forged, &map, host, &keys, &visual)
            .is_err());
        let mut stale_show = owner
            .qualify_visual_instruction(&map, host, &keys, &visual)
            .unwrap()
            .unwrap();
        stale_show.show.show.play_sequence += 1;
        assert!(owner
            .revalidate_visual_instruction(stale_show, &map, host, &keys, &visual)
            .is_err());
        assert_eq!(
            raster.frames, 1,
            "forged Fire and stale Show refuse before raster effect"
        );
        let witness = owner
            .revalidate_visual_instruction(witness, &map, host, &keys, &visual)
            .unwrap();
        let effect = visual
            .execute_instruction(witness, host, |face, wording| raster.paint(face, wording))
            .unwrap();
        assert_eq!(
            raster.wording.as_deref(),
            Some(format!("Use {label} to Begin.").as_str())
        );
        assert_ne!(raster.digest, initial_digest);
        assert_eq!(visual.frame_digest(), raster.digest);
        assert_eq!(raster.frames, 2);
        let frame =
            conduit_core::StructuredInfoValue::from_canonical_bytes(effect.canonical_instruction())
                .unwrap();
        let mut inspect =
            conduit_face::PreparedParsecInstructionFrameInspection::prepare().unwrap();
        let inspected = inspect.inspect(effect.canonical_instruction()).unwrap();
        assert_eq!(inspected.wording(), raster.wording.as_deref().unwrap());
        assert_eq!(
            frame.canonical_bytes().unwrap(),
            effect.canonical_instruction()
        );
        let raw_visual = raster
            .scene
            .as_ref()
            .unwrap()
            .interaction(
                &face.identity,
                face.revision,
                FaceControl {
                    action: 0,
                    argument: None,
                },
                &shown,
                vec![],
                42,
            )
            .unwrap();
        let TerminalInputOutcome::Interaction(raw_linear) = owner
            .terminal
            .as_mut()
            .unwrap()
            .input(TerminalInput::Apply, &linear_show, 42)
            .unwrap()
        else {
            panic!("typed linear interaction");
        };
        let visual_return = visual.interact(raw_visual).unwrap();
        let linear_return = owner.execution.interact(raw_linear).unwrap();
        assert_eq!(visual_return.interaction.action_id, "game/start");
        assert_eq!(
            visual_return.interaction.action_id,
            linear_return.interaction.action_id
        );
        assert_eq!(
            visual_return.interaction.target,
            linear_return.interaction.target
        );
        assert_eq!(
            visual_return.interaction.arguments,
            linear_return.interaction.arguments
        );
        assert_eq!(
            visual_return.interaction.face_id,
            linear_return.interaction.face_id
        );
        assert_eq!(
            visual_return.interaction.face_revision,
            linear_return.interaction.face_revision
        );
        assert_eq!(
            visual_return.interaction.sequence,
            linear_return.interaction.sequence
        );
        assert_eq!(visual_return.interaction.show_id, shown.show_id.as_str());
        assert_eq!(
            linear_return.interaction.show_id,
            linear_show.show_id.as_str()
        );
        assert!(visual.current_show().is_none());
    }
}

#[test]
fn action_prompt_i1_visual_cancel_refuses_current_witness_before_raster_effect() {
    let (host, plan, play, identity, _, _) = super::super::tests::fixture_with_visual(true);
    let host = host.advertisement();
    let mut map = ParsecAcknowledgedMap::prepare(&plan, host, &play, &identity).unwrap();
    map.confirm(host, &play).unwrap();
    let mut owner = ParsecTerminalOwner::prepare(&plan, host, 80, 24).unwrap();
    owner.ensure_present(&map, host, &mut Vec::new()).unwrap();
    let mut adapter = Pending;
    let (mut keys, requests) = prepared_keys(&mut adapter, host, &plan, &play);
    accept(&mut keys, &requests);
    let face = owner.current_face_show(&map, host).unwrap().0.clone();
    let mut visual = HostedVisualMaskExecution::new_attached(host).unwrap();
    visual.bind_parsec_body_scope(&plan, &play, 4).unwrap();
    let mut raster = Raster::default();
    visual
        .present(&face, |face, wording| raster.paint(face, wording))
        .unwrap();
    let witness = owner
        .qualify_visual_instruction(&map, host, &keys, &visual)
        .unwrap()
        .unwrap();
    let frames = raster.frames;
    keys.cancel();
    assert!(owner
        .revalidate_visual_instruction(witness, &map, host, &keys, &visual)
        .is_err());
    assert_eq!(raster.frames, frames);
    visual.cancel().unwrap();
    assert!(owner
        .qualify_visual_instruction(&map, host, &keys, &visual)
        .unwrap()
        .is_none());
}

#[test]
fn action_prompt_i1_visual_stale_map_refuses_before_supplement_effect() {
    let (host, plan, play, identity, port, node) = super::super::tests::fixture_with_visual(true);
    let host = host.advertisement();
    let mut map = ParsecAcknowledgedMap::prepare(&plan, host, &play, &identity).unwrap();
    map.confirm(host, &play).unwrap();
    let mut owner = ParsecTerminalOwner::prepare(&plan, host, 80, 24).unwrap();
    owner.ensure_present(&map, host, &mut Vec::new()).unwrap();
    let mut adapter = Pending;
    let (mut keys, requests) = prepared_keys(&mut adapter, host, &plan, &play);
    accept(&mut keys, &requests);
    let face = owner.current_face_show(&map, host).unwrap().0.clone();
    let mut visual = HostedVisualMaskExecution::new_attached(host).unwrap();
    visual.bind_parsec_body_scope(&plan, &play, 4).unwrap();
    let mut raster = Raster::default();
    visual
        .present(&face, |face, wording| raster.paint(face, wording))
        .unwrap();
    let witness = owner
        .qualify_visual_instruction(&map, host, &keys, &visual)
        .unwrap()
        .unwrap();
    let sources = crate::flow_activation::ParsecSourcePlans::prepare(host).unwrap();
    let mut evaluator = sources.prepare_evaluator().unwrap();
    let command = conduit_face::ParsecMapCommand::new("game/remap".into(), 1, true)
        .unwrap()
        .encode()
        .unwrap();
    let result = evaluator
        .evaluate(map.current_state().unwrap(), &command)
        .unwrap()
        .result_bytes()
        .to_vec();
    map.acknowledge_lowered(&port, node, 0, &result).unwrap();
    assert!(owner
        .revalidate_visual_instruction(witness, &map, host, &keys, &visual)
        .is_err());
    assert_eq!(raster.frames, 1);
    visual.cancel().unwrap();
}
