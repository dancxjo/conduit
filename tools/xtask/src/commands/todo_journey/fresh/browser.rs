//! Fresh browser capture and its exact-source retained owner checks.
use super::{capture, checkpoint_files, json_output, media};
use crate::cli::TodoJourneyArgs;
use serde_json::Value;
use std::{fs, path::Path, process::Command};

pub(super) struct BrowserInputs<'a> {
    pub args: &'a TodoJourneyArgs,
    pub repository: &'a Path,
    pub output: &'a Path,
    pub bin: &'a Path,
    pub state: &'a Path,
    pub handbook: &'a Path,
    pub playwright: &'a Path,
    pub item_text: &'a str,
    pub commit: &'a str,
    pub body_id: &'a str,
    pub selected: &'a Path,
}

pub(super) struct BrowserCapture {
    pub browser_command: Value,
    pub browser_receipt: Value,
    pub before_png: Value,
    pub after_png: Value,
    pub browser_receipt_file: Value,
    pub face_after: Value,
    pub checkpoint_after: Value,
}

pub(super) fn run(input: BrowserInputs<'_>) -> Result<BrowserCapture, String> {
    let BrowserInputs {
        args,
        repository,
        output,
        bin,
        state,
        handbook,
        playwright,
        item_text,
        commit,
        body_id,
        selected,
    } = input;
    let mut browser = Command::new("timeout");
    browser
        .args(["-k", "5s", "180s", "node"])
        .arg(repository.join("proof/browser/todo-owner-browser.mjs"))
        .arg(repository)
        .arg(bin)
        .arg(state)
        .arg(handbook)
        .arg(output.join("browser"))
        .arg(playwright)
        .arg(item_text)
        .arg(if args.mask_correspondence {
            "correspondence"
        } else if args.direct_speech {
            "cross-mask-speech"
        } else if args.cross_mask_actions {
            "cross-mask"
        } else {
            "first-add"
        })
        .current_dir(repository);
    let browser_command = capture(output, "browser-add", &mut browser, None)?;
    let browser_receipt: Value = serde_json::from_slice(
        &fs::read(output.join("browser/receipt.json")).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    if browser_receipt["schema"] != "conduit.proof/todo-owner-browser@1"
        || browser_receipt["source_relation"] != "exact-source"
        || browser_receipt["owner_source_commit"] != commit
        || browser_receipt["body_id"] != body_id
        || browser_receipt["item_text"] != item_text
        || browser_receipt["before"]["show_id"] == browser_receipt["after"]["show_id"]
    {
        return Err("browser Add receipt does not bind the fresh Body and exact source".into());
    }
    let before_png = media(output, "browser/browser-before.png")?;
    let after_png = media(output, "browser/browser-after.png")?;
    let browser_receipt_file = media(output, "browser/receipt.json")?;
    let mut after = Command::new("timeout");
    after
        .args(["-k", "5s", "15s"])
        .arg(bin)
        .args(["body", "face", "--state-dir"])
        .arg(state)
        .arg("--json");
    let face_after = json_output(output, "after-face", &mut after)?;
    if face_after["presentation"]["basis"]["body_id"] != body_id
        || !face_after["presentation"]["subjects"]
            .as_array()
            .is_some_and(|subjects| subjects.iter().any(|subject| subject["name"] == item_text))
    {
        return Err("installed Owner did not retain the browser-added Todo item".into());
    }
    let checkpoint_after = checkpoint_files(selected)?;
    if checkpoint_after["files"].as_u64().unwrap_or(0) == 0 {
        return Err("browser Add did not publish a selected Todo checkpoint".into());
    }
    Ok(BrowserCapture {
        browser_command,
        browser_receipt,
        before_png,
        after_png,
        browser_receipt_file,
        face_after,
        checkpoint_after,
    })
}
