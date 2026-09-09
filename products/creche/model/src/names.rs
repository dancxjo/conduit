//! The Crèche's existing versioned persona catalog, shared with native Hosts.
//! Friendly suggestions are collision-tolerant metadata, never Body identity.
use alloc::{format, string::String, vec::Vec};
use serde::Deserialize;
use sha2::{Digest, Sha256};

const CATALOG: &str = include_str!("../../names/catalog.mjs");
pub const MAX_FRIENDLY_NAME_BYTES: usize = 64;

#[derive(Deserialize)]
pub struct NamingCatalog {
    pub version: u32,
    pub systems: Vec<NamingSystem>,
}
#[derive(Deserialize)]
pub struct NamingSystem {
    pub id: String,
    pub label: String,
    stocks: Vec<Stock>,
    forms: Vec<NameForm>,
}
#[derive(Deserialize)]
struct Stock {
    id: String,
    entries: Vec<String>,
}
#[derive(Deserialize)]
struct NameForm {
    id: String,
    slots: Vec<String>,
    pattern: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NameSuggestion {
    pub name: String,
    pub version: u32,
    pub system_id: String,
    pub system_label: String,
    pub form_id: String,
    pub variation: u32,
    pub stock_indexes: Vec<(String, usize)>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NamingRefusal {
    Catalog,
    Uuid,
    UnknownSystem,
    Bounds,
}

impl NamingCatalog {
    pub fn shared() -> Result<Self, NamingRefusal> {
        // The source of truth is one JSON literal, framed as a standard data
        // module for browser import. Native code does not interpret JavaScript.
        let json = CATALOG
            .strip_prefix("export default ")
            .and_then(|value| value.strip_suffix(";\n"))
            .ok_or(NamingRefusal::Catalog)?;
        serde_json::from_str(json).map_err(|_| NamingRefusal::Catalog)
    }

    pub fn name_for(
        &self,
        uuid: &str,
        requested: &str,
        variation: u32,
    ) -> Result<NameSuggestion, NamingRefusal> {
        if uuid.len() != 36
            || uuid.bytes().enumerate().any(|(index, byte)| {
                if [8, 13, 18, 23].contains(&index) {
                    byte != b'-'
                } else {
                    !byte.is_ascii_hexdigit()
                }
            })
        {
            return Err(NamingRefusal::Uuid);
        }
        let uuid = uuid.to_ascii_lowercase();
        if requested != "surprise" && !self.systems.iter().any(|system| system.id == requested) {
            return Err(NamingRefusal::UnknownSystem);
        }
        for counter in 0..16u32 {
            let seed = serde_json::to_vec(&(
                "conduit-creche-persona-v1",
                self.version,
                &uuid,
                requested,
                variation,
                counter,
            ))
            .map_err(|_| NamingRefusal::Catalog)?;
            let entropy = Sha256::digest(&seed);
            let mut word = 0;
            let mut next = |length: usize| -> Result<usize, NamingRefusal> {
                if length == 0 || length > u32::MAX as usize || word >= 8 {
                    return Err(NamingRefusal::Bounds);
                }
                let at = word * 4;
                word += 1;
                Ok(u32::from_be_bytes(
                    entropy[at..at + 4]
                        .try_into()
                        .map_err(|_| NamingRefusal::Bounds)?,
                ) as usize
                    % length)
            };
            let system = if requested == "surprise" {
                &self.systems[next(self.systems.len())?]
            } else {
                self.systems
                    .iter()
                    .find(|system| system.id == requested)
                    .ok_or(NamingRefusal::UnknownSystem)?
            };
            let sizes: Vec<usize> = system
                .forms
                .iter()
                .map(|form| {
                    form.slots.iter().try_fold(1usize, |total, slot| {
                        total
                            .checked_mul(system.stock(slot)?.entries.len())
                            .ok_or(NamingRefusal::Bounds)
                    })
                })
                .collect::<Result<_, _>>()?;
            let size = sizes.iter().try_fold(0usize, |total, size| {
                total.checked_add(*size).ok_or(NamingRefusal::Bounds)
            })?;
            let mut ticket = next(size)?;
            let index = sizes
                .iter()
                .position(|size| {
                    if ticket < *size {
                        true
                    } else {
                        ticket -= size;
                        false
                    }
                })
                .ok_or(NamingRefusal::Catalog)?;
            let form = &system.forms[index];
            let mut name = form.pattern.clone();
            let mut indexes = Vec::with_capacity(form.slots.len());
            for slot in &form.slots {
                let stock = system.stock(slot)?;
                let index = next(stock.entries.len())?;
                name = name.replace(&format!("{{{slot}}}"), &stock.entries[index]);
                indexes.push((slot.clone(), index));
            }
            if name.len() <= MAX_FRIENDLY_NAME_BYTES {
                return Ok(NameSuggestion {
                    name,
                    version: self.version,
                    system_id: system.id.clone(),
                    system_label: system.label.clone(),
                    form_id: form.id.clone(),
                    variation,
                    stock_indexes: indexes,
                });
            }
        }
        Err(NamingRefusal::Bounds)
    }
}
impl NamingSystem {
    fn stock(&self, id: &str) -> Result<&Stock, NamingRefusal> {
        self.stocks
            .iter()
            .find(|stock| stock.id == id)
            .ok_or(NamingRefusal::Catalog)
    }
}
