//! iPhone and iPad mirroring over USB, view only: macOS exposes a connected iOS device's screen and
//! sound as a muxed AVFoundation capture device (the one QuickTime records from) once CoreMediaIO
//! is told to allow screen capture devices. Frames arrive decoded, so they go straight to the slot.

use crate::audio::{self, RecSink, Ring};
use crate::stream::{Slot, UserEvent};
use dispatch2::DispatchQueue;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{AllocAnyThread, ClassType, DefinedClass, define_class, msg_send};
use objc2_av_foundation::{
    AVAuthorizationStatus, AVCaptureAudioDataOutput, AVCaptureAudioDataOutputSampleBufferDelegate, AVCaptureConnection,
    AVCaptureDevice, AVCaptureDeviceDiscoverySession, AVCaptureDeviceInput, AVCaptureDevicePosition,
    AVCaptureDeviceTypeExternal, AVCaptureOutput, AVCaptureSession, AVCaptureVideoDataOutput,
    AVCaptureVideoDataOutputSampleBufferDelegate, AVMediaType, AVMediaTypeAudio, AVMediaTypeMuxed, AVMediaTypeVideo,
};
use objc2_core_media::{CMAudioFormatDescriptionGetStreamBasicDescription, CMSampleBuffer};
use objc2_core_media_io::{
    CMIOObjectPropertyAddress, CMIOObjectSetPropertyData, kCMIOHardwarePropertyAllowScreenCaptureDevices,
    kCMIOObjectPropertyElementMain, kCMIOObjectPropertyScopeGlobal, kCMIOObjectSystemObject,
};
use objc2_core_video::{
    CVPixelBufferGetBaseAddressOfPlane, CVPixelBufferGetBytesPerRowOfPlane, CVPixelBufferGetHeightOfPlane,
    CVPixelBufferGetPixelFormatType, CVPixelBufferGetPlaneCount, CVPixelBufferGetWidthOfPlane,
    CVPixelBufferLockBaseAddress, CVPixelBufferLockFlags, CVPixelBufferUnlockBaseAddress,
    kCVPixelBufferPixelFormatTypeKey, kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange,
};
use objc2_foundation::{NSArray, NSDate, NSDictionary, NSNumber, NSRunLoop, NSString};
use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use winit::event_loop::EventLoopProxy;

/// A connected iOS device that can be mirrored.
pub struct Device {
    pub uid: String,
    pub name: String,
}

/// iOS devices only show up as capture devices after this, and then only a moment later.
fn allow_screen_capture_devices() {
    let address = CMIOObjectPropertyAddress {
        mSelector: kCMIOHardwarePropertyAllowScreenCaptureDevices,
        mScope: kCMIOObjectPropertyScopeGlobal,
        mElement: kCMIOObjectPropertyElementMain,
    };
    let allow: u32 = 1;
    let status = unsafe {
        CMIOObjectSetPropertyData(
            kCMIOObjectSystemObject,
            &address,
            0,
            std::ptr::null(),
            size_of::<u32>() as u32,
            (&allow as *const u32).cast(),
        )
    };
    if status != 0 {
        eprintln!("could not allow screen capture devices (CoreMediaIO status {status})");
    }
}

fn list() -> Vec<Device> {
    let Some(muxed) = (unsafe { AVMediaTypeMuxed }) else { return Vec::new() };
    let types = NSArray::from_slice(&[unsafe { AVCaptureDeviceTypeExternal }]);
    let session = unsafe {
        AVCaptureDeviceDiscoverySession::discoverySessionWithDeviceTypes_mediaType_position(
            &types,
            None,
            AVCaptureDevicePosition::Unspecified,
        )
    };
    unsafe { session.devices() }
        .iter()
        .filter(|d| unsafe { d.hasMediaType(muxed) })
        .map(|d| Device { uid: unsafe { d.uniqueID() }.to_string(), name: unsafe { d.localizedName() }.to_string() })
        .collect()
}

/// The iOS devices connected over USB, waiting up to `wait` for the first to appear. Runs the
/// current thread's run loop while it waits, so call it from the main thread.
pub fn devices(wait: Duration) -> Vec<Device> {
    allow_screen_capture_devices();
    let until = Instant::now() + wait;
    loop {
        let found = list();
        if !found.is_empty() || Instant::now() >= until {
            return found;
        }
        NSRunLoop::currentRunLoop().runUntilDate(&NSDate::dateWithTimeIntervalSinceNow(0.25));
    }
}

/// Without camera (and, for sound, microphone) access the capture runs but delivers nothing, so ask
/// first. The first time, macOS shows its prompt and this waits for the answer.
fn ensure_access(media: Option<&AVMediaType>, what: &str) -> Result<(), String> {
    let Some(media) = media else { return Ok(()) };
    let denied = || {
        format!(
            "Handset needs {what} access to mirror an iPhone. Allow it in System Settings > Privacy & Security > \
             {what}, then start again."
        )
    };
    let granted = match unsafe { AVCaptureDevice::authorizationStatusForMediaType(media) } {
        AVAuthorizationStatus::Authorized => true,
        AVAuthorizationStatus::NotDetermined => {
            let (tx, rx) = std::sync::mpsc::channel();
            let handler = block2::RcBlock::new(move |ok: objc2::runtime::Bool| {
                let _ = tx.send(ok.as_bool());
            });
            unsafe { AVCaptureDevice::requestAccessForMediaType_completionHandler(media, &handler) };
            rx.recv().unwrap_or(false)
        }
        _ => false,
    };
    if granted { Ok(()) } else { Err(denied()) }
}

/// iOS only sends audio while something plays, but the recorder needs a steady stream (ffmpeg's
/// `-shortest` would otherwise cut the video at the end of the sound), so gaps are filled with silence.
struct Pacer {
    start: Instant,
    /// Samples (not frames) handed to the recorder since `start`, real or silent.
    sent: AtomicU64,
}

/// How far the recorder's audio may fall behind the clock before silence fills the gap.
const SLACK: Duration = Duration::from_millis(100);
const SAMPLES_PER_SEC: u64 = audio::SAMPLE_RATE as u64 * audio::CHANNELS as u64;

impl Pacer {
    fn fill(&self, rec: &RecSink) {
        let due = self.start.elapsed().saturating_sub(SLACK).as_micros() as u64 * SAMPLES_PER_SEC / 1_000_000;
        let sent = self.sent.load(Relaxed);
        if due > sent {
            // whole frames only, so the channels stay in order
            let missing = (due - sent) & !1;
            self.sent.fetch_add(missing, Relaxed);
            if let Some(tx) = rec.lock().unwrap().as_ref() {
                let _ = tx.send(vec![0u8; missing as usize * 2]);
            }
        }
    }
}

struct Sinks {
    slot: Arc<Mutex<Slot>>,
    proxy: Mutex<EventLoopProxy<UserEvent>>,
    ring: Option<Ring>,
    rec: RecSink,
    logged: AtomicBool,
    /// The last audio format seen, so a change is logged once.
    audio_format: AtomicU64,
    seen_video: AtomicBool,
    pacer: Arc<Pacer>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = AllocAnyThread]
    #[name = "HandsetIosCapture"]
    #[ivars = Sinks]
    struct Delegate;

    unsafe impl NSObjectProtocol for Delegate {}

    // Video and audio share the selector; the output's class says which this is.
    unsafe impl AVCaptureVideoDataOutputSampleBufferDelegate for Delegate {
        #[unsafe(method(captureOutput:didOutputSampleBuffer:fromConnection:))]
        fn did_output(&self, output: &AVCaptureOutput, buffer: &CMSampleBuffer, _connection: &AVCaptureConnection) {
            if output.isKindOfClass(AVCaptureVideoDataOutput::class()) {
                self.video(buffer);
            } else {
                self.audio(buffer);
            }
        }
    }

    unsafe impl AVCaptureAudioDataOutputSampleBufferDelegate for Delegate {}
);

impl Delegate {
    fn new(sinks: Sinks) -> Retained<Self> {
        let this = Self::alloc().set_ivars(sinks);
        unsafe { msg_send![super(this), init] }
    }

    fn video(&self, buffer: &CMSampleBuffer) {
        let Some(image) = (unsafe { buffer.image_buffer() }) else { return };
        let s = &self.ivars();
        if !s.seen_video.swap(true, Relaxed) {
            eprintln!(
                "iOS video: {}x{}, format {:08x}",
                CVPixelBufferGetWidthOfPlane(&image, 0),
                CVPixelBufferGetHeightOfPlane(&image, 0),
                CVPixelBufferGetPixelFormatType(&image)
            );
        }
        if CVPixelBufferGetPixelFormatType(&image) != kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange
            || CVPixelBufferGetPlaneCount(&image) != 2
        {
            if !s.logged.swap(true, Relaxed) {
                eprintln!("iOS video: unexpected pixel format {:08x}", CVPixelBufferGetPixelFormatType(&image));
            }
            return;
        }
        if unsafe { CVPixelBufferLockBaseAddress(&image, CVPixelBufferLockFlags::ReadOnly) } != 0 {
            return;
        }
        let (w, h) = (CVPixelBufferGetWidthOfPlane(&image, 0), CVPixelBufferGetHeightOfPlane(&image, 0));
        let (cw, ch) = (CVPixelBufferGetWidthOfPlane(&image, 1), CVPixelBufferGetHeightOfPlane(&image, 1));
        let (ys, cs) = (CVPixelBufferGetBytesPerRowOfPlane(&image, 0), CVPixelBufferGetBytesPerRowOfPlane(&image, 1));
        let (yp, cp) = (CVPixelBufferGetBaseAddressOfPlane(&image, 0), CVPixelBufferGetBaseAddressOfPlane(&image, 1));
        let mut slot = s.slot.lock().unwrap();
        let was_fresh = slot.fresh;
        if !yp.is_null() && !cp.is_null() {
            let luma = unsafe { std::slice::from_raw_parts(yp as *const u8, ys * h) };
            let chroma = unsafe { std::slice::from_raw_parts(cp as *const u8, cs * ch) };
            let yuv = &mut slot.yuv;
            yuv.width = w as u32;
            yuv.height = h as u32;
            yuv.stride_y = ys as u32;
            yuv.stride_uv = cw as u32;
            yuv.y.clear();
            yuv.y.extend_from_slice(luma);
            // the renderer takes I420: split the interleaved CbCr plane
            yuv.u.clear();
            yuv.v.clear();
            for row in chroma.chunks_exact(cs) {
                for pair in row[..cw * 2].as_chunks::<2>().0 {
                    yuv.u.push(pair[0]);
                    yuv.v.push(pair[1]);
                }
            }
            slot.black = crate::black::is_black(luma);
            slot.fresh = true;
        }
        drop(slot);
        unsafe { CVPixelBufferUnlockBaseAddress(&image, CVPixelBufferLockFlags::ReadOnly) };
        if !was_fresh {
            let _ = s.proxy.lock().unwrap().send_event(UserEvent::Frame);
        }
    }

    fn audio(&self, buffer: &CMSampleBuffer) {
        let s = &self.ivars();
        let Some(desc) = (unsafe { buffer.format_description() }) else { return };
        let Some(a) = (unsafe { CMAudioFormatDescriptionGetStreamBasicDescription(&desc).as_ref() }) else { return };
        let format = Pcm {
            float: a.mFormatFlags & FLAG_FLOAT != 0,
            bits: a.mBitsPerChannel,
            channels: a.mChannelsPerFrame as usize,
            planar: a.mFormatFlags & FLAG_NON_INTERLEAVED != 0,
        };
        // the output settings are not always honoured, so say what actually arrives when it changes
        let key = (a.mSampleRate as u64) << 32 | (a.mFormatFlags as u64) << 8 | a.mBitsPerChannel as u64;
        if s.audio_format.swap(key, Relaxed) != key {
            eprintln!(
                "iOS audio: {} Hz, {} channels, {} bits, flags {:x}",
                a.mSampleRate, a.mChannelsPerFrame, a.mBitsPerChannel, a.mFormatFlags
            );
            if a.mSampleRate as u32 != audio::SAMPLE_RATE {
                eprintln!("iOS audio is not {} Hz: it will play at the wrong speed", audio::SAMPLE_RATE);
            }
        }
        let Some(data) = (unsafe { buffer.data_buffer() }) else { return };
        let len = unsafe { data.data_length() };
        let mut raw = vec![0u8; len];
        let Some(dest) = NonNull::new(raw.as_mut_ptr().cast()) else { return };
        if unsafe { data.copy_data_bytes(0, len, dest) } != 0 {
            return;
        }
        let Some(pcm) = format.to_s16_stereo(&raw, unsafe { buffer.num_samples() } as usize) else { return };
        s.pacer.sent.fetch_add(pcm.len() as u64 / 2, Relaxed);
        if let Some(r) = &s.ring {
            audio::push_samples(r, &pcm);
        }
        if let Some(tx) = s.rec.lock().unwrap().as_ref() {
            let _ = tx.send(pcm);
        }
    }
}

const FLAG_FLOAT: u32 = 0x1;
const FLAG_NON_INTERLEAVED: u32 = 0x20;

/// The layout of the linear PCM a capture delivers.
struct Pcm {
    float: bool,
    bits: u32,
    channels: usize,
    planar: bool,
}

impl Pcm {
    /// Convert `frames` frames to interleaved 16-bit stereo, what playback and the recorder take. A
    /// planar buffer holds each channel's frames one after the other. `None` for a layout not handled.
    fn to_s16_stereo(&self, raw: &[u8], frames: usize) -> Option<Vec<u8>> {
        let width = match (self.float, self.bits) {
            (false, 16) => 2,
            (true, 32) => 4,
            _ => return None,
        };
        if self.channels == 0 || raw.len() < frames * self.channels * width {
            return None;
        }
        let sample = |frame: usize, ch: usize| -> i16 {
            let ch = ch.min(self.channels - 1); // mono plays on both sides
            let i = if self.planar { ch * frames + frame } else { frame * self.channels + ch } * width;
            if self.float {
                let v = f32::from_le_bytes(raw[i..i + 4].try_into().unwrap());
                (v.clamp(-1.0, 1.0) * 32767.0) as i16
            } else {
                i16::from_le_bytes([raw[i], raw[i + 1]])
            }
        };
        let mut out = Vec::with_capacity(frames * 4);
        for f in 0..frames {
            out.extend_from_slice(&sample(f, 0).to_le_bytes());
            out.extend_from_slice(&sample(f, 1).to_le_bytes());
        }
        Some(out)
    }
}

/// A running capture. Dropping it stops the session and the disconnect watch.
pub struct Capture {
    session: Retained<AVCaptureSession>,
    pub name: String,
    pub uid: String,
    pub audio: bool,
    stop: Arc<AtomicBool>,
    _delegate: Retained<Delegate>,
}

impl Drop for Capture {
    fn drop(&mut self) {
        self.stop.store(true, Relaxed);
        unsafe { self.session.stopRunning() };
    }
}

fn number_dict(pairs: &[(&NSString, Retained<NSNumber>)]) -> Retained<NSDictionary<NSString, AnyObject>> {
    let keys: Vec<&NSString> = pairs.iter().map(|(k, _)| *k).collect();
    let values: Vec<&AnyObject> = pairs.iter().map(|(_, v)| -> &AnyObject { v }).collect();
    NSDictionary::from_slices(&keys, &values)
}

impl Capture {
    pub fn start(
        device: &Device,
        with_audio: bool,
        ring: Option<Ring>,
        rec: RecSink,
        slot: Arc<Mutex<Slot>>,
        proxy: EventLoopProxy<UserEvent>,
    ) -> Result<Capture, String> {
        ensure_access(unsafe { AVMediaTypeVideo }, "Camera")?;
        let with_audio = with_audio
            && ensure_access(unsafe { AVMediaTypeAudio }, "Microphone").map_err(|e| eprintln!("{e}")).is_ok();
        let av = unsafe { AVCaptureDevice::deviceWithUniqueID(&NSString::from_str(&device.uid)) }
            .ok_or_else(|| format!("{} is no longer connected", device.name))?;
        let input = unsafe { AVCaptureDeviceInput::deviceInputWithDevice_error(&av) }
            .map_err(|e| format!("Cannot open {}: {}", device.name, e.localizedDescription()))?;
        let session = unsafe { AVCaptureSession::new() };
        unsafe { session.beginConfiguration() };
        if !unsafe { session.canAddInput(&input) } {
            return Err(format!("Cannot capture from {}", device.name));
        }
        unsafe { session.addInput(&input) };

        let pacer = Arc::new(Pacer { start: Instant::now(), sent: AtomicU64::new(0) });
        let rec_fill = rec.clone();
        let delegate = Delegate::new(Sinks {
            slot,
            proxy: Mutex::new(proxy.clone()),
            ring,
            rec,
            logged: AtomicBool::new(false),
            audio_format: AtomicU64::new(0),
            seen_video: AtomicBool::new(false),
            pacer: pacer.clone(),
        });
        let video = unsafe { AVCaptureVideoDataOutput::new() };
        // CFString and NSString are toll-free bridged
        let format_key: &NSString = unsafe { &*(kCVPixelBufferPixelFormatTypeKey as *const _ as *const NSString) };
        let settings = number_dict(&[(format_key, NSNumber::new_u32(kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange))]);
        unsafe {
            video.setVideoSettings(Some(&settings));
            video.setAlwaysDiscardsLateVideoFrames(true);
            let queue = DispatchQueue::new("handset.ios.video", None);
            video.setSampleBufferDelegate_queue(Some(ProtocolObject::from_ref(&*delegate)), Some(&queue));
        }
        if !unsafe { session.canAddOutput(&video) } {
            return Err(format!("Cannot read video from {}", device.name));
        }
        unsafe { session.addOutput(&video) };

        let mut audio = false;
        if with_audio {
            let out = unsafe { AVCaptureAudioDataOutput::new() };
            // the same PCM the Android server sends, so playback and recording need no changes
            let lpcm = u32::from_be_bytes(*b"lpcm");
            let s = |k: &str| NSString::from_str(k);
            let (id, rate, channels, bits, float, big, planar) = (
                s("AVFormatIDKey"),
                s("AVSampleRateKey"),
                s("AVNumberOfChannelsKey"),
                s("AVLinearPCMBitDepthKey"),
                s("AVLinearPCMIsFloatKey"),
                s("AVLinearPCMIsBigEndianKey"),
                s("AVLinearPCMIsNonInterleaved"),
            );
            let settings = number_dict(&[
                (&id, NSNumber::new_u32(lpcm)),
                (&rate, NSNumber::new_f64(audio::SAMPLE_RATE as f64)),
                (&channels, NSNumber::new_u32(audio::CHANNELS as u32)),
                (&bits, NSNumber::new_u32(16)),
                (&float, NSNumber::new_bool(false)),
                (&big, NSNumber::new_bool(false)),
                (&planar, NSNumber::new_bool(false)),
            ]);
            unsafe {
                out.setAudioSettings(Some(&settings));
                let queue = DispatchQueue::new("handset.ios.audio", None);
                out.setSampleBufferDelegate_queue(Some(ProtocolObject::from_ref(&*delegate)), Some(&queue));
            }
            if unsafe { session.canAddOutput(&out) } {
                unsafe { session.addOutput(&out) };
                audio = true;
            } else {
                eprintln!("audio is not available from {}", device.name);
            }
        }
        unsafe {
            session.commitConfiguration();
            session.startRunning();
        }
        if !unsafe { session.isRunning() } {
            return Err(format!("Mirroring {} did not start. Unlock it and tap Trust if it asks.", device.name));
        }

        let stop = Arc::new(AtomicBool::new(false));
        if audio {
            let stop = stop.clone();
            std::thread::spawn(move || {
                while !stop.load(Relaxed) {
                    std::thread::sleep(Duration::from_millis(20));
                    pacer.fill(&rec_fill);
                }
            });
        }

        // AVFoundation stops delivering frames when the cable is pulled; nothing else says so.
        let (watch, uid) = (stop.clone(), device.uid.clone());
        std::thread::spawn(move || {
            while !watch.load(Relaxed) {
                std::thread::sleep(Duration::from_secs(1));
                let gone = unsafe { AVCaptureDevice::deviceWithUniqueID(&NSString::from_str(&uid)) }
                    .is_none_or(|d| !unsafe { d.isConnected() });
                if gone && !watch.load(Relaxed) {
                    let _ = proxy.send_event(UserEvent::Disconnected);
                    return;
                }
            }
        });

        Ok(Capture { session, name: device.name.clone(), uid: device.uid.clone(), audio, stop, _delegate: delegate })
    }
}

#[cfg(test)]
mod tests {
    use super::Pcm;

    fn s16(out: &[u8]) -> Vec<i16> {
        out.as_chunks::<2>().0.iter().map(|b| i16::from_le_bytes(*b)).collect()
    }

    #[test]
    fn interleaved_s16_stereo_passes_through() {
        let raw: Vec<u8> = [1i16, -2, 3, -4].iter().flat_map(|v| v.to_le_bytes()).collect();
        let pcm = Pcm { float: false, bits: 16, channels: 2, planar: false };
        assert_eq!(s16(&pcm.to_s16_stereo(&raw, 2).unwrap()), [1, -2, 3, -4]);
    }

    #[test]
    fn planar_float_is_interleaved_and_scaled() {
        // left: 0.5, 1.0; right: -0.5, -2.0 (clipped)
        let raw: Vec<u8> = [0.5f32, 1.0, -0.5, -2.0].iter().flat_map(|v| v.to_le_bytes()).collect();
        let pcm = Pcm { float: true, bits: 32, channels: 2, planar: true };
        assert_eq!(s16(&pcm.to_s16_stereo(&raw, 2).unwrap()), [16383, -16383, 32767, -32767]);
    }

    #[test]
    fn mono_plays_on_both_sides_and_odd_layouts_are_refused() {
        let raw: Vec<u8> = [7i16, 9].iter().flat_map(|v| v.to_le_bytes()).collect();
        let mono = Pcm { float: false, bits: 16, channels: 1, planar: false };
        assert_eq!(s16(&mono.to_s16_stereo(&raw, 2).unwrap()), [7, 7, 9, 9]);
        let s24 = Pcm { float: false, bits: 24, channels: 2, planar: false };
        assert!(s24.to_s16_stereo(&raw, 1).is_none());
        assert!(mono.to_s16_stereo(&raw, 3).is_none(), "too short");
    }
}
