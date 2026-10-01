//! Native codecs for authored or externally proposed navigation goals and time.

use crate::navigation_codec_support::{decode_native, encode_native};
use crate::{NavigationCodecError, NavigationGoal, NavigationTime};
use alloc::vec::Vec;

pub fn decode_navigation_time(encoded: &[u8]) -> Result<NavigationTime, NavigationCodecError> {
    decode_native(encoded)
}
pub fn decode_navigation_goal(encoded: &[u8]) -> Result<NavigationGoal, NavigationCodecError> {
    decode_native(encoded)
}
pub fn encode_navigation_time(value: &NavigationTime) -> Result<Vec<u8>, NavigationCodecError> {
    encode_native(value)
}
pub fn encode_navigation_goal(value: &NavigationGoal) -> Result<Vec<u8>, NavigationCodecError> {
    encode_native(value)
}
