//! Host work that a host library records into a title's command list in
//! place of a console one: AMD's PC FidelityFX upscaler in place of the
//! title's console one (`victory_hle::modules::fidelityfx`), and its frame
//! generation prepare (`crate::framegen`). The library is handed raw D3D12
//! pointers: the host list behind the title's, and each title texture's host
//! copy in the state it is told, prepared as a compute shader's inputs would
//! be.
//!
//! The shape is gleaf's fork (gleaf-6/VIctory-7900XTX-, `crates/victory-gpu/
//! src/external.rs`) and the input rules are trustno1's (trustno1-coder/
//! VIctory-NVIDIA, after DexNom's fork), both GPL-3.0-or-later. Everything
//! here runs on the title's thread while it records the list.

use std::ffi::c_void;

use windows::Win32::Graphics::Direct3D12::{ID3D12Device, ID3D12Resource};
use windows::core::Interface;

use crate::host::HostList;
use crate::object::{Object, This};

/// The host D3D12 device, created on first use; `None` without a host GPU.
pub fn host_device() -> Option<ID3D12Device> {
    crate::host::host().map(|h| h.device.clone())
}

/// [`host_device`] as the pointer a library's backend description takes;
/// null without a host GPU. The device lives as long as the process.
pub fn host_device_raw() -> *mut c_void {
    crate::host::host().map_or(std::ptr::null_mut(), |h| h.device.as_raw())
}

/// How a library uses a title texture, and the host resource state
/// (`D3D12_RESOURCE_STATES`) it wants it in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Use {
    /// Read: prepared as a compute shader's input first (alias, upload of
    /// title-written contents, depth twin).
    Input(u32),
    /// Written: its contents are on the host afterwards.
    Output(u32),
}

/// A title texture's host copy, as [`record`] hands it to the library.
#[derive(Clone, Copy, Debug)]
pub struct Texture {
    /// `ID3D12Resource*`, alive for the call.
    pub resource: *mut c_void,
    /// The state it is in: the one asked for without the bits the host copy
    /// cannot take (`host::legal_state`).
    pub state: u32,
    /// Its `DXGI_FORMAT`.
    pub format: i32,
}

/// Records a library's work at this place of the title list `list`: each
/// title texture of `slots` is found as its host copy, prepared and moved to
/// the state asked for, then `f` runs with the host command list
/// (`ID3D12GraphicsCommandList*`) and those copies. What `f` binds is put
/// back before the list's next draw or dispatch. `Err` without a host list
/// or a host copy (`f` did not run); else what `f` returned. Outputs count
/// as written by the host when `f` returns true.
pub fn record<const N: usize>(
    list: *const Object,
    slots: [Option<(*const Object, Use)>; N],
    f: impl FnOnce(*mut c_void, &[Option<Texture>; N]) -> bool,
) -> Result<bool, &'static str> {
    let this: This = list;
    if this.is_null() || crate::list::host_list(this).is_none() {
        return Err("the title's list records nowhere (no host list)");
    }
    let mut result = Err("the title's list records nowhere (no host list)");
    crate::list::with_state(this, |host, state| {
        // Alive for the call (the list holds them too).
        let mut held: Vec<ID3D12Resource> = Vec::with_capacity(N);
        let mut textures = [None; N];
        for (i, slot) in slots.iter().enumerate() {
            let Some((resource, use_)) = *slot else { continue };
            if resource.is_null() {
                continue;
            }
            let wanted = match use_ {
                Use::Input(state) => {
                    if prepare_input(host, resource).is_none() {
                        result = Err("an input has no host copy");
                        return;
                    }
                    state
                }
                Use::Output(state) => state,
            };
            let Some(texture) = crate::resource::host_resource(resource).cloned() else {
                result = Err("a texture has no host copy");
                return;
            };
            crate::list::ensure_state(host, resource, wanted);
            let legal = crate::host::legal_state(&texture, wanted);
            // SAFETY: a live resource.
            let format = unsafe { texture.GetDesc() }.Format.0;
            textures[i] = Some(Texture { resource: texture.as_raw(), state: legal, format });
            held.push(texture);
        }
        let done = f(host.list.as_raw(), &textures);
        if done {
            for (resource, use_) in slots.iter().flatten() {
                if matches!(use_, Use::Output(_)) && !resource.is_null() {
                    crate::texture::host_written(*resource);
                }
            }
        }
        state.replay(host);
        result = Ok(done);
    });
    result
}

/// A title texture's host copy, moved to the host resource `state` in
/// `list`, and the state it is in: `state` without the bits the host copy
/// cannot take (see `host::legal_state`). `None` for a buffer.
/// (`crate::streamline`.)
pub(crate) fn host_texture(list: &HostList, resource: This, state: u32) -> Option<(ID3D12Resource, u32)> {
    let texture = crate::resource::host_resource(resource)?.clone();
    crate::list::ensure_state(list, resource, state);
    let state = crate::host::legal_state(&texture, state);
    Some((texture, state))
}

/// The library wrote `resource` on the host: its contents are there, not in
/// title memory.
pub(crate) fn output_written(resource: This) {
    crate::texture::host_written(resource);
}

/// Sends what the title submitted (the line of submissions first) and
/// settles it, before a library lets go of resources the GPU may still use.
pub(crate) fn finish_submitted() {
    crate::queue::before_sync();
    crate::host::settle_for_fence();
}

/// Prepares a title-authored input with the alias, upload and depth twin
/// rules a compute shader's resource view has in the renderer.
pub(crate) fn prepare_input(list: &HostList, resource: This) -> Option<()> {
    let texture = crate::resource::host_resource(resource)?;
    let aliased = crate::resource::alias_of(resource)
        .is_some_and(|source| crate::texture::alias(list, resource, source));
    if !aliased && crate::texture::title_written(texture) {
        crate::texture::upload_from(list, resource, 0);
    }
    list.twin_use(resource, false, false);
    Some(())
}
