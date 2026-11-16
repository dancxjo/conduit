#![no_std]
use conduit_kernel::dictionary_live_sign_archive::{DictionaryLiveSignLog,LivePrefixArchive};
macro_rules! row {($name:ident,$g:literal)=>{#[no_mangle] pub static $name:[u8; DictionaryLiveSignLog::<128,4,$g,3,2>::required_archive_backing_bytes()]=[0;DictionaryLiveSignLog::<128,4,$g,3,2>::required_archive_backing_bytes()];};}
row!(archive_charge_g48,48);row!(archive_charge_g47,47);row!(archive_charge_g46,46);row!(archive_charge_g45,45);row!(archive_charge_g44,44);row!(archive_charge_g42,42);row!(archive_charge_g40,40);
#[no_mangle]pub static inline_archive_g48:[u8;core::mem::size_of::<LivePrefixArchive<128,4,48,3,2>>()]=[0;core::mem::size_of::<LivePrefixArchive<128,4,48,3,2>>()];
#[no_mangle]pub static wrapper:[u8;core::mem::size_of::<DictionaryLiveSignLog<128,4,48,3,2>>()]=[0;core::mem::size_of::<DictionaryLiveSignLog<128,4,48,3,2>>()];
extern crate alloc;
#[no_mangle]pub static active_sign_backing:[u8;conduit_kernel::HostedSignLog::canonical_storage_bytes(61,6).unwrap()]=[0;conduit_kernel::HostedSignLog::canonical_storage_bytes(61,6).unwrap()];
#[no_mangle]pub static transfer_token_g44:[u8;core::mem::size_of::<conduit_kernel::live_sign_archive::PreparedPrefixTransfer<'static,128,44,2,alloc::boxed::Box<LivePrefixArchive<128,4,44,3,2>>>>()]=[0;core::mem::size_of::<conduit_kernel::live_sign_archive::PreparedPrefixTransfer<'static,128,44,2,alloc::boxed::Box<LivePrefixArchive<128,4,44,3,2>>>>()];
