//! Adapter/device acquisition for the GPU viewport backend (wave D, card UI-D1).
//!
//! [`acquire`] returns the device/queue the GPU backend renders on. The
//! acquisition order is:
//!
//! 1. **egui's device**: when the app runs `eframe` with the wgpu renderer,
//!    `main.rs` calls [`register_egui_device`] with the device eframe already
//!    created, so the viewport literally shares the same `wgpu::Device` the
//!    UI renders on (the wave-D "shared device with egui" line). No second
//!    device is created and the lockfile keeps a single `wgpu` version.
//! 2. **autonomous acquisition**: when no egui device was registered (the
//!    `oep-viewer` crate test, or an app that fell back to the glow
//!    renderer), this module creates its own `Instance`/`Adapter`/`Device`.
//! 3. **honest `None`**: if [`GPU_FLAG`] is `off` or no adapter is
//!    reachable, callers fall back to the CPU backend.
//!
//! Everything the backend needs from the device is offscreen and
//! color-target only, so the shared device is safe to use mid-frame: the
//! viewport submits its own command buffer into the same device wgpu
//! serializes internally, before eframe paints.

use std::future::Future;
use std::sync::{Mutex, OnceLock};
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

/// Environment flag that disables the GPU viewport path. Setting
/// `ISOLITH_GPU=off` (any case) forces the CPU backend and the glow
/// renderer; any other value or absence leaves the GPU enabled.
pub const GPU_FLAG: &str = "ISOLITH_GPU";

/// Device context a [`super::GpuBackend`] renders with.
#[derive(Clone)]
pub struct GpuContext {
    /// The device to create resources on and submit command buffers to.
    pub device: wgpu::Device,
    /// The queue for the device.
    pub queue: wgpu::Queue,
    /// Instance that created the autonomous device, kept alive for as long
    /// as the context so the adapter/device stay valid (`None` on the
    /// shared-with-egui path, where eframe keeps its own instance alive).
    _instance: Option<wgpu::Instance>,
}

impl GpuContext {
    /// An autonomous context (a device this module created itself).
    fn autonomous(device: wgpu::Device, queue: wgpu::Queue, instance: wgpu::Instance) -> Self {
        Self {
            device,
            queue,
            _instance: Some(instance),
        }
    }

    /// A context sharing the device an external owner (egui) already created.
    fn shared(device: wgpu::Device, queue: wgpu::Queue) -> Self {
        Self {
            device,
            queue,
            _instance: None,
        }
    }
}

/// The device eframe's wgpu renderer created, registered by `main.rs` so the
/// viewport shares it (set once per process; `None` until registered, or
/// forever when the app runs the glow renderer).
static EGUI_DEVICE: OnceLock<Mutex<Option<GpuContext>>> = OnceLock::new();

/// Register the device/queue eframe's wgpu renderer uses, making it the
/// preferred device for every [`GpuContext`] acquisition from here on.
pub fn register_egui_device(device: wgpu::Device, queue: wgpu::Queue) {
    let slot = EGUI_DEVICE.get_or_init(|| Mutex::new(None));
    let mut guard = slot.lock().expect("egui device registration lock");
    // First registration wins; an override would race the viewport's reads.
    if guard.is_none() {
        *guard = Some(GpuContext::shared(device, queue));
    }
}

/// Whether the GPU viewport path is enabled (reads [`GPU_FLAG`]; absent or
/// anything but `off` means enabled).
pub fn gpu_enabled() -> bool {
    !std::env::var(GPU_FLAG)
        .map(|v| v.eq_ignore_ascii_case("off"))
        .unwrap_or(false)
}

/// A lightweight "is a GPU adapter reachable" probe (instance + adapter
/// request, no device). Used by `main.rs` to pick the eframe renderer and to
/// name the expected viewport backend in the smoke line. The full decision
/// the viewport makes is [`gpu_enabled`] AND [`acquire`] succeeding, which
/// this predicts exactly: an adapter that responds to
/// [`Instance::request_adapter`] creates a device.
pub fn adapter_available() -> bool {
    if !gpu_enabled() {
        return false;
    }
    let instance = wgpu::Instance::default();
    block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).is_ok()
}

/// Acquire a [`GpuContext`]: the registered egui device if present, otherwise
/// a freshly created autonomous device. `None` when no adapter is reachable.
pub fn acquire() -> Option<GpuContext> {
    if let Some(slot) = EGUI_DEVICE.get() {
        if let Some(ctx) = slot.lock().expect("egui shared read lock").clone() {
            return Some(ctx);
        }
    }
    autonomous()
}

/// Create a standalone `Instance`/`Adapter`/`Device`.
fn autonomous() -> Option<GpuContext> {
    let instance = wgpu::Instance::default();
    let adapter =
        block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).ok()?;
    let (device, queue) =
        block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).ok()?;
    Some(GpuContext::autonomous(device, queue, instance))
}

/// Minimal synchronous driver for the wgpu async `request_adapter` /
/// `request_device` futures, std-only (no executor dependency on this card).
///
/// The wgpu-native device-creation futures resolve on the first polls without
/// registering for external wakeups, so a no-op waker plus a yield loop is
/// sufficient and terminates immediately. The unsafe block is the certified
/// `std::task::RawWaker` construction; it is confined to this one function.
fn block_on<F: Future>(future: F) -> F::Output {
    fn clone(_: *const ()) -> RawWaker {
        raw_waker()
    }
    fn wake(_: *const ()) {}
    fn wake_by_ref(_: *const ()) {}
    fn drop_fn(_: *const ()) {}
    fn raw_waker() -> RawWaker {
        static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake_by_ref, drop_fn);
        RawWaker::new(std::ptr::null(), &VTABLE)
    }
    let waker: Waker = unsafe { Waker::from_raw(raw_waker()) };
    let mut cx = Context::from_waker(&waker);
    let mut future = Box::pin(future);
    loop {
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(value) => return value,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpu_flag_off_disables_selection_and_restores() {
        // The env flag gates the whole selection path: with GPU=off, the
        // availability probe (what main.rs and select_backend consult) must
        // report no GPU without even probing the runtime. `acquire()` itself
        // is deliberately flag-agnostic - the gating lives in gpu_enabled()
        // and select_backend, so a caller that already asked for a context
        // gets one. The flag is restored so parallel tests stay unaffected.
        std::env::set_var(GPU_FLAG, "off");
        assert!(!gpu_enabled());
        assert!(!adapter_available());
        std::env::remove_var(GPU_FLAG);
        assert!(gpu_enabled());
    }
}
